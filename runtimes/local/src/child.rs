//! Child lifetime follows the last channel. Readiness is an explicit handshake.
use crate::hosting::ScheduledChannel;
use arut_product_session::hosting::{Host, HostMode};
use arut_rpc::{Code, Request, Response, RpcChannel, RpcFuture, RpcStream, Spawner, Status};
use std::{path::PathBuf, sync::Arc};
use tokio::io::{AsyncBufReadExt, BufReader};

pub struct ChildHost {
    pub executable: PathBuf,
    pub socket: PathBuf,
    pub data: PathBuf,
    pub spawner: Arc<dyn Spawner>,
}
struct ChildChannel {
    channel: ScheduledChannel,
    child: std::sync::Mutex<tokio::process::Child>,
    socket: PathBuf,
}
impl Drop for ChildChannel {
    fn drop(&mut self) {
        let _ = self
            .child
            .get_mut()
            .expect("child lock poisoned")
            .start_kill();
        let _ = std::fs::remove_file(&self.socket);
    }
}
impl Host for ChildHost {
    fn mode(&self) -> HostMode {
        HostMode::ChildProcess
    }
    fn connect(&self) -> RpcFuture<Arc<dyn RpcChannel>> {
        let executable = self.executable.clone();
        let socket = self.socket.clone();
        let data = self.data.clone();
        let spawner = self.spawner.clone();
        Box::pin(async move {
            let mut child = tokio::process::Command::new(executable)
                .env("ARUT_SOCKET", &socket)
                .env("ARUT_DATA", data)
                .stdout(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(unavailable)?;
            let output = child.stdout.take().expect("piped child stdout");
            let mut line = String::new();
            BufReader::new(output)
                .read_line(&mut line)
                .await
                .map_err(unavailable)?;
            if line.trim() != "READY" {
                return Err(Status::new(
                    Code::Unavailable,
                    "daemon exited before readiness",
                ));
            }
            let channel = arut_transport_ipc::IpcChannel::new(&socket).map_err(unavailable)?;
            Ok(Arc::new(ChildChannel {
                channel: ScheduledChannel::new(Arc::new(channel), spawner),
                child: std::sync::Mutex::new(child),
                socket,
            }) as Arc<dyn RpcChannel>)
        })
    }
}
fn unavailable(error: impl std::fmt::Display) -> Status {
    Status::new(Code::Unavailable, error.to_string())
}
impl RpcChannel for ChildChannel {
    fn unary(&self, p: &str, r: Request<Vec<u8>>) -> RpcFuture<Response<Vec<u8>>> {
        self.channel.unary(p, r)
    }
    fn server_stream(
        &self,
        p: &str,
        r: Request<Vec<u8>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        self.channel.server_stream(p, r)
    }
    fn client_stream(
        &self,
        p: &str,
        r: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<Vec<u8>>> {
        self.channel.client_stream(p, r)
    }
    fn bidirectional(
        &self,
        p: &str,
        r: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        self.channel.bidirectional(p, r)
    }
}
