use std::path::Path;

use anyhow::Result;

use crate::ipc::{ServiceClient, SocketClient};

use super::service::daemon::{Client, RequestContent as Req, ResponseContent as Res};

pub struct DaemonClient {
    inner: SocketClient<Req, Res>,
}

impl DaemonClient {
    pub fn connect(socket_path: &Path) -> Result<Self> {
        Ok(Self {
            inner: SocketClient::connect(socket_path)?,
        })
    }
}

impl ServiceClient<Req, Res> for DaemonClient {
    fn send_unchecked(&mut self, req: Req) -> Result<Res> {
        self.inner.send_unchecked(req)
    }
}

impl Client for DaemonClient {
    type Client = Self;

    fn retrieve_client(&mut self) -> &mut Self::Client {
        self
    }
}
