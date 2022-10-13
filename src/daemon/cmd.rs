use clap::Args;

#[derive(Args)]
pub struct DaemonStartArgs {
    #[clap(long, help = "Do nothing if the daemon is already started")]
    pub ignore_started: bool,
}
