//! # Connect HTTP
//!
//! Unary Protobuf and server streams follow https://connectrpc.com/docs/protocol/.
//! Identity compression is supported. Unsupported compression flags are rejected.
//! The decoder accepts fragmented envelopes and limits each message to 8 MiB.
//! Errors preserve status codes and opaque details. The axum router owns framing,
//! not product dispatch. Request-streaming methods return Unimplemented.

#![cfg(not(target_arch = "wasm32"))]
pub mod framing;
mod server;
use arut_rpc::{Code, Metadata, Request, Response, RpcChannel, RpcFuture, RpcStream, Status};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
pub use server::router;

pub struct HttpRpcChannel {
    endpoint: String,
    client: reqwest::Client,
}
impl HttpRpcChannel {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self::with_client(endpoint, reqwest::Client::new())
    }
    pub fn with_client(endpoint: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            endpoint: endpoint.into().trim_end_matches('/').to_owned(),
            client,
        }
    }
    fn request(
        &self,
        procedure: &str,
        metadata: &Metadata,
        streaming: bool,
    ) -> reqwest::RequestBuilder {
        let mut request = self
            .client
            .post(format!("{}{procedure}", self.endpoint))
            .header(
                "content-type",
                if streaming {
                    "application/connect+proto"
                } else {
                    "application/proto"
                },
            )
            .header("connect-protocol-version", "1");
        for (key, value) in metadata.iter() {
            request = request.header(
                key,
                if key.ends_with("-bin") {
                    STANDARD.encode(value)
                } else {
                    String::from_utf8_lossy(value).into_owned()
                },
            );
        }
        request
    }
}
fn io(error: reqwest::Error) -> Status {
    Status::new(Code::Unavailable, error.to_string())
}
pub(crate) fn metadata(headers: &axum::http::HeaderMap) -> Metadata {
    let mut result = Metadata::default();
    for (key, value) in headers {
        let bytes = if key.as_str().ends_with("-bin") {
            STANDARD.decode(value.as_bytes()).unwrap_or_default()
        } else {
            value.as_bytes().to_vec()
        };
        result.insert(key.as_str(), bytes);
    }
    result
}
impl RpcChannel for HttpRpcChannel {
    fn unary(&self, procedure: &str, request: Request<Vec<u8>>) -> RpcFuture<Response<Vec<u8>>> {
        let request = self
            .request(procedure, &request.metadata, false)
            .body(request.message);
        Box::pin(async move {
            let response = request.send().await.map_err(io)?;
            let success = response.status().is_success();
            let metadata = metadata(response.headers());
            let body = response.bytes().await.map_err(io)?.to_vec();
            if !success {
                return Err(framing::parse_error(
                    &serde_json::from_slice(&body).unwrap_or_default(),
                ));
            }
            Ok(Response {
                message: body,
                metadata,
            })
        })
    }
    fn server_stream(
        &self,
        procedure: &str,
        request: Request<Vec<u8>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        let request = self
            .request(procedure, &request.metadata, true)
            .body(framing::envelope(0, &request.message));
        Box::pin(async move {
            let response = request.send().await.map_err(io)?;
            let metadata = metadata(response.headers());
            if !response.status().is_success() {
                return Err(framing::parse_error(
                    &response.json::<serde_json::Value>().await.map_err(io)?,
                ));
            }
            let stream = response.bytes_stream();
            let output = futures_util::stream::unfold(
                (Box::pin(stream), Vec::new(), false),
                |(mut input, mut buffer, ended)| async move {
                    if ended {
                        return None;
                    }
                    loop {
                        match framing::take(&mut buffer) {
                            Ok(Some((0, body))) => return Some((Ok(body), (input, buffer, false))),
                            Ok(Some((2, body))) => {
                                let end: serde_json::Value = match serde_json::from_slice(&body) {
                                    Ok(end) => end,
                                    Err(_) => {
                                        return Some((
                                            Err(Status::new(
                                                Code::Internal,
                                                "invalid end envelope",
                                            )),
                                            (input, buffer, true),
                                        ));
                                    }
                                };
                                return end.get("error").map(|error| {
                                    (Err(framing::parse_error(error)), (input, buffer, true))
                                });
                            }
                            Err(error) => return Some((Err(error), (input, buffer, true))),
                            _ => {}
                        }
                        match input.next().await {
                            Some(Ok(bytes)) => buffer.extend_from_slice(&bytes),
                            Some(Err(error)) => {
                                return Some((Err(io(error)), (input, buffer, true)));
                            }
                            None => {
                                return Some((
                                    Err(Status::new(
                                        Code::Internal,
                                        "stream ended without end envelope",
                                    )),
                                    (input, buffer, true),
                                ));
                            }
                        }
                    }
                },
            );
            Ok(Response {
                message: Box::pin(output) as RpcStream<Vec<u8>>,
                metadata,
            })
        })
    }
    fn client_stream(
        &self,
        _: &str,
        _: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<Vec<u8>>> {
        unsupported()
    }
    fn bidirectional(
        &self,
        _: &str,
        _: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        unsupported()
    }
}
fn unsupported<T>() -> RpcFuture<T> {
    Box::pin(async {
        Err(Status::new(
            Code::Unimplemented,
            "request streaming is not enabled",
        ))
    })
}
