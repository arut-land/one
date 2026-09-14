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
use arut_protocol::runtime::local::v1::{ReadinessDetail, ReadinessState};
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
    pub const ALL: [Self; 5] = [
        Self::Ready,
        Self::LeaseHeld,
        Self::SocketUnreachable,
        Self::SpawnFailed,
        Self::TimedOut,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidReadinessDetail;

impl std::fmt::Display for InvalidReadinessDetail {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("invalid readiness detail")
    }
}

impl std::error::Error for InvalidReadinessDetail {}

impl From<Readiness> for ReadinessDetail {
    fn from(readiness: Readiness) -> Self {
        Self {
            state: match readiness {
                Readiness::Ready => ReadinessState::Ready,
                Readiness::LeaseHeld => ReadinessState::LeaseHeld,
                Readiness::SocketUnreachable => ReadinessState::SocketUnreachable,
                Readiness::SpawnFailed => ReadinessState::SpawnFailed,
                Readiness::TimedOut => ReadinessState::TimedOut,
            }
            .into(),
        }
    }
}

impl TryFrom<ReadinessDetail> for Readiness {
    type Error = InvalidReadinessDetail;

    fn try_from(detail: ReadinessDetail) -> Result<Self, Self::Error> {
        match ReadinessState::try_from(detail.state).map_err(|_| InvalidReadinessDetail)? {
            ReadinessState::Ready => Ok(Self::Ready),
            ReadinessState::LeaseHeld => Ok(Self::LeaseHeld),
            ReadinessState::SocketUnreachable => Ok(Self::SocketUnreachable),
            ReadinessState::SpawnFailed => Ok(Self::SpawnFailed),
            ReadinessState::TimedOut => Ok(Self::TimedOut),
            ReadinessState::Unspecified => Err(InvalidReadinessDetail),
        }
    }
}

impl StatusDetail for Readiness {
    const CODE: Code = Code::Unavailable;
    type Wire = ReadinessDetail;

    fn into_wire(self) -> Self::Wire {
        self.into()
    }

    fn from_wire(detail: Self::Wire) -> Option<Self> {
        detail.try_into().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arut_rpc::Status;

    #[test]
    fn every_variant_round_trips_through_a_status() {
        for state in Readiness::ALL {
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
    fn every_readiness_state_round_trips_through_its_wire_detail() {
        for state in Readiness::ALL {
            assert_eq!(Readiness::try_from(ReadinessDetail::from(state)), Ok(state));
        }
        assert_eq!(
            Readiness::try_from(ReadinessDetail { state: 99 }),
            Err(InvalidReadinessDetail)
        );
    }
}
