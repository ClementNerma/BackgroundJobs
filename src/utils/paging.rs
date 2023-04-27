use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

pub fn run_pager(
    fetch_logs: impl Fn() -> Result<String>,
    pager: &str,
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

    loop {
        match child
            .try_wait()
            .with_context(|| format!("Pager command '{pager}' failed"))?
        {
            Some(exit) => {
                if exit.success() {
                    break;
                } else {
                    bail!("Pager command '{pager}' returned a non-zero exit code");
                }
            }

            None => {
                let new_logs = fetch_logs()?;

                if new_logs != prev_logs {
                    let logs_update = new_logs.strip_prefix(&prev_logs).unwrap();

                    let mut stdin = child
                        .stdin
                        .as_ref()
                        .context("Failed to get STDIN pipe from pager")?;

                    write!(stdin, "{logs_update}")
                        .context("Failed to write data to the pager's STDIN pipe")?;

                    prev_logs = new_logs;
                }
            }
        }
    }

    Ok(())
}
