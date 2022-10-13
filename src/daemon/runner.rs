use std::{
    io::{BufRead, BufReader},
    process::Command,
};

use crate::{
    datetime::get_now,
    task::{Task, TaskResult},
};

use anyhow::{Context, Result};

pub static DEFAULT_SHELL_CMD: &str = "/bin/sh -c";

pub fn runner(task: Task) -> Result<TaskResult> {
    let shell_cmd = task.shell.unwrap_or_else(|| DEFAULT_SHELL_CMD.to_string());

    let mut shell_cmd_parts = shell_cmd.split(' ');

    let mut cmd = Command::new(shell_cmd_parts.next().unwrap());

    for part in shell_cmd_parts {
        cmd.arg(part);
    }

    cmd.arg(&task.cmd);

    let (reader, writer) = os_pipe::pipe().context("Failed to obtain a pipe")?;

    cmd.stdout(writer.try_clone().context("Failed to clone the writer")?);
    cmd.stderr(writer);

    if let Some(start_dir) = task.start_dir {
        cmd.current_dir(start_dir);
    }

    let mut handle = cmd.spawn().context("Failed to spawn the command")?;

    drop(cmd);

    let reader = BufReader::new(reader);

    for line in reader.lines() {
        let line = line.unwrap();

        task.output
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
