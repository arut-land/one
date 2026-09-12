//! Typed failures for the chat and composer projections (ADR 0016).
//!
//! Every variant carries the data a surface needs to decide what to say and
//! what to offer; none of them carries the sentence. The `#[error("…")]`
//! attributes exist only so `Display` can put something in a log or a span --
//! no surface reads them, and adding a language touches surfaces only.
//!
//! `arut_rpc::Status` keeps its developer text for exactly that reason: it is
//! the transport's own diagnostic, so it is projected here through its code and
//! its message is left behind at the boundary.
//!
//! Each enum also carries `message_key`, the Fluent message id whose sentence
//! lives once in `product/i18n` (ADR 0022). A key is not text: it names a
//! string a surface renders through its own localization system. The match is
//! exhaustive, so a new variant cannot reach a surface without one, and
//! `arut-i18n`'s `error_keys` test fails if a locale is missing the message.

use arut_rpc::{Code, Status};
use thiserror::Error;

/// Why a call to the node produced no usable answer.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
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

impl NodeFailure {
    /// Every variant, for the localization test that proves each one has a
    /// string in every locale (ADR 0022).
    ///
    /// A new variant makes [`NodeFailure::message_key`] below fail to compile,
    /// which is the reminder to add it here and to `errors.ftl` as well.
    pub const ALL: &'static [Self] = &[
        Self::Unreachable,
        Self::TimedOut,
        Self::Cancelled,
        Self::Refused,
        Self::Overloaded,
        Self::Rejected,
        Self::Missing,
        Self::Conflict,
        Self::Unsupported,
        Self::Internal,
    ];

    /// The Fluent message id a surface renders for this failure (ADR 0022).
    /// This is a key, not text: the sentence lives in `product/i18n`.
    #[must_use]
    pub const fn message_key(self) -> &'static str {
        match self {
            Self::Unreachable => "node-failure-unreachable",
            Self::TimedOut => "node-failure-timed-out",
            Self::Cancelled => "node-failure-cancelled",
            Self::Refused => "node-failure-refused",
            Self::Overloaded => "node-failure-overloaded",
            Self::Rejected => "node-failure-rejected",
            Self::Missing => "node-failure-missing",
            Self::Conflict => "node-failure-conflict",
            Self::Unsupported => "node-failure-unsupported",
            Self::Internal => "node-failure-internal",
        }
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
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

impl ComposerError {
    /// Every variant, one sample payload each, for the localization test.
    ///
    /// A new variant makes [`ComposerError::message_key`] fail to compile.
    pub const ALL: &'static [Self] = &[
        Self::Node(NodeFailure::Unreachable),
        Self::RevisionConflict { current: 0 },
        Self::AuthorityChanged { current_epoch: 0 },
        Self::SnapshotMissing,
        Self::OutcomeMissing,
        Self::ScopeMissing,
        Self::ScopeMismatch,
    ];

    /// The Fluent message id a surface renders for this error (ADR 0022).
    /// The wrapping variants delegate, because a node failure reads the same
    /// whichever scope met it.
    #[must_use]
    pub const fn message_key(self) -> &'static str {
        match self {
            Self::Node(failure) => failure.message_key(),
            Self::RevisionConflict { .. } => "composer-error-revision-conflict",
            Self::AuthorityChanged { .. } => "composer-error-authority-changed",
            Self::SnapshotMissing => "composer-error-snapshot-missing",
            Self::OutcomeMissing => "composer-error-outcome-missing",
            Self::ScopeMissing => "composer-error-scope-missing",
            Self::ScopeMismatch => "composer-error-scope-mismatch",
        }
    }
}

impl From<&Status> for ComposerError {
    fn from(status: &Status) -> Self {
        Self::Node(status.into())
    }
}

impl From<Status> for ComposerError {
    fn from(status: Status) -> Self {
        Self::Node(status.into())
    }
}

/// Why a chat could not accept what a person did.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ChatError {
    #[error("the node did not answer the chat: {0}")]
    Node(NodeFailure),
    /// Sending needs either a conversation or a pending scope to start one.
    #[error("this chat has no conversation to send to")]
    NoConversation,
    /// The conversation scope was cancelled, so nothing was sent.
    #[error("the conversation scope was cancelled")]
    Cancelled,
    #[error("the draft could not be committed: {0}")]
    Draft(ComposerError),
    #[error("the node started a conversation without naming it")]
    ChatIdMissing,
}

impl ChatError {
    /// Every variant, one sample payload each, for the localization test.
    ///
    /// A new variant makes [`ChatError::message_key`] fail to compile.
    pub const ALL: &'static [Self] = &[
        Self::Node(NodeFailure::Unreachable),
        Self::NoConversation,
        Self::Cancelled,
        Self::Draft(ComposerError::SnapshotMissing),
        Self::ChatIdMissing,
    ];

    /// The Fluent message id a surface renders for this error (ADR 0022).
    #[must_use]
    pub const fn message_key(self) -> &'static str {
        match self {
            Self::Node(failure) => failure.message_key(),
            Self::NoConversation => "chat-error-no-conversation",
            Self::Cancelled => "chat-error-cancelled",
            Self::Draft(error) => error.message_key(),
            Self::ChatIdMissing => "chat-error-chat-id-missing",
        }
    }
}

impl From<ComposerError> for ChatError {
    fn from(error: ComposerError) -> Self {
        Self::Draft(error)
    }
}

impl From<&Status> for ChatError {
    fn from(status: &Status) -> Self {
        Self::Node(status.into())
    }
}

impl From<Status> for ChatError {
    fn from(status: Status) -> Self {
        Self::Node(status.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_transport_code_projects_onto_one_product_failure() {
        for (code, expected) in [
            (Code::Unavailable, NodeFailure::Unreachable),
            (Code::DeadlineExceeded, NodeFailure::TimedOut),
            (Code::Cancelled, NodeFailure::Cancelled),
            (Code::PermissionDenied, NodeFailure::Refused),
            (Code::Unauthenticated, NodeFailure::Refused),
            (Code::ResourceExhausted, NodeFailure::Overloaded),
            (Code::InvalidArgument, NodeFailure::Rejected),
            (Code::FailedPrecondition, NodeFailure::Rejected),
            (Code::OutOfRange, NodeFailure::Rejected),
            (Code::NotFound, NodeFailure::Missing),
            (Code::AlreadyExists, NodeFailure::Conflict),
            (Code::Aborted, NodeFailure::Conflict),
            (Code::Unimplemented, NodeFailure::Unsupported),
            (Code::Internal, NodeFailure::Internal),
        ] {
            assert_eq!(NodeFailure::from(code), expected);
        }
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
