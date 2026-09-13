//! Conversions between composer domain values and Protobuf contracts.
use super::{ComposerScope, ComposerSnapshot, ReplaceOutcome};
use arut_protocol::chat::composer::v1::{
    ComposerAuthorityMismatch, ComposerCommitApplied, ComposerRevisionConflict,
    ComposerScope as WireScope, ComposerSnapshot as WireSnapshot, composer_scope,
    replace_composer_response,
};
pub(crate) fn scope_to_wire(scope: &ComposerScope) -> WireScope {
    WireScope {
        scope_id: Some(match scope {
            ComposerScope::Pending(id) => composer_scope::ScopeId::PendingScopeId(id.clone()),
            ComposerScope::Chat(id) => composer_scope::ScopeId::ChatId(id.clone()),
        }),
    }
}

pub(crate) fn scope_from_wire(scope: WireScope) -> Option<ComposerScope> {
    match scope.scope_id? {
        composer_scope::ScopeId::PendingScopeId(id) if !id.is_empty() => {
            Some(ComposerScope::Pending(id))
        }
        composer_scope::ScopeId::ChatId(id) if !id.is_empty() => Some(ComposerScope::Chat(id)),
        _ => None,
    }
}

impl From<ComposerSnapshot> for WireSnapshot {
    fn from(snapshot: ComposerSnapshot) -> Self {
        Self {
            scope: Some(scope_to_wire(&snapshot.scope)),
            authority_epoch: snapshot.authority_epoch,
            text: snapshot.text,
            revision: snapshot.revision,
        }
    }
}

impl From<ReplaceOutcome> for replace_composer_response::Outcome {
    fn from(outcome: ReplaceOutcome) -> Self {
        match outcome {
            ReplaceOutcome::Applied {
                snapshot,
                duplicate,
            } => Self::Applied(ComposerCommitApplied {
                snapshot: Some(snapshot.into()),
                duplicate,
            }),
            ReplaceOutcome::RevisionConflict { snapshot } => {
                Self::RevisionConflict(ComposerRevisionConflict {
                    snapshot: Some(snapshot.into()),
                })
            }
            ReplaceOutcome::AuthorityMismatch { current_epoch } => {
                Self::AuthorityMismatch(ComposerAuthorityMismatch { current_epoch })
            }
        }
    }
}

impl From<&ComposerScope> for WireScope {
    fn from(scope: &ComposerScope) -> Self {
        scope_to_wire(scope)
    }
}
