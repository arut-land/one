use arut_rpc::{RpcChannel, RpcFuture};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostMode {
    InProcess,
    ChildProcess,
    SystemService,
    Remote,
}

/// The composition root selects a host and passes its channel to the session.
pub trait Host: Send + Sync {
    fn mode(&self) -> HostMode;
    fn connect(&self) -> RpcFuture<Arc<dyn RpcChannel>>;
}
