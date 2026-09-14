use crate::ConversationTitle;
use crate::facts::ChatProjection;
use arut_authority::Command;
use arut_protocol::chat::v1::{
    ChatFact, ChatMessage, ChatRole, ChatStarted, ConversationDeleted, ConversationRenamed,
    MessagesSent, chat_fact,
};
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

#[derive(Clone)]
pub(crate) struct PendingDraft {
    pub(crate) scope_id: String,
    pub(crate) revision: u64,
}

#[derive(Clone)]
pub(crate) enum ChatCommand {
    Start {
        command_id: CommandId,
        chat_id: String,
        pending: PendingDraft,
        text: String,
    },
    Send {
        command_id: CommandId,
        chat_id: String,
        text: String,
    },
    Rename {
        command_id: CommandId,
        chat_id: String,
        title: ConversationTitle,
    },
    Delete {
        command_id: CommandId,
        chat_id: String,
    },
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
        self.command_id().as_str()
    }
    fn epoch(&self) -> u64 {
        1
    }
    fn apply(self, current: &ChatProjection, now: u64) -> Result<ChatFact, Rejection> {
        match self {
            Self::Start {
                chat_id,
                pending,
                text,
                ..
            } => {
                if current.conversation(&chat_id).is_some() {
                    return Err(Rejection::ConversationExists);
                }
                Ok(message_fact(current, chat_id, Some(pending), text, now))
            }
            Self::Send { chat_id, text, .. } => {
                if current.conversation(&chat_id).is_none() {
                    return Err(Rejection::ConversationMissing);
                }
                Ok(message_fact(current, chat_id, None, text, now))
            }
            Self::Rename { chat_id, title, .. } => {
                if current.conversation(&chat_id).is_none() {
                    return Err(Rejection::ConversationMissing);
                }
                Ok(mutation_fact(
                    chat_id,
                    chat_fact::Change::Renamed(ConversationRenamed {
                        title: title.into(),
                    }),
                    now,
                ))
            }
            Self::Delete { chat_id, .. } => {
                if current.conversation(&chat_id).is_none() {
                    return Err(Rejection::ConversationMissing);
                }
                Ok(mutation_fact(
                    chat_id,
                    chat_fact::Change::Deleted(ConversationDeleted {}),
                    now,
                ))
            }
        }
    }
}

impl ChatCommand {
    pub(crate) fn command_id(&self) -> &CommandId {
        match self {
            Self::Start { command_id, .. }
            | Self::Send { command_id, .. }
            | Self::Rename { command_id, .. }
            | Self::Delete { command_id, .. } => command_id,
        }
    }

    pub(crate) fn chat_id(&self) -> &str {
        match self {
            Self::Start { chat_id, .. }
            | Self::Send { chat_id, .. }
            | Self::Rename { chat_id, .. }
            | Self::Delete { chat_id, .. } => chat_id,
        }
    }

    /// A duplicate command ID is valid only when it names the same operation,
    /// not merely the same conversation.
    pub(crate) fn matches(&self, fact: &ChatFact) -> bool {
        if fact.chat_id != self.chat_id() {
            return false;
        }
        match (self, &fact.change) {
            (Self::Start { pending, text, .. }, Some(chat_fact::Change::Started(started))) => {
                started.pending_scope_id == pending.scope_id
                    && started.pending_revision == pending.revision
                    && started
                        .messages
                        .first()
                        .is_some_and(|message| message.text == *text)
            }
            (Self::Send { text, .. }, Some(chat_fact::Change::Sent(sent))) => sent
                .messages
                .first()
                .is_some_and(|message| message.text == *text),
            (Self::Rename { title, .. }, Some(chat_fact::Change::Renamed(renamed))) => {
                title.as_ref() == renamed.title
            }
            (Self::Delete { .. }, Some(chat_fact::Change::Deleted(_))) => true,
            _ => false,
        }
    }
}

fn message_fact(
    current: &ChatProjection,
    chat_id: String,
    pending: Option<PendingDraft>,
    text: String,
    accepted_at_ms: u64,
) -> ChatFact {
    let next = current
        .conversation(&chat_id)
        .map_or(0, |conversation| conversation.messages.len()) as u64;
    let reply = format!("You said: {text}");
    let messages = vec![
        message(next + 1, ChatRole::User, text, accepted_at_ms),
        message(next + 2, ChatRole::Assistant, reply, accepted_at_ms),
    ];
    ChatFact {
        accepted_at_ms,
        chat_id,
        change: Some(match pending {
            Some(pending) => chat_fact::Change::Started(ChatStarted {
                pending_revision: pending.revision,
                pending_scope_id: pending.scope_id,
                messages,
            }),
            None => chat_fact::Change::Sent(MessagesSent { messages }),
        }),
    }
}

fn mutation_fact(chat_id: String, change: chat_fact::Change, accepted_at_ms: u64) -> ChatFact {
    ChatFact {
        accepted_at_ms,
        chat_id,
        change: Some(change),
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
