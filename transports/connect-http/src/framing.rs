use arut_rpc::{Code, Status};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
pub const MAX_MESSAGE: usize = 8 * 1024 * 1024;

pub fn envelope(flags: u8, body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(5 + body.len());
    bytes.push(flags);
    bytes.extend_from_slice(&(body.len() as u32).to_be_bytes());
    bytes.extend_from_slice(body);
    bytes
}

pub fn take(buffer: &mut Vec<u8>) -> Result<Option<(u8, Vec<u8>)>, Status> {
    if buffer.len() < 5 {
        return Ok(None);
    }
    let flags = buffer[0];
    if flags != 0 && flags != 2 {
        return Err(Status::invalid_argument("unsupported envelope flags"));
    }
    let length = u32::from_be_bytes(buffer[1..5].try_into().unwrap()) as usize;
    if length > MAX_MESSAGE {
        return Err(Status::new(
            Code::ResourceExhausted,
            "message exceeds limit",
        ));
    }
    if buffer.len() < length + 5 {
        return Ok(None);
    }
    let body = buffer[5..5 + length].to_vec();
    buffer.drain(..5 + length);
    Ok(Some((flags, body)))
}

pub fn error_json(error: &Status) -> Value {
    json!({"code": code_name(error.code), "message": error.message,
        "details": if error.details.is_empty() { vec![] } else { vec![json!({"type": "arut.rpc.StatusDetails", "value": STANDARD.encode(&error.details)})] }})
}
pub fn parse_error(value: &Value) -> Status {
    let code = [
        Code::Cancelled,
        Code::InvalidArgument,
        Code::DeadlineExceeded,
        Code::NotFound,
        Code::AlreadyExists,
        Code::PermissionDenied,
        Code::ResourceExhausted,
        Code::FailedPrecondition,
        Code::Aborted,
        Code::OutOfRange,
        Code::Unimplemented,
        Code::Internal,
        Code::Unavailable,
        Code::Unauthenticated,
    ]
    .into_iter()
    .find(|code| Some(code_name(*code)) == value["code"].as_str())
    .unwrap_or(Code::Internal);
    let mut status = Status::new(code, value["message"].as_str().unwrap_or("RPC failed"));
    status.details = value["details"][0]["value"]
        .as_str()
        .and_then(|s| STANDARD.decode(s).ok())
        .unwrap_or_default();
    status
}
pub fn code_name(code: Code) -> &'static str {
    match code {
        Code::Cancelled => "cancelled",
        Code::InvalidArgument => "invalid_argument",
        Code::DeadlineExceeded => "deadline_exceeded",
        Code::NotFound => "not_found",
        Code::AlreadyExists => "already_exists",
        Code::PermissionDenied => "permission_denied",
        Code::ResourceExhausted => "resource_exhausted",
        Code::FailedPrecondition => "failed_precondition",
        Code::Aborted => "aborted",
        Code::OutOfRange => "out_of_range",
        Code::Unimplemented => "unimplemented",
        Code::Internal => "internal",
        Code::Unavailable => "unavailable",
        Code::Unauthenticated => "unauthenticated",
    }
}
pub fn http_status(code: Code) -> u16 {
    match code {
        Code::Cancelled => 499,
        Code::InvalidArgument | Code::OutOfRange | Code::FailedPrecondition => 400,
        Code::DeadlineExceeded => 504,
        Code::NotFound => 404,
        Code::AlreadyExists | Code::Aborted => 409,
        Code::PermissionDenied => 403,
        Code::ResourceExhausted => 429,
        Code::Unimplemented => 501,
        Code::Internal => 500,
        Code::Unavailable => 503,
        Code::Unauthenticated => 401,
    }
}
