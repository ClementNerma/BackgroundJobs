use std::collections::BTreeMap;

use crate::service;

use super::task::TaskWrapper;

service!(
    daemon (functions) {
        fn hello() -> String;
        fn stop();

        fn tasks() -> super::super::Tasks;
        fn running_tasks_count() -> usize;

        fn run(task: crate::task::Task);
        fn restart(task_name: String) -> Result<(), String>;
        fn kill(task_name: String) -> Result<(), String>;
        fn logs(task_name: String) -> Result<Vec<String>, String>;
    }
);

mod functions {
    use std::sync::{Arc, RwLock};

    use crate::{
        daemon::{
            runner::runner,
            task::{TaskStatus, TaskWrapper},
        },
        sleep::sleep_ms,
        task::Task,
    };

    use super::Tasks;

    pub type State = RwLock<super::State>;

    pub fn hello(_: Arc<State>) -> String {
        "Hello".to_string()
    }

    pub fn stop(state: Arc<State>) {
        state.write().unwrap().exit = true;

        while state.read().unwrap().exit {
            sleep_ms(20);
        }
    }

    pub fn tasks(state: Arc<State>) -> Tasks {
        state.read().unwrap().tasks.clone()
    }

    pub fn running_tasks_count(state: Arc<State>) -> usize {
        state
            .read()
            .unwrap()
            .tasks
            .values()
            .filter(|task| !task.state.lock().unwrap().status.is_completed())
            .count()
    }

    pub fn run(state: Arc<State>, task: Task) {
        let wrapper = TaskWrapper::new(task);

        state
            .write()
            .unwrap()
            .tasks
            .insert(wrapper.task.name.clone(), wrapper.clone());

        std::thread::spawn(move || {
            let result = runner(wrapper.clone());

            let mut state = state.write().unwrap();
            let task = state.tasks.get_mut(&wrapper.task.name).unwrap();

            if let Err(err) = result {
                task.state.lock().unwrap().status = TaskStatus::RunnerFailed {
                    message: format!("{err:?}"),
                };
            }
        });
    }

    pub fn restart(state: Arc<State>, task_name: String) -> Result<(), String> {
        let task = {
            state
                .write()
                .unwrap()
                .tasks
                .remove(&task_name)
                .ok_or("Provided task was not found")?
        };

        run(state, task.task);

        Ok(())
    }

    pub fn kill(state: Arc<State>, task_name: String) -> Result<(), String> {
        let tasks = &mut state.write().unwrap().tasks;

        let task = tasks
            .get(&task_name)
            .ok_or("Provided task does not exist")?;

        let mut task_state = task.state.lock().unwrap();

        let handle = task_state
            .status
            .get_child()
            .ok_or("Provided task is not running")?;

        handle
            .kill()
            .map_err(|err| format!("Failed to kill task: {err}"))?;

        Ok(())
    }

    pub fn logs(state: Arc<State>, task_name: String) -> Result<Vec<String>, String> {
        let tasks = &state.read().unwrap().tasks;
        let task = tasks.get(&task_name).ok_or("Provided task was not found")?;

        let logs = &task.state.lock().unwrap().output;

        Ok(logs.clone())
    }
}

pub struct State {
    pub exit: bool,
    pub tasks: Tasks,
}

impl State {
    pub fn new() -> Self {
        Self {
            exit: false,
            tasks: Tasks::default(),
        }
    }
}

pub type Tasks = BTreeMap<String, TaskWrapper>;
