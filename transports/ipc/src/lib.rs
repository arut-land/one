#![cfg(unix)]
use arut_rpc::{Request, Response, RpcChannel, RpcFuture, RpcStream};
pub struct IpcChannel(arut_transport_connect_http::HttpRpcChannel);
impl IpcChannel {
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .unix_socket(path.as_ref().to_owned())
            .build()?;
        Ok(Self(
            arut_transport_connect_http::HttpRpcChannel::with_client("http://localhost", client),
        ))
    }
}
impl RpcChannel for IpcChannel {
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
