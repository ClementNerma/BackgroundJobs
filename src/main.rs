#![forbid(unsafe_code)]
#![forbid(unused_must_use)]

mod cmd;
mod daemon;
mod ipc;
mod task;
mod utils;

use utils::logging::PRINT_DEBUG_MESSAGES;
pub use utils::*;

use std::{fs, sync::atomic::Ordering};

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use colored::Colorize;
use tabular::{row, Table};

use crate::{
    cmd::{Action, CheckArgs, Cmd, KillArgs, LogsArgs, RemoveArgs, RunArgs},
    daemon::{is_daemon_running, start_daemon, DaemonClient, TaskStatus, TaskWrapper},
    paging::run_pager,
    sleep::sleep_ms,
    task::Task,
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

    let data_dir = match cmd.custom_data_dir {
        Some(data_dir) => data_dir,
        None => dirs::data_local_dir()
            .context("Failed to get path to local data directory")?
            .join("bjobs"),
    };

    if !data_dir.exists() {
        fs::create_dir(&data_dir).context("Failed to create the data directory")?;
    }

    let socket_path = data_dir.join("bjobs.sock");
    let log_file = data_dir.join("daemon.log");

    match cmd.action {
        Action::List => {
            let mut client = DaemonClient::connect(&socket_path)?;

            let tasks = client.tasks()?;

            if tasks.is_empty() {
                info!("No task found.");
                return Ok(());
            }

            info!("Found {} task(s):", tasks.len().to_string().bright_yellow());
            info!("");

            let mut table = Table::new("{:>} {:<} {:<} {:<} {:<}");

            for TaskWrapper { task, state } in tasks.values() {
                table.add_row(row!(
                    "*".bright_blue(),
                    task.name.bright_yellow(),
                    match &state.lock().unwrap().status {
                        TaskStatus::NotStartedYet => "Not started yet".bright_black(),
                        TaskStatus::Running { child: _ } => "Running".bright_cyan(),
                        TaskStatus::Success => "Succeeded".bright_green(),
                        TaskStatus::Failed { code: _ } => "Failed".bright_red(),
                        TaskStatus::RunnerFailed { message } =>
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
            start_daemon(&socket_path, &log_file, &args)?;
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
            };

            let mut client = DaemonClient::connect(&socket_path)?;

            let tasks = client.tasks()?;

            if let Some(TaskWrapper {
                task: existing,
                state,
            }) = tasks.get(&name)
            {
                if existing.shell == task.shell && existing.cmd == task.cmd && ignore_identicals {
                    if restart_if_finished && state.lock().unwrap().status.is_completed() {
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
            let mut client = DaemonClient::connect(&socket_path)?;

            client.kill(name)?.map_err(|err| anyhow!("{err}"))?;

            success!("Successfully killed task.");
        }

        Action::Remove(RemoveArgs { name }) => {
            let mut client = DaemonClient::connect(&socket_path)?;

            client.remove(name)?.map_err(|err| anyhow!("{err}"))?;

            success!("Successfully removed task.");
        }

        Action::Check(CheckArgs { succeeded, silent }) => {
            let mut client = DaemonClient::connect(&socket_path)?;

            let tasks = client.tasks()?;

            if tasks.is_empty() {
                info!("No task found.");
                return Ok(());
            }

            let mut failed = false;

            let mut fail = |task_name: &str, exit_msg: &str| {
                if !silent {
                    error!("Task {} exited ({})", task_name.bright_yellow(), exit_msg);
                }

                failed = true;
            };

            for (name, task) in tasks {
                match &task.state.lock().unwrap().status {
                    TaskStatus::NotStartedYet | TaskStatus::Running { child: _ } => {}

                    TaskStatus::Success => {
                        if succeeded {
                            fail(&name, &"gracefully".bright_green());
                        }
                    }

                    TaskStatus::Failed { code } => {
                        fail(
                            &name,
                            &match code {
                                None => "failed - no exit code".bright_yellow(),
                                Some(code) => {
                                    format!("failed with exit code {code}").bright_yellow()
                                }
                            },
                        );
                    }

                    TaskStatus::RunnerFailed { message } => fail(
                        &name,
                        &format!("task runner failed with message '{message}'").bright_yellow(),
                    ),
                }
            }

            if failed {
                std::process::exit(1);
            }
        }

        Action::Status => {
            debug!("Checking daemon's status...");

            if !is_daemon_running(&socket_path)? {
                warn!("Daemon is not running.");
                return Ok(());
            }

            debug!("Daemon is running, sending a test request...");

            let mut client = DaemonClient::connect(&socket_path)?;
            let res = client.hello()?;

            if res == "Hello" {
                success!("Daemon is running and responding to requests.");
            } else {
                error!("Daemon responsed unsuccessfully to a test request.");
            }
        }

        Action::Stop => {
            debug!("Asking the daemon to stop...");

            let mut client = DaemonClient::connect(&socket_path)?;

            match client.stop() {
                Ok(()) => {}
                Err(err) => {
                    if let Ok(false) = is_daemon_running(&socket_path) {
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
                match is_daemon_running(&socket_path) {
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

        Action::Logs(LogsArgs {
            task_name,
            pager,
            no_less_options,
        }) => {
            let logs = match task_name {
                Some(task_name) => {
                    let mut client = DaemonClient::connect(&socket_path)?;

                    client
                        .logs(task_name)?
                        .map_err(|err| anyhow!("{err}"))?
                        .join("\n")
                }

                None => fs::read_to_string(&log_file).context("Failed to read the log file")?,
            };

            let pager = pager
                .or_else(|| std::env::var("PAGER").ok())
                .unwrap_or_else(|| "less".to_owned());

            run_pager(&logs, &pager, no_less_options)?;
        }
    }

    Ok(())
}
