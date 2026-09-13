//! Session failures projected before a feature scope exists.
use arut_rpc::{Code, Status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    Unavailable,
    Cancelled,
    Rejected,
    TimedOut,
    NotFound,
    AlreadyExists,
    Denied,
    Exhausted,
    Changed,
    Unsupported,
    Internal,
}
impl From<Status> for SessionError {
    fn from(error: Status) -> Self {
        match error.code {
            Code::Unavailable => Self::Unavailable,
            Code::Cancelled => Self::Cancelled,
            Code::InvalidArgument | Code::OutOfRange => Self::Rejected,
            Code::DeadlineExceeded => Self::TimedOut,
            Code::NotFound => Self::NotFound,
            Code::AlreadyExists => Self::AlreadyExists,
            Code::PermissionDenied | Code::Unauthenticated => Self::Denied,
            Code::ResourceExhausted => Self::Exhausted,
            Code::FailedPrecondition | Code::Aborted => Self::Changed,
            Code::Unimplemented => Self::Unsupported,
            Code::Internal => Self::Internal,
        }
    }
}
