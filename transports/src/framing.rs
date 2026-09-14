use arut_rpc::{Code, Status};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD},
};
use bytes::{Buf, BytesMut};
use serde_json::{Value, json};
use tokio_util::codec::Decoder;
pub const MAX_MESSAGE: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnvelopeKind {
    Message,
    End,
}

impl From<EnvelopeKind> for u8 {
    fn from(kind: EnvelopeKind) -> Self {
        match kind {
            EnvelopeKind::Message => 0,
            EnvelopeKind::End => 2,
        }
    }
}

impl TryFrom<u8> for EnvelopeKind {
    type Error = Status;

    fn try_from(flags: u8) -> Result<Self, Self::Error> {
        match flags {
            0 => Ok(Self::Message),
            2 => Ok(Self::End),
            _ => Err(Status::invalid_argument("unsupported envelope flags")),
        }
    }
}

pub(crate) fn envelope(kind: EnvelopeKind, body: &[u8]) -> Result<Vec<u8>, Status> {
    if body.len() > MAX_MESSAGE {
        return Err(message_limit());
    }
    let length = u32::try_from(body.len()).map_err(|_| message_limit())?;
    let mut bytes = Vec::with_capacity(5 + body.len());
    bytes.push(kind.into());
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(body);
    Ok(bytes)
}

pub(crate) fn message_limit() -> Status {
    Status::new(Code::ResourceExhausted, "message exceeds limit")
}

pub(crate) fn end_envelope(error: Option<Status>) -> Vec<u8> {
    let end = error.map_or_else(|| json!({}), |error| json!({"error": error_json(&error)}));
    envelope(EnvelopeKind::End, end.to_string().as_bytes()).unwrap_or_else(|_| {
        envelope(
            EnvelopeKind::End,
            br#"{"error":{"code":"resource_exhausted","message":"message exceeds limit"}}"#,
        )
        .expect("fixed end envelope fits the message limit")
    })
}

/// The Connect envelope: one flags byte, a big-endian length, then the body.
/// Reading it is the `Decoder` contract, so a caller keeps only a buffer.
pub(crate) struct Envelope;

impl Decoder for Envelope {
    type Item = (EnvelopeKind, Vec<u8>);
    type Error = Status;

    fn decode(&mut self, buffer: &mut BytesMut) -> Result<Option<Self::Item>, Status> {
        if buffer.len() < 5 {
            return Ok(None);
        }
        let kind = buffer[0].try_into()?;
        let length =
            u32::from_be_bytes(buffer[1..5].try_into().expect("five header bytes")) as usize;
        if length > MAX_MESSAGE {
            return Err(message_limit());
        }
        if buffer.len() < length + 5 {
            buffer.reserve(length + 5 - buffer.len());
            return Ok(None);
        }
        buffer.advance(5);
        Ok(Some((kind, buffer.split_to(length).to_vec())))
    }
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
    status.details = value["details"][0]["value"]
        .as_str()
        .and_then(|value| decode_binary(value.as_bytes()).ok())
        .unwrap_or_default();
    status
}
pub(crate) fn decode_binary(value: &[u8]) -> Result<Vec<u8>, base64::DecodeError> {
    STANDARD_NO_PAD
        .decode(value)
        .or_else(|_| STANDARD.decode(value))
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
    use super::*;

    fn take(bytes: &[u8]) -> Result<Option<(EnvelopeKind, Vec<u8>)>, Status> {
        Envelope.decode(&mut BytesMut::from(bytes))
    }

    #[test]
    fn framing_handles_fragmentation_limits_and_invalid_flags() {
        let envelope = envelope(EnvelopeKind::Message, b"message").unwrap();
        let mut buffer = BytesMut::new();
        for byte in &envelope[..envelope.len() - 1] {
            buffer.extend_from_slice(&[*byte]);
            assert!(Envelope.decode(&mut buffer).unwrap().is_none());
        }
        buffer.extend_from_slice(&[*envelope.last().unwrap()]);
        let message = Envelope.decode(&mut buffer).unwrap();

        assert_eq!(message, Some((EnvelopeKind::Message, b"message".to_vec())));
        assert!(buffer.is_empty());
        assert!(take(&[1, 0, 0, 0, 0]).is_err());
        assert!(take(&[&[0][..], &((MAX_MESSAGE + 1) as u32).to_be_bytes()].concat()).is_err());
    }

    #[test]
    fn outbound_envelopes_reject_oversized_messages() {
        assert_eq!(
            envelope(EnvelopeKind::Message, &vec![0; MAX_MESSAGE + 1])
                .unwrap_err()
                .code,
            Code::ResourceExhausted
        );
    }
}
