use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::Result;

use crate::{
    daemon::upcoming::{get_new_upcoming_moment, get_upcoming_moment},
    datetime::{get_now, get_now_second_precision},
    info, notice,
    task::{Task, Tasks},
};

pub fn run_tasks(
    tasks: Tasks,
    task_runner: impl Fn(&Task) + Send + Sync + 'static,
    stop_on: impl Fn() -> bool,
) -> Result<()> {
    let task_runner = Arc::new(RwLock::new(task_runner));

    let now = get_now();

    let queue = tasks
        .values()
        .map(|task| {
            (
                task.name.clone(),
                get_upcoming_moment(now, &task.run_at).unwrap(),
            )
        })
        .collect::<HashMap<_, _>>();

    let queue = Arc::new(RwLock::new(queue));

    let short_sleep = || {
        // notice!("Nothing to do, sleeping until the next second...");

        // Sleep until the next second
        let remaining = 1_000_000_000 - get_now().nanosecond();
        std::thread::sleep(Duration::from_nanos(remaining.into()));
    };

    info!("Scheduler is running.");

    while !stop_on() {
        let now = get_now_second_precision();

        let nearest = queue
            .read()
            .unwrap()
            .iter()
            .min_by_key(|(_, moment)| **moment)
            .map(|(a, b)| (a.clone(), *b));

        let (task_name, planned_for) = match nearest {
            None => {
                short_sleep();
                continue;
            }
            Some((_, planned_for)) if planned_for > now => {
                short_sleep();
                continue;
            }
            Some(nearest) => nearest,
        };

        queue.write().unwrap().remove(&task_name).unwrap();

        let queue = Arc::clone(&queue);
        let task = tasks.get(&task_name).unwrap().clone();
        let task_runner = Arc::clone(&task_runner);

        notice!(
            "Running task '{}' late of {} second(s).",
            task.name,
            (now - planned_for).whole_seconds()
        );

        std::thread::spawn(move || {
            task_runner.read().unwrap()(&task);

            let mut queue = queue.write().unwrap();

            let planned = get_new_upcoming_moment(get_now(), &task.run_at, planned_for).unwrap();

            queue.insert(task.name.clone(), planned);
        });
    }

    Ok(())
}
