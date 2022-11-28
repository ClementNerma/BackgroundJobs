use std::{
    io::{BufRead, BufReader},
    process::Command,
};

use crate::datetime::get_now;

use anyhow::{Context, Result};
use command_group::CommandGroup;

use super::task::{TaskStatus, TaskWrapper};

pub static DEFAULT_SHELL_CMD: &str = "/bin/sh -c";

pub fn runner(TaskWrapper { state, task }: TaskWrapper) -> Result<()> {
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

    let handle = cmd.group_spawn().context("Failed to spawn the command")?;

    state.lock().unwrap().status = TaskStatus::Running {
        child: Some(handle),
    };

    drop(cmd);

    let reader = BufReader::new(reader);

    for line in reader.lines() {
        let line = line.unwrap();

        state
            .lock()
            .expect("Failed to lock the command's output")
            .output
            .push(format!("[{}] {}", get_now(), line));
    }

    let status = state
        .lock()
        .unwrap()
        .status
        .get_child()
        .context("No child handle in running command's status")?
        .wait()
        .context("Failed to run the task's command")?;

    state.lock().unwrap().status = if status.success() {
        TaskStatus::Success
    } else {
        TaskStatus::Failed {
            code: status.code(),
        }
    };

    Ok(())
}
