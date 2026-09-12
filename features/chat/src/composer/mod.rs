pub mod authority;
pub mod product;
pub mod service;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ComposerSnapshot {
    pub scope: ComposerScope,
    pub authority_epoch: u64,
    pub text: String,
    pub revision: u64,
}

impl ComposerSnapshot {
    pub fn empty(scope: ComposerScope) -> Self {
        Self {
            scope,
            authority_epoch: 1,
            text: String::new(),
            revision: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceComposer {
    pub scope: ComposerScope,
    pub command_id: String,
    pub authority_epoch: u64,
    pub base_revision: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplaceOutcome {
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
