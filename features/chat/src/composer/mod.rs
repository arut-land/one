//! Ephemeral per-scope drafts: last-writer-wins on revision, never facts (ADR 0018).
mod authority;
mod client;
mod scope;
mod service;
use crate::errors::ComposerError;
pub(crate) use authority::{ComposerAuthority, PromoteError};
pub use client::ComposerClient;
pub use scope::ComposerScope;
pub(crate) use scope::{ComposerSnapshot, ReplaceComposer, ReplaceOutcome};
pub(crate) use service::ComposerServiceImpl;

#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerStatus {
    Connecting,
    Synced,
    Failed,
}

#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerState {
    pub text: String,
    pub revision: u64,
    pub status: ComposerStatus,
    /// Set exactly when `status` is `Failed`; a surface reads the variant.
    pub error: Option<ComposerError>,
}
