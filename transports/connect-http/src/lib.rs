//! Connect unary Protobuf and server streams over HTTP.
//!
//! The router owns framing and byte dispatch. Identity compression is supported;
//! fragmented envelopes are bounded to 8 MiB and errors retain codes and details.
//! Request-streaming methods return Unimplemented.
//!
//! IPC reuses this framing and router.

#![cfg(not(target_arch = "wasm32"))]
mod framing;
pub use framing::MAX_MESSAGE;
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
async fn bounded_body(response: reqwest::Response) -> Result<Vec<u8>, Status> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_MESSAGE as u64)
    {
        return Err(framing::message_limit());
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(io)?;
        if chunk.len() > MAX_MESSAGE - body.len() {
            return Err(framing::message_limit());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
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
        if request.message.len() > MAX_MESSAGE {
            return Box::pin(async { Err(framing::message_limit()) });
        }
        let request = self
            .request(procedure, &request.metadata, false)
            .body(request.message);
        Box::pin(async move {
            let response = request.send().await.map_err(io)?;
            let success = response.status().is_success();
            let metadata = metadata(response.headers());
            let body = bounded_body(response).await?;
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
        let body = framing::envelope(0, &request.message);
        let request = self.request(procedure, &request.metadata, true);
        Box::pin(async move {
            let response = request.body(body?).send().await.map_err(io)?;
            let metadata = metadata(response.headers());
            if !response.status().is_success() {
                return Err(framing::parse_error(
                    &serde_json::from_slice(&bounded_body(response).await?).unwrap_or_default(),
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
