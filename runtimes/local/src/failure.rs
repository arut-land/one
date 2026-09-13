//! Why the local node could not be reached or started at all (ADR 0011).
//!
//! This is deliberately not a variant of `arut_feature_chat::errors::NodeFailure`:
//! that enum is why a call over an already-open connection failed, and its
//! `strings.rs` rendering on every surface is already exhaustive over its
//! current variants. `NodeStartupFailure` is why the connection never came to
//! be in the first place -- `ChildHost::connect` returns it, wrapped in the
//! `Status` its `Host::connect` signature already promises, so a caller that
//! does not know about it still gets `Code::Unavailable` and a message, and
//! one that does can recover the typed reason with `from_status`.
//!
//! `Code` (`arut_rpc`) stays the transport's own coarse, shared vocabulary, so
//! this does not ask it for a new variant; the reason travels instead in
//! `Status::details` as a single tag byte, the field the type exists for.
use arut_i18n_macros::Localized;
use arut_rpc::{Code, Status};
use thiserror::Error;

/// Why `ChildHost` could not give a session a working connection to the local
/// node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Localized)]
pub enum NodeStartupFailure {
    #[error("the node's lease is already held by another running node")]
    LeaseHeld,
    #[error("the node's socket exists but nothing answered on it")]
    SocketUnreachable,
    #[error("the node process could not be started")]
    SpawnFailed,
}

impl NodeStartupFailure {
    /// Every variant, for the localization test that proves each one has a
    /// string in every locale (ADR 0022).
    pub const ALL: &'static [Self] = &[Self::LeaseHeld, Self::SocketUnreachable, Self::SpawnFailed];

    fn tag(self) -> u8 {
        match self {
            Self::LeaseHeld => 1,
            Self::SocketUnreachable => 2,
            Self::SpawnFailed => 3,
        }
    }

    /// Wrap this reason in a `Status` a `Host::connect` caller can return.
    #[must_use]
    pub fn into_status(self, message: impl Into<String>) -> Status {
        Status {
            code: Code::Unavailable,
            message: message.into(),
            details: vec![self.tag()],
        }
    }

    /// The typed reason behind a connect failure, if `status` carries one.
    /// `None` covers both an ordinary `Status` and one with an unrecognized
    /// tag, so a future variant this binary does not know about degrades to
    /// the plain `Unavailable` a caller already handles.
    #[must_use]
    pub fn from_status(status: &Status) -> Option<Self> {
        if status.code != Code::Unavailable {
            return None;
        }
        match status.details.as_slice() {
            [1] => Some(Self::LeaseHeld),
            [2] => Some(Self::SocketUnreachable),
            [3] => Some(Self::SpawnFailed),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_round_trips_through_a_status() {
        for &failure in NodeStartupFailure::ALL {
            let status = failure.into_status("test");
            assert_eq!(status.code, Code::Unavailable);
            assert_eq!(NodeStartupFailure::from_status(&status), Some(failure));
        }
    }

    #[test]
    fn a_status_with_no_recognized_tag_carries_no_reason() {
        assert_eq!(
            NodeStartupFailure::from_status(&Status::new(Code::Unavailable, "plain")),
            None
        );
        assert_eq!(
            NodeStartupFailure::from_status(&Status::new(Code::Internal, "other")),
            None
        );
    }
}
