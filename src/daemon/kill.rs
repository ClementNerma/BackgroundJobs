use std::process::{Child, Command};

use anyhow::{bail, Context, Result};

pub fn kill(child: &Child) -> Result<()> {
    let status = Command::new("kill")
        .arg("-9")
        .arg(format!("-{}", child.id()))
        .status()
        .context("Failed to run the 'kill' command")?;

    if !status.success() {
        bail!("Command 'kill' failed");
    }

    Ok(())
}
