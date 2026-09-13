use crate::{framing, metadata::decode as metadata};
use arut_rpc::{Request, RpcChannel, Status};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use std::sync::Arc;

pub fn router(channel: Arc<dyn RpcChannel>) -> Router {
    Router::new()
        .route("/{*procedure}", post(invoke))
        .layer(DefaultBodyLimit::max(framing::MAX_MESSAGE + 5))
        .with_state(channel)
}
fn failure(error: Status) -> Response {
    (
        StatusCode::from_u16(framing::http_status(error.code)).unwrap(),
        [("content-type", "application/json")],
        framing::error_json(&error).to_string(),
    )
        .into_response()
}
fn headers(response: &mut Response, metadata: &arut_rpc::Metadata) {
    for (key, value) in metadata.iter() {
        let value = crate::metadata::encode_value(key, value);
        if let (Ok(name), Ok(value)) = (HeaderName::try_from(key), HeaderValue::try_from(value)) {
            response.headers_mut().insert(name, value);
        }
    }
}
async fn invoke(
    State(channel): State<Arc<dyn RpcChannel>>,
    Path(procedure): Path<String>,
    incoming: HeaderMap,
    body: Bytes,
) -> Response {
    let content_type = incoming
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let streaming = content_type == "application/connect+proto";
    if !streaming && content_type != "application/proto" {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    let mut bytes = body.to_vec();
    let message = if streaming {
        match framing::take(&mut bytes) {
            Ok(Some((0, message))) if bytes.is_empty() => message,
            _ => return failure(Status::invalid_argument("expected one request envelope")),
        }
    } else {
        if bytes.len() > framing::MAX_MESSAGE {
            return failure(framing::message_limit());
        }
        bytes
    };
    let request = Request {
        message,
        metadata: metadata(&incoming),
    };
    let procedure = format!("/{procedure}");
    if !streaming {
        return match channel.unary(&procedure, request).await {
            Ok(result) => {
                if result.message.len() > framing::MAX_MESSAGE {
                    return failure(framing::message_limit());
                }
                let mut response =
                    ([("content-type", "application/proto")], result.message).into_response();
                headers(&mut response, &result.metadata);
                response
            }
            Err(error) => failure(error),
        };
    }
    let result = channel.server_stream(&procedure, request).await;
    let (stream, metadata) = match result {
        Ok(response) => (response.message, response.metadata),
        Err(error) => (
            Box::pin(futures_util::stream::once(async { Err(error) }))
                as arut_rpc::RpcStream<Vec<u8>>,
            Default::default(),
        ),
    };
    let output = crate::streaming::encode(stream);
    let mut response = (
        [("content-type", "application/connect+proto")],
        Body::from_stream(output),
    )
        .into_response();
    headers(&mut response, &metadata);
    response
}
