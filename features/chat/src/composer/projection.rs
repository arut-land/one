//! Renderable composer state, independent of service and authority implementation.
use crate::errors::ComposerError;

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
