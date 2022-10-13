use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::daemon::DaemonStartArgs;

#[derive(Parser)]
#[clap(author, version)]
pub struct Cmd {
    #[clap(short, long, help = "Display debug messages")]
    pub verbose: bool,

    #[clap(short, long, help = "Path to the socket file")]
    pub socket_path: PathBuf,

    #[clap(subcommand)]
    pub action: Action,
}

#[derive(Subcommand)]
pub enum Action {
    #[clap(about = "List all tasks")]
    List,

    #[clap(about = "Check if any task recently failed")]
    Check,

    #[clap(about = "Start a task")]
    Run(RunArgs),

    #[clap(about = "Stop a task")]
    Kill(KillArgs),

    #[clap(about = "Remove a task")]
    Remove(RemoveArgs),

    #[clap(about = "Start the daemon")]
    Start(DaemonStartArgs),

    #[clap(about = "Check the daemon's status")]
    Status,

    #[clap(about = "Stop the daemon")]
    Stop,

    #[clap(about = "Display the logs")]
    Logs(LogsArgs),
}

#[derive(Args)]
pub struct RunArgs {
    #[clap(help = "Name of the task")]
    pub name: String,

    #[clap(short, long, help = "The command to run")]
    pub cmd: String,

    #[clap(long, help = "The shell to use")]
    pub using: Option<String>,

    #[clap(short, long, help = "Ignore identical commands")]
    pub ignore_identicals: bool,

    #[clap(short, long, help = "Restart if finished")]
    pub restart_if_finished: bool,

    #[clap(long, help = "Don't display messages outside of errors")]
    pub silent: bool,
}

#[derive(Args)]
pub struct KillArgs {
    #[clap(help = "Name of the task to kill")]
    pub name: String,
}

#[derive(Args)]
pub struct RemoveArgs {
    #[clap(help = "Name of the task to unregister")]
    pub name: String,
}

#[derive(Args)]
pub struct LogsArgs {
    #[clap(help = "The task to show the logs of")]
    pub task_name: String,
}
