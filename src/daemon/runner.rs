use std::{
    io::{BufRead, BufReader},
    process::Command,
    sync::{Arc, Mutex},
};

use crate::{datetime::get_now, task::TaskResult};

use anyhow::{Context, Result};

pub static DEFAULT_SHELL_CMD: &str = "/bin/sh -c";

pub fn runner(
    task_cmd: &str,
    shell_cmd: Option<&str>,
    output: Arc<Mutex<Vec<String>>>,
) -> Result<TaskResult> {
    let shell_cmd = shell_cmd.unwrap_or(DEFAULT_SHELL_CMD);

    let mut shell_cmd_parts = shell_cmd.split(' ');

    let mut cmd = Command::new(shell_cmd_parts.next().unwrap());

    for part in shell_cmd_parts {
        cmd.arg(part);
    }

    cmd.arg(&task_cmd);

    let (reader, writer) = os_pipe::pipe().context("Failed to obtain a pipe")?;

    cmd.stdout(writer.try_clone().context("Failed to clone the writer")?);
    cmd.stderr(writer);

    let mut handle = cmd.spawn().context("Failed to spawn the command")?;

    drop(cmd);

    let reader = BufReader::new(reader);

    for line in reader.lines() {
        let line = line.unwrap();

        output
            .lock()
            .expect("Failed to lock command's output")
            .push(format!("[{}] {}", get_now(), line));
    }

    let status = handle.wait().context("Failed to run the task's command")?;

    Ok(if status.success() {
        TaskResult::Success
    } else {
        TaskResult::Failed {
            code: status.code(),
        }
    })
}
