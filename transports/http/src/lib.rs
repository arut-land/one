#[cfg(not(target_arch = "wasm32"))]
use arut_rpc::{Code, Request, Response, RpcChannel, RpcFuture, RpcStream, Status};

#[cfg(not(target_arch = "wasm32"))]
pub struct HttpRpcChannel {
    endpoint: String,
    client: reqwest::Client,
}

#[cfg(not(target_arch = "wasm32"))]
impl HttpRpcChannel {
    pub fn new(endpoint: impl Into<String>) -> Self {
        let client = reqwest::Client::new();
        Self {
            endpoint: endpoint.into().trim_end_matches('/').to_owned(),
            client,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl RpcChannel for HttpRpcChannel {
    fn unary(&self, procedure: &str, request: Request<Vec<u8>>) -> RpcFuture<Response<Vec<u8>>> {
        let client = self.client.clone();
        let url = format!("{}{procedure}", self.endpoint);
        Box::pin(async move {
            let response = client
                .post(url)
                .header("content-type", "application/x-protobuf")
                .body(request.message)
                .send()
                .await
                .map_err(|error| Status::new(Code::Unavailable, error.to_string()))?;
            let status = response.status().as_u16();
            let body = response
                .bytes()
                .await
                .map_err(|error| Status::new(Code::Unavailable, error.to_string()))?
                .to_vec();
            if (200..300).contains(&status) {
                Ok(Response::new(body))
            } else {
                Err(Status::new(
                    status_code(status),
                    String::from_utf8_lossy(&body),
                ))
            }
        })
    }

    fn server_stream(
        &self,
        _procedure: &str,
        _request: Request<Vec<u8>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        unsupported("HTTP response streaming is not implemented")
    }

    fn client_stream(
        &self,
        _procedure: &str,
        _request: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<Vec<u8>>> {
        unsupported("HTTP request streaming is not implemented")
    }

    fn bidirectional(
        &self,
        _procedure: &str,
        _request: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        unsupported("bidirectional HTTP streaming is not implemented")
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn unsupported<T>(message: &'static str) -> RpcFuture<T> {
    Box::pin(async move { Err(Status::new(Code::Unimplemented, message)) })
}

#[cfg(not(target_arch = "wasm32"))]
fn status_code(status: u16) -> Code {
    match status {
        400 => Code::InvalidArgument,
        401 => Code::Unauthenticated,
        403 => Code::PermissionDenied,
        404 => Code::Unimplemented,
        409 => Code::Aborted,
        412 => Code::FailedPrecondition,
        429 => Code::ResourceExhausted,
        503 => Code::Unavailable,
        _ => Code::Internal,
    }
}
