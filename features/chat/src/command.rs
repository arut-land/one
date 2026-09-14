use crate::facts::ChatProjection;
use arut_authority::Command;
use arut_protocol::chat::v1::{ChatFact, ChatMessage, ChatRole, OperationFact, OperationPhase};
use arut_rpc::Status;

/// A command's retry key, which ADR 0004 makes the surface's own identity for
/// the operation. Validated here so no other signature accepts a raw string.
#[derive(Debug, Clone)]
pub(crate) struct CommandId(String);

impl CommandId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandId {
    type Error = Status;
    fn try_from(value: String) -> Result<Self, Status> {
        match uuid::Uuid::parse_str(&value) {
            Ok(id) if id.get_version_num() == 7 && id.to_string() == value => Ok(Self(value)),
            _ => Err(Status::invalid_argument(
                "command requires a canonical UUIDv7 ID",
            )),
        }
    }
}

impl From<CommandId> for String {
    fn from(id: CommandId) -> Self {
        id.0
    }
}

pub(crate) struct PendingDraft {
    pub(crate) scope_id: String,
    pub(crate) revision: u64,
}

pub(crate) struct ChatCommand {
    pub(crate) command_id: CommandId,
    pub(crate) chat_id: String,
    pub(crate) pending_scope: Option<PendingDraft>,
    pub(crate) text: String,
}
/// Why the chat authority refused a command, before any surface wording.
#[derive(Debug)]
pub(crate) enum Rejection {
    ConversationMissing,
    ConversationExists,
    /// This command ID already names another conversation or another method.
    CommandConflict,
    /// The pending draft moved on before the start command committed.
    PendingRevisionConflict,
    /// The pending draft does not hold the text the start command carries.
    PendingTextMismatch,
}
impl Command for ChatCommand {
    type Fact = ChatFact;
    type Projection = ChatProjection;
    type Rejection = Rejection;
    fn command_id(&self) -> &str {
        self.command_id.as_str()
    }
    fn epoch(&self) -> u64 {
        1
    }
    fn apply(self, current: &ChatProjection, now: u64) -> Result<ChatFact, Rejection> {
        let exists = current.conversation(&self.chat_id).is_some();
        if self.pending_scope.is_none() && !exists {
            return Err(Rejection::ConversationMissing);
        }
        if self.pending_scope.is_some() && exists {
            return Err(Rejection::ConversationExists);
        }
        Ok(Self::exchange(
            current,
            self.chat_id,
            self.pending_scope,
            self.text,
            self.command_id.into(),
            now,
        ))
    }
}

impl ChatCommand {
    /// One command commits an atomic batch of transcript and operation facts.
    fn exchange(
        current: &ChatProjection,
        chat_id: String,
        pending: Option<PendingDraft>,
        text: String,
        operation_id: String,
        accepted_at_ms: u64,
    ) -> ChatFact {
        let next = current
            .conversation(&chat_id)
            .map_or(0, |conversation| conversation.messages.len()) as u64;
        let reply = format!("You said: {text}");
        ChatFact {
            accepted_at_ms,
            chat_id,
            pending_revision: pending.as_ref().map(|pending| pending.revision),
            pending_scope_id: pending.map_or_else(String::new, |pending| pending.scope_id),
            messages: vec![
                message(next + 1, ChatRole::User, text, accepted_at_ms),
                message(next + 2, ChatRole::Assistant, reply, accepted_at_ms),
            ],
            operations: vec![
                operation(&operation_id, OperationPhase::Started),
                operation(&operation_id, OperationPhase::Completed),
            ],
        }
    }
}

fn message(id: u64, role: ChatRole, text: String, accepted_at_ms: u64) -> ChatMessage {
    ChatMessage {
        accepted_at_ms,
        id,
        role: role as i32,
        text,
    }
}

fn operation(id: &str, phase: OperationPhase) -> OperationFact {
    OperationFact {
        operation_id: id.to_owned(),
        phase: phase as i32,
    }
}
