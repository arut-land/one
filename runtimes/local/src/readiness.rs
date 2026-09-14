//! One table for the daemon's startup handshake: the line `arutd` prints, the
//! tag byte a `Status` carries, and the string a surface localizes (ADR 0011).
//!
//! Keeping the three in one enum is what stops them diverging -- the previous
//! split let `arutd` print a `NOTREADY io` line that no arm in `child.rs`
//! matched. This is deliberately not a variant of
//! `arut_feature_chat::errors::NodeFailure`: that enum is why a call over an
//! already-open connection failed, this is why the connection never came to be.
//!
//! `ChildHost::connect` returns the reason wrapped in the `Status` its
//! `Host::connect` signature already promises, so a caller that does not know
//! about it still gets `Code::Unavailable` and a message, and one that does
//! recovers the typed reason with [`arut_rpc::StatusDetail::from_status`].
use arut_i18n_macros::Localized;
use arut_rpc::{Code, StatusDetail};
use thiserror::Error;

/// Whether the local node came up, and if not, why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Localized)]
pub enum Readiness {
    #[error("the node is serving")]
    Ready,
    #[error("the node's lease is already held by another running node")]
    LeaseHeld,
    #[error("the node's socket exists but nothing answered on it")]
    SocketUnreachable,
    #[error("the node process could not be started")]
    SpawnFailed,
    #[error("the node did not report readiness in time")]
    TimedOut,
}

impl Readiness {
    /// The handshake line `arutd` prints for each state it can report. The two
    /// states only the parent can observe have no line, and `parse` never
    /// returns them.
    const LINES: [&'static str; 5] = [
        "READY",
        "NOTREADY lease",
        "NOTREADY bind",
        "NOTREADY spawn",
        "",
    ];

    /// The line to print, empty where this state never reaches the handshake.
    #[must_use]
    pub fn line(self) -> &'static str {
        Self::LINES[Self::ALL
            .iter()
            .position(|state| *state == self)
            .expect("every variant is listed in ALL")]
    }

    /// The state a handshake line reports. An unrecognized line means the
    /// daemon died saying something else, which is [`Self::SpawnFailed`].
    #[must_use]
    pub fn parse(line: &str) -> Self {
        if line.is_empty() {
            return Self::SpawnFailed;
        }
        Self::LINES
            .iter()
            .position(|known| *known == line)
            .map_or(Self::SpawnFailed, |index| Self::ALL[index])
    }
}

impl StatusDetail for Readiness {
    const CODE: Code = Code::Unavailable;
    const ALL: &'static [Self] = &[
        Self::Ready,
        Self::LeaseHeld,
        Self::SocketUnreachable,
        Self::SpawnFailed,
        Self::TimedOut,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use arut_rpc::Status;

    #[test]
    fn every_variant_round_trips_through_a_status() {
        for &state in Readiness::ALL {
            let status = state.into_status("test");
            assert_eq!(status.code, Code::Unavailable);
            assert_eq!(Readiness::from_status(&status), Some(state));
        }
        assert_eq!(
            Readiness::from_status(&Status::new(Code::Unavailable, "plain")),
            None
        );
        assert_eq!(
            Readiness::from_status(&Status::new(Code::Internal, "other")),
            None
        );
    }

    #[test]
    fn every_printed_line_parses_back_to_the_state_that_printed_it() {
        for &state in Readiness::ALL {
            let line = state.line();
            if line.is_empty() {
                continue;
            }
            assert_eq!(Readiness::parse(line), state);
        }
        assert_eq!(
            Readiness::parse("NOTREADY something else"),
            Readiness::SpawnFailed
        );
        assert_eq!(Readiness::parse(""), Readiness::SpawnFailed);
    }
}
