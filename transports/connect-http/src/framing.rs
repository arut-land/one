use arut_rpc::{Code, Status};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD},
};
use serde_json::{Value, json};
pub const MAX_MESSAGE: usize = 8 * 1024 * 1024;

pub(crate) fn envelope(flags: u8, body: &[u8]) -> Result<Vec<u8>, Status> {
    if body.len() > MAX_MESSAGE {
        return Err(message_limit());
    }
    let length = u32::try_from(body.len()).map_err(|_| message_limit())?;
    let mut bytes = Vec::with_capacity(5 + body.len());
    bytes.push(flags);
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(body);
    Ok(bytes)
}

pub(crate) fn message_limit() -> Status {
    Status::new(Code::ResourceExhausted, "message exceeds limit")
}

pub(crate) fn end_envelope(error: Option<Status>) -> Vec<u8> {
    let end = error.map_or_else(|| json!({}), |error| json!({"error": error_json(&error)}));
    envelope(2, end.to_string().as_bytes()).unwrap_or_else(|_| {
        envelope(
            2,
            br#"{"error":{"code":"resource_exhausted","message":"message exceeds limit"}}"#,
        )
        .expect("fixed end envelope fits the message limit")
    })
}

pub(crate) fn take(buffer: &mut Vec<u8>) -> Result<Option<(u8, Vec<u8>)>, Status> {
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

pub(crate) fn error_json(error: &Status) -> Value {
    json!({"code": code_name(error.code), "message": error.message,
        "details": if error.details.is_empty() { vec![] } else { vec![json!({"type": "arut.rpc.StatusDetails", "value": STANDARD.encode(&error.details)})] }})
}
pub(crate) fn parse_error(value: &Value) -> Status {
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
    // Connect writes detail values with the unpadded standard alphabet;
    // accept the padded form too rather than dropping a well-formed detail.
    status.details = value["details"][0]["value"]
        .as_str()
        .and_then(|s| {
            STANDARD_NO_PAD
                .decode(s)
                .or_else(|_| STANDARD.decode(s))
                .ok()
        })
        .unwrap_or_default();
    status
}
fn code_name(code: Code) -> &'static str {
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
pub(crate) fn http_status(code: Code) -> u16 {
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

#[cfg(test)]
mod tests {
    #[test]
    fn framing_handles_fragmentation_limits_and_invalid_flags() {
        use super::*;
        let envelope = envelope(0, b"message").unwrap();
        let mut buffer = vec![];
        for byte in &envelope[..envelope.len() - 1] {
            buffer.push(*byte);
            assert!(take(&mut buffer).unwrap().is_none());
        }
        buffer.push(*envelope.last().unwrap());
        assert_eq!(take(&mut buffer).unwrap(), Some((0, b"message".to_vec())));
        assert!(buffer.is_empty());
        assert!(take(&mut vec![1, 0, 0, 0, 0]).is_err());
        let mut too_large = vec![0];
        too_large.extend_from_slice(&((MAX_MESSAGE + 1) as u32).to_be_bytes());
        assert!(take(&mut too_large).is_err());
    }

    #[test]
    fn outbound_envelopes_reject_oversized_messages() {
        assert_eq!(
            super::envelope(0, &vec![0; super::MAX_MESSAGE + 1])
                .unwrap_err()
                .code,
            arut_rpc::Code::ResourceExhausted
        );
    }
}
