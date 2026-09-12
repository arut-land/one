//! # Memory
//!
//! A byte-channel adapter for in-process services. It preserves metadata, errors,
//! and all streaming shapes without spawning or encoding an extra time.

use arut_rpc::{Request, Response, RpcChannel, RpcFuture, RpcStream};
pub struct MemoryChannel(std::sync::Arc<dyn RpcChannel>);
impl MemoryChannel {
    pub fn new(channel: std::sync::Arc<dyn RpcChannel>) -> Self {
        Self(channel)
    }
}
impl RpcChannel for MemoryChannel {
    fn unary(&self, p: &str, r: Request<Vec<u8>>) -> RpcFuture<Response<Vec<u8>>> {
        self.0.unary(p, r)
    }
    fn server_stream(
        &self,
        p: &str,
        r: Request<Vec<u8>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        self.0.server_stream(p, r)
    }
    fn client_stream(
        &self,
        p: &str,
        r: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<Vec<u8>>> {
        self.0.client_stream(p, r)
    }
    fn bidirectional(
        &self,
        p: &str,
        r: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        self.0.bidirectional(p, r)
    }
}
