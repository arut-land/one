//! Renderable chat metadata and immutable transcript rows.
use crate::errors::ChatError;

#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: u64,
    pub role: ChatRole,
    pub text: String,
    /// Authority acceptance time in Unix milliseconds; zero for older facts.
    pub accepted_at_ms: u64,
}

#[boltffi::data]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChatStatus {
    #[default]
    Idle,
    Sending,
    Failed,
}

#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatState {
    pub id: Option<String>,
    /// Immutable messages through this key are available from `messages_after`.
    pub last_message_id: u64,
    pub status: ChatStatus,
    /// Set exactly when `status` is `Failed`; a surface reads the variant.
    pub error: Option<ChatError>,
}
