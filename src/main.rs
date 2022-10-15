#![forbid(unsafe_code)]
#![forbid(unused_must_use)]

mod cmd;
mod daemon;
mod ipc;
mod task;
mod utils;

use utils::logging::PRINT_DEBUG_MESSAGES;
pub use utils::*;

use std::sync::{atomic::Ordering, Arc, Mutex, RwLock};

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use colored::Colorize;
use minus::Pager;
use tabular::{row, Table};

use crate::{
    cmd::{Action, Cmd, KillArgs, LogsArgs, RemoveArgs, RunArgs},
    daemon::{is_daemon_running, start_daemon, DaemonClient},
    sleep::sleep_ms,
    task::{Task, TaskResult},
};

fn main() {
    if let Err(err) = inner_main() {
        error_anyhow!(err);
        std::process::exit(1);
    }
}

fn inner_main() -> Result<()> {
    debug!("Entered inner main.");

    let cmd = Cmd::parse();

    if cmd.verbose {
        PRINT_DEBUG_MESSAGES.store(true, Ordering::SeqCst);
    }

    match cmd.action {
        Action::List => {
            let mut client = DaemonClient::connect(&cmd.socket_path)?;

            let tasks = client.tasks()?;

            if tasks.is_empty() {
                info!("No task found.");
                return Ok(());
            }

            info!("Found {} task(s):", tasks.len().to_string().bright_yellow());
            info!("");

            let mut table = Table::new("{:>} {:<} {:<} {:<} {:<}");

            for task in tasks.values() {
                table.add_row(row!(
                    "*".bright_blue(),
                    task.name.bright_yellow(),
                    match &task.result {
                        None => "Running".bright_cyan(),
                        Some(TaskResult::Success) => "Succeeded".bright_green(),
                        Some(TaskResult::Failed { code: _ }) => "Failed".bright_red(),
                        Some(TaskResult::RunnerFailed { message }) =>
                            format!("Runner failed ({message})").bright_red(),
                    },
                    match &task.shell {
                        Some(shell) => shell.bright_magenta(),
                        None => "-".bright_black(),
                    },
                    task.cmd.bright_magenta(),
                ));
            }

            println!("{}", table);
        }

        Action::Start(args) => {
            start_daemon(&cmd.socket_path, &args)?;
        }

        Action::Run(RunArgs {
            name,
            using: shell,
            cmd: task_cmd,
            start_dir,
            silent,
            ignore_identicals,
            restart_if_finished,
        }) => {
            let task = Task {
                name: name.clone(),
                cmd: task_cmd,
                shell,
                start_dir,
                result: None,
                output: Arc::new(Mutex::new(vec![])),
                child_handle: Arc::new(RwLock::new(None)),
            };

            let mut client = DaemonClient::connect(&cmd.socket_path)?;

            let tasks = client.tasks()?;

            if let Some(existing) = tasks.get(&name) {
                if existing.shell == task.shell && existing.cmd == task.cmd && ignore_identicals {
                    if restart_if_finished && existing.result.is_some() {
                        if !silent {
                            success!("Restarting task {}.", name.bright_yellow());
                        }

                        client.restart(task.name)?.map_err(|err| anyhow!("{err}"))?;
                    }

                    return Ok(());
                }

                bail!("A task with this name already exists!");
            }

            client.run(task)?;

            if !silent {
                success!("Successfully registered task {}.", name.bright_yellow());
            }
        }

        Action::Kill(KillArgs { name }) => {
            let mut client = DaemonClient::connect(&cmd.socket_path)?;

            let tasks = client.tasks()?;

            let task = tasks
                .get(&name)
                .with_context(|| format!("Unknown task '{name}'"))?;

            if task.result.is_some() {
                bail!("Task is not running!");
            }

            client.kill(name)?.map_err(|err| anyhow!("{err}"))?;
        }

        Action::Check => todo!(),

        Action::Remove(RemoveArgs { name: _ }) => todo!(),

        Action::Status => {
            debug!("Checking daemon's status...");

            if !is_daemon_running(&cmd.socket_path)? {
                warn!("Daemon is not running.");
                return Ok(());
            }

            debug!("Daemon is running, sending a test request...");

            let mut client = DaemonClient::connect(&cmd.socket_path)?;
            let res = client.hello()?;

            if res == "Hello" {
                success!("Daemon is running and responding to requests.");
            } else {
                error!("Daemon responsed unsuccessfully to a test request.");
            }
        }

        Action::Stop => {
            debug!("Asking the daemon to stop...");

            let mut client = DaemonClient::connect(&cmd.socket_path)?;

            match client.stop() {
                Ok(()) => {}
                Err(err) => {
                    if let Ok(false) = is_daemon_running(&cmd.socket_path) {
                        success!("Daemon was successfully stopped!");
                        return Ok(());
                    }

                    return Err(err);
                }
            }

            debug!("Request succesfully transmitted, waiting for the daemon to actually stop...");

            let mut last_running = 0;
            let mut had_error = false;

            loop {
                match is_daemon_running(&cmd.socket_path) {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(err) => {
                        if had_error {
                            return Err(err);
                        }

                        had_error = true;
                        sleep_ms(20);
                        continue;
                    }
                }

                let running = match client.running_tasks_count() {
                    Ok(running) => running,
                    Err(err) => {
                        if had_error {
                            return Err(err);
                        }

                        had_error = true;
                        sleep_ms(20);
                        continue;
                    }
                };

                if running != last_running {
                    info!("Waiting for {} task(s) to complete...", running);
                    last_running = running;
                }

                sleep_ms(100);
            }

            success!("Daemon was successfully stopped!");
        }

        Action::Logs(LogsArgs { task_name }) => {
            let mut client = DaemonClient::connect(&cmd.socket_path)?;

            let logs = client
                .logs(task_name)?
                .map_err(|err| anyhow!("{err}"))?
                .join("\n");

            let output = Pager::new();

            output
                .set_text(&logs)
                .context("Failed to write log content to the pager")?;

            minus::page_all(output).context("Pager failed")?;
        }
    }

    Ok(())
}
