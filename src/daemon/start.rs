use std::{
    fs::{self, OpenOptions},
    os::unix::net::UnixListener,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc, Mutex, RwLock},
};

use anyhow::{bail, Context, Result};
use daemonize_me::Daemon;
use once_cell::sync::Lazy;

use crate::{
    daemon::{
        is_daemon_running,
        service::{daemon::process, State},
        DaemonClient, DaemonStartArgs,
    },
    datetime::get_now_second_precision,
    error, info,
    ipc::serve_on_socket,
    logging::PRINT_MESSAGES_DATETIME,
    sleep::sleep_ms,
    success,
};

static SOCKET_FILE_PATH: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));

pub fn start_daemon(socket_path: &Path, log_file: &Path, args: &DaemonStartArgs) -> Result<()> {
    if is_daemon_running(&socket_path)? {
        if args.ignore_started {
            return Ok(());
        }

        bail!("Daemon is already running.");
    }

    if socket_path.exists() {
        fs::remove_file(&socket_path).context("Failed to remove the existing socket file")?;
    }

    *SOCKET_FILE_PATH.lock().unwrap() = Some(socket_path.to_path_buf());

    if log_file.exists() {
        fs::remove_file(&log_file).context("Failed to remove the log file")?;
    }

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .context("Failed to open the log file")?;

    Daemon::new()
        .stdout(log_file.try_clone().unwrap())
        .stderr(log_file)
        .setup_post_fork_parent_hook(fork_exit)
        .start()
        .context("Failed to start the daemon")?;

    PRINT_MESSAGES_DATETIME.store(true, Ordering::SeqCst);

    if let Err(err) = daemon_core(socket_path) {
        error!("Daemon exited with an error: {:?}", err);
        std::process::exit(1);
    }

    #[allow(unreachable_code)]
    {
        unreachable!()
    }
}

fn daemon_core(socket_path: &Path) -> Result<()> {
    info!(
        "Successfully started the daemon on {}",
        get_now_second_precision()
    );

    info!("Setting up the socket...");

    let socket = UnixListener::bind(&socket_path)
        .context("Failed to create socket with the provided path")?;

    info!("Launching a separate thread for the socket listener...");

    let state = Arc::new(RwLock::new(State::new()));
    let state_server = Arc::clone(&state);

    std::thread::spawn(|| serve_on_socket(socket, process, state_server));

    daemon_core_loop(socket_path, state)
}

fn daemon_core_loop(socket_path: &Path, state: Arc<RwLock<State>>) -> ! {
    info!("Starting the engine...");

    loop {
        if state.read().unwrap().exit {
            info!("Exiting safely as requested...");

            state.write().unwrap().exit = false;

            let tasks = state.read().unwrap().tasks.clone();

            info!("[Exiting] Terminating {} tasks...", tasks.len());

            for (i, task) in tasks.values().enumerate() {
                info!("[Exiting] Terminating task {} / {}...", i + 1, tasks.len());

                if let Some(child) = task.state.lock().unwrap().status.get_child() {
                    // TODO: error management
                    child.kill().unwrap();
                }
            }

            info!("[Exiting] Terminated all tasks.");
            info!("[Exiting] Now exiting.");

            if let Err(err) = fs::remove_file(&socket_path) {
                error!("Failed to remove the socket file, this might cause problem during the next start: {err}");
            }

            std::process::exit(0);
        }

        sleep_ms(100);
    }
}

fn fork_exit(_parent_pid: i32, _child_pid: i32) -> ! {
    let guard = SOCKET_FILE_PATH.lock().unwrap();
    let socket_path = guard.as_ref().unwrap();

    while !socket_path.exists() {
        sleep_ms(50);
    }

    let mut client = DaemonClient::connect(socket_path).unwrap();
    client.hello().unwrap();

    success!("Successfully started BJobs daemon!");

    std::process::exit(0);
}
