use crate::facts::ChatProjection;
use arut_authority::Command;
use arut_protocol::chat::v1::{ChatFact, ChatMessage, ChatRole, OperationFact, OperationPhase};

pub(crate) struct PendingDraft {
    pub(crate) scope_id: String,
    pub(crate) revision: u64,
}

pub(crate) struct ChatCommand {
    pub(crate) command_id: String,
    pub(crate) chat_id: String,
    pub(crate) pending_scope: Option<PendingDraft>,
    pub(crate) text: String,
}
#[derive(Debug)]
pub(crate) enum Rejection {
    ConversationMissing,
    ConversationExists,
}
impl Command for ChatCommand {
    type Scope = String;
    type Fact = ChatFact;
    type Projection = ChatProjection;
    type Rejection = Rejection;
    fn command_id(&self) -> &str {
        &self.command_id
    }
    fn scope(&self) -> &String {
        &self.chat_id
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
            self.command_id,
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
        ChatFact {
            accepted_at_ms,
            chat_id,
            pending_revision: pending.as_ref().map(|pending| pending.revision),
            pending_scope_id: pending.map_or_else(String::new, |pending| pending.scope_id),
            messages: vec![
                message(next + 1, ChatRole::User, text.clone(), accepted_at_ms),
                message(
                    next + 2,
                    ChatRole::Assistant,
                    crate::domain::respond(&text),
                    accepted_at_ms,
                ),
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
