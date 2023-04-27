use std::io::Write;
use std::process::{Child, Command, Stdio};

use anyhow::{bail, Context, Result};

pub fn run_pager(
    fetch_logs: impl Fn() -> Result<String>,
    pager: &str,
    follow: bool,
    no_less_options: bool,
) -> Result<()> {
    let mut cmd = Command::new(pager);

    if pager == "less" && !no_less_options {
        cmd.arg(
            // Handle ANSI color sequences
            "-R",
        );

        cmd.arg(
            // Quit if input is smaller than the screen's size
            "-F",
        );
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to run pager: {pager}"))?;

    let mut prev_logs = String::new();

    let mut pipe_logs = |child: &mut Child| -> Result<()> {
        let logs = fetch_logs()?;

        if logs == prev_logs {
            return Ok(());
        }

        let updated_logs = logs.strip_prefix(&prev_logs).unwrap();

        let mut stdin = child
            .stdin
            .as_ref()
            .context("Failed to get STDIN pipe from pager")?;

        write!(stdin, "{updated_logs}").context("Failed to pipe logs into pager")?;

        prev_logs = logs;

        Ok(())
    };

    pipe_logs(&mut child)?;

    let exit = if !follow {
        child.wait()?
    } else {
        loop {
            match child.try_wait()? {
                Some(exit) => break exit,
                None => pipe_logs(&mut child)?,
            }
        }
    };

    if !exit.success() {
        bail!("Pager command '{pager}' returned a non-zero exit code");
    }

    Ok(())
}
