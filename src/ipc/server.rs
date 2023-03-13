use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    sync::Arc,
    time::Duration,
};

use serde::{de::DeserializeOwned, Serialize};

use crate::{error, sleep::sleep_ms};

use super::{PartialRequest, Request, Response};

pub fn serve_on_socket<A: DeserializeOwned, B: Serialize, S: Send + Sync + 'static>(
    listener: UnixListener,
    process: impl Fn(A, Arc<S>) -> B + Send + Sync + 'static,
    state: Arc<S>,
) -> ! {
    let process = Arc::new(process);

    for client in listener.incoming() {
        let client = match client {
            Ok(client) => client,
            Err(err) => {
                error!("Failed to retrieve client: {err}");
                continue;
            }
        };

        // if let Err(err) = client.set_nonblocking(true) {
        //     error!("Failed to set client in non-blocking mode: {err}");
        //     continue;
        // }

        let process = Arc::clone(&process);
        let state = Arc::clone(&state);
        std::thread::spawn(move || serve_client(client, process, state));
    }

    unreachable!()
}

fn serve_client<A: DeserializeOwned, B: Serialize, S>(
    mut client: UnixStream,
    process: Arc<impl Fn(A, Arc<S>) -> B>,
    state: Arc<S>,
) {
    loop {
        let mut message = String::new();

        if let Err(err) = BufReader::new(&client).read_line(&mut message) {
            error!(
                "Failed to read message from the client (waiting before retrying): {:?}",
                err
            );
            sleep_ms(5000);
        }

        if message.is_empty() {
            break;
        }

        let res = match serde_json::from_str::<Request<A>>(&message) {
            Ok(Request { id, content }) => Response {
                for_id: id,
                result: Ok(process(content, Arc::clone(&state))),
            },

            Err(err) => match serde_json::from_str::<PartialRequest>(&message) {
                Ok(PartialRequest { id }) => Response {
                    for_id: id,
                    result: Err(format!("Failed to parse client request: {err}")),
                },

                Err(_) => {
                    error!("Failed to parse request from client: {err}");
                    short_sleep();
                    continue;
                }
            },
        };

        let mut res = match serde_json::to_string(&res) {
            Ok(res) => res,
            Err(err) => {
                error!("Failed to stringify response for client: {err}");
                short_sleep();
                continue;
            }
        };

        // Message separator
        res.push('\n');

        if let Err(err) = client.write_all(res.as_bytes()) {
            error!("Failed to transmit response to client: {err}");
            short_sleep();
            continue;
        }

        if let Err(err) = client.flush() {
            error!("Failed to flush the client's stream: {err}");
            short_sleep();
            continue;
        }
    }
}

fn short_sleep() {
    std::thread::sleep(Duration::from_millis(100))
}
