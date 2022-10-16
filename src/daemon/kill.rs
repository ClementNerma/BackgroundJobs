use std::process::{Child, Command};

use anyhow::{bail, Context, Result};

pub fn kill(child: &Child) -> Result<()> {
    let status = Command::new("pkill")
        .args(&["-P", &child.id().to_string()])
        .status()
        .context("Failed to run the 'pkill' command")?;

    if !status.success() {
        bail!("Command 'pkill' failed");
    }

    Ok(())
}
