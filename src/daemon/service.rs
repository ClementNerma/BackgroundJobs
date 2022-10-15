use std::collections::BTreeMap;

use crate::{service, task::Task};

service!(
    daemon (functions) {
        fn hello() -> String;
        fn stop();

        fn tasks() -> super::super::Tasks;
        fn running_tasks_count() -> usize;

        fn run(task: crate::task::Task);
        fn restart(task_name: String);
        fn kill(task_name: String);
        fn logs(task_name: String) -> Vec<String>;
    }
);

mod functions {
    use std::sync::{Arc, Mutex, RwLock};

    use crate::{
        daemon::runner::runner,
        sleep::sleep_ms,
        task::{Task, TaskResult},
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
            .filter(|task| task.result.is_none())
            .count()
    }

    pub fn run(state: Arc<State>, task: Task) {
        state.write().unwrap().tasks.insert(
            task.name.clone(),
            Task {
                result: None,
                output: Arc::new(Mutex::new(vec![])),
                ..task.clone()
            },
        );

        std::thread::spawn(move || {
            let result = runner(task.clone());

            let mut state = state.write().unwrap();
            let task = state.tasks.get_mut(&task.name).unwrap();

            match result {
                Ok(result) => task.result = Some(result),

                Err(err) => {
                    task.result = Some(TaskResult::RunnerFailed {
                        message: format!("{err:?}"),
                    })
                }
            }
        });
    }

    pub fn restart(state: Arc<State>, task_name: String) {
        let task = { state.write().unwrap().tasks.remove(&task_name).unwrap() };

        run(state, task)
    }

    pub fn kill(state: Arc<State>, task_name: String) {
        state
            .write()
            .unwrap()
            .tasks
            .get(&task_name)
            .unwrap()
            .child_handle
            .write()
            .unwrap()
            .as_mut()
            .unwrap()
            .kill()
            .unwrap()
    }

    pub fn logs(state: Arc<State>, task_name: String) -> Vec<String> {
        state
            .read()
            .unwrap()
            .tasks
            .get(&task_name)
            .unwrap()
            .output
            .lock()
            .unwrap()
            .clone()
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

pub type Tasks = BTreeMap<String, Task>;
