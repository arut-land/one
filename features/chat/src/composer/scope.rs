//! Composer domain values and the Protobuf contracts they cross the wire as.
use arut_protocol::chat::composer::v1::{
    ComposerAuthorityMismatch, ComposerCommitApplied, ComposerRevisionConflict,
    ComposerScope as WireScope, ComposerSnapshot as WireSnapshot, composer_scope,
    replace_composer_response,
};
use arut_rpc::Status;
use arut_storage::StorageError;

/// The draft a conversation holds, or the one a chat that does not exist yet holds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ComposerScope {
    Pending(String),
    Chat(String),
}

impl ComposerScope {
    pub fn pending(id: impl Into<String>) -> Self {
        Self::Pending(id.into())
    }

    pub fn chat(id: impl Into<String>) -> Self {
        Self::Chat(id.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComposerSnapshot {
    pub scope: ComposerScope,
    pub authority_epoch: u64,
    pub text: String,
    pub revision: u64,
}

impl ComposerSnapshot {
    pub(crate) fn empty(scope: ComposerScope) -> Self {
        Self {
            scope,
            authority_epoch: 1,
            text: String::new(),
            revision: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReplaceComposer {
    pub scope: ComposerScope,
    pub command_id: String,
    pub authority_epoch: u64,
    pub base_revision: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplaceOutcome {
    Applied {
        snapshot: ComposerSnapshot,
        duplicate: bool,
    },
    RevisionConflict {
        snapshot: ComposerSnapshot,
    },
    AuthorityMismatch {
        current_epoch: u64,
    },
}

impl From<&ComposerScope> for WireScope {
    fn from(scope: &ComposerScope) -> Self {
        Self {
            scope_id: Some(match scope {
                ComposerScope::Pending(id) => composer_scope::ScopeId::PendingScopeId(id.clone()),
                ComposerScope::Chat(id) => composer_scope::ScopeId::ChatId(id.clone()),
            }),
        }
    }
}

impl TryFrom<WireScope> for ComposerScope {
    type Error = Status;
    fn try_from(scope: WireScope) -> Result<Self, Status> {
        match scope.scope_id {
            Some(composer_scope::ScopeId::PendingScopeId(id)) if !id.is_empty() => {
                Ok(Self::Pending(id))
            }
            Some(composer_scope::ScopeId::ChatId(id)) if !id.is_empty() => Ok(Self::Chat(id)),
            _ => Err(Status::invalid_argument("composer scope is required")),
        }
    }
}

impl From<ComposerSnapshot> for WireSnapshot {
    fn from(snapshot: ComposerSnapshot) -> Self {
        Self {
            scope: Some((&snapshot.scope).into()),
            authority_epoch: snapshot.authority_epoch,
            text: snapshot.text,
            revision: snapshot.revision,
        }
    }
}

impl TryFrom<WireSnapshot> for ComposerSnapshot {
    type Error = StorageError;
    fn try_from(snapshot: WireSnapshot) -> Result<Self, StorageError> {
        Ok(Self {
            scope: snapshot
                .scope
                .ok_or(StorageError::Corrupt)?
                .try_into()
                .map_err(|_: Status| StorageError::Corrupt)?,
            authority_epoch: snapshot.authority_epoch,
            text: snapshot.text,
            revision: snapshot.revision,
        })
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
