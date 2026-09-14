//! Typed failures for the chat and composer projections (ADR 0016).
//!
//! Every variant carries the data a surface needs to decide what to say and
//! what to offer; none of them carries the sentence. The `#[error("…")]`
//! attributes exist only so `Display` can put something in a log or a span --
//! surfaces select strings from the shared Fluent source through native resources.
//!
//! `arut_rpc::Status` keeps its developer text for exactly that reason: it is
//! the transport's own diagnostic, so it is projected here through its code and
//! its message is left behind at the boundary.
//!
//! `#[derive(Localized)]` gives each enum `message_key`, the Fluent message id
//! whose sentence lives once in `product/i18n` (ADR 0022). A key is not text:
//! it names a string a surface renders through its own localization system.
//! The derive reads the string source while this crate compiles, so a variant
//! added without a message does not build. It is a proc macro and nothing it
//! emits names a type from above this layer, so the core stays where it is.
//! It also emits `MESSAGE_KEYS`, the keys each enum names itself, which is what
//! `product/i18n/tests/error_keys.rs` checks every locale against, and
//! `message_args`, each variant's fields beside the Fluent names that select
//! them, so no layer above restates the argument order.

use arut_i18n_macros::Localized;
use arut_rpc::{Code, Status};
use thiserror::Error;

/// Why a call to the node produced no usable answer.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Localized)]
pub enum NodeFailure {
    #[error("the node could not be reached")]
    Unreachable,
    #[error("the node did not answer in time")]
    TimedOut,
    #[error("the request was cancelled before the node answered")]
    Cancelled,
    #[error("the node refused the request")]
    Refused,
    #[error("the node is over its limits")]
    Overloaded,
    #[error("the node rejected the request as it stands")]
    Rejected,
    #[error("what the request names is gone")]
    Missing,
    #[error("something else changed first")]
    Conflict,
    #[error("the node does not offer this")]
    Unsupported,
    #[error("the node failed to carry the request out")]
    Internal,
}

impl From<Code> for NodeFailure {
    fn from(code: Code) -> Self {
        match code {
            Code::Unavailable => Self::Unreachable,
            Code::DeadlineExceeded => Self::TimedOut,
            Code::Cancelled => Self::Cancelled,
            Code::PermissionDenied | Code::Unauthenticated => Self::Refused,
            Code::ResourceExhausted => Self::Overloaded,
            Code::InvalidArgument | Code::FailedPrecondition | Code::OutOfRange => Self::Rejected,
            Code::NotFound => Self::Missing,
            Code::AlreadyExists | Code::Aborted => Self::Conflict,
            Code::Unimplemented => Self::Unsupported,
            Code::Internal => Self::Internal,
        }
    }
}

impl From<&Status> for NodeFailure {
    fn from(status: &Status) -> Self {
        status.code.into()
    }
}

impl From<Status> for NodeFailure {
    fn from(status: Status) -> Self {
        status.code.into()
    }
}

/// Why the draft in one composer scope is not what the node holds.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Localized)]
pub enum ComposerError {
    #[error("the node did not answer the composer: {0}")]
    Node(NodeFailure),
    /// Someone else's edit landed first; `current` is the revision that won.
    #[error("the draft moved to revision {current} before this edit committed")]
    RevisionConflict { current: u64 },
    /// The conversation's authority moved; the edit was written against an old one.
    #[error("the conversation authority moved on to epoch {current_epoch}")]
    AuthorityChanged { current_epoch: u64 },
    #[error("the node answered without the snapshot the composer needs")]
    SnapshotMissing,
    #[error("the node answered without an outcome for the edit")]
    OutcomeMissing,
    #[error("the node answered without naming a composer scope")]
    ScopeMissing,
    #[error("the node answered for a different composer scope")]
    ScopeMismatch,
}

/// Why a chat could not accept what a person did.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Localized)]
pub enum ChatError {
    #[error("the node did not answer the chat: {0}")]
    Node(NodeFailure),
    /// The conversation scope was cancelled, so nothing was sent.
    #[error("the conversation scope was cancelled")]
    Cancelled,
    #[error("the draft could not be committed: {0}")]
    Draft(#[from] ComposerError),
    #[error("the node started a conversation without naming it")]
    ChatIdMissing,
}

macro_rules! node_errors {
    ($($error:ty),+ $(,)?) => {$(
        impl From<&Status> for $error {
            fn from(status: &Status) -> Self { Self::Node(status.into()) }
        }
        impl From<Status> for $error {
            fn from(status: Status) -> Self { Self::from(&status) }
        }
    )+};
}
node_errors!(ComposerError, ChatError);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_error_carries_its_arguments_under_the_names_its_message_interpolates() {
        assert_eq!(
            ComposerError::RevisionConflict { current: 7 }.message_args(),
            [("current".to_owned(), "7".to_owned())]
        );
        assert_eq!(
            ComposerError::AuthorityChanged { current_epoch: 2 }.message_args(),
            [("currentEpoch".to_owned(), "2".to_owned())]
        );
        assert!(ComposerError::ScopeMismatch.message_args().is_empty());
    }

    #[test]
    fn a_delegating_variant_forwards_the_key_and_the_arguments_it_wraps() {
        let draft = ComposerError::RevisionConflict { current: 9 };
        let error = ChatError::Draft(draft);
        assert_eq!(error.message_key(), draft.message_key());
        assert_eq!(error.message_args(), draft.message_args());
        assert!(
            ChatError::Node(NodeFailure::TimedOut)
                .message_args()
                .is_empty()
        );
    }

    #[test]
    fn a_status_reaches_a_projection_as_a_code_and_never_as_its_text() {
        let status = Status::new(Code::Aborted, "conversation revision changed");
        assert_eq!(
            ChatError::from(&status),
            ChatError::Node(NodeFailure::Conflict)
        );
        assert_eq!(
            ComposerError::from(status),
            ComposerError::Node(NodeFailure::Conflict)
        );
    }
}
