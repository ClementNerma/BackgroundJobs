use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::{Context, Result};

use crate::{
    daemon::upcoming::get_upcoming_moment,
    datetime::get_now,
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

    let sleep_for = |seconds: u32| -> bool {
        let now = get_now();

        while (get_now() - now).whole_seconds() < seconds.into() {
            std::thread::sleep(Duration::from_secs(1));

            if stop_on() {
                return true;
            }
        }

        false
    };

    info!("Scheduler is running.");

    loop {
        if tasks.is_empty() {
            notice!("No task registered, sleeping for 1 second.");

            if sleep_for(1) {
                return Ok(());
            }

            continue;
        }

        let now = get_now();

        let nearest = queue
            .read()
            .unwrap()
            .iter()
            .min_by_key(|(_, moment)| **moment)
            .map(|(a, b)| (a.clone(), *b));

        let (task_name, planned_for) = match nearest {
            Some(nearest) => nearest,
            None => {
                notice!("No planned task for now, sleeping for 1 second.");
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
        };

        if planned_for > now {
            notice!("No task to run, checking free time before next task...");

            let can_sleep_for = queue
                .read()
                .unwrap()
                .iter()
                .map(|(_, moment)| (*moment - now).whole_seconds())
                .min()
                .context("No future task found in queue, should not be empty")
                .unwrap();

            notice!(
                "Nearest task scheduled to run in {} second(s), sleeping until then.",
                can_sleep_for
            );

            let can_sleep_for: u32 = u64::try_from(can_sleep_for)
                .context("Found negative waiting time for planned task")
                .unwrap()
                .try_into()
                .context("Found >32-bit waiting time for planned task")
                .unwrap();

            // NOTE: Waiting for one more second is required as it can otherwise lead
            // to a very tricky bug: the clock may get to the task's planned time, minus
            // a few milliseconds or even microseconds. In which case, this will run thousands of times.
            if sleep_for(can_sleep_for + 1) {
                return Ok(());
            }
            continue;
        }

        queue.write().unwrap().remove(&task_name);

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

            queue.insert(
                task.name.clone(),
                get_upcoming_moment(get_now(), &task.run_at).unwrap(),
            );
        });

        if stop_on() {
            return Ok(());
        }
    }
}
