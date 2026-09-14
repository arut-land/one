use arut_authority::Projection;
use arut_protocol::chat::v1::{ChatFact, ChatProjectionSnapshot, Conversation, chat_fact};

/// The durable projection of accepted chat facts; snapshots persist it as a row.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct ChatProjection {
    pub conversations: Vec<Conversation>,
    pub consumed_drafts: std::collections::HashMap<String, u64>,
}

impl From<ChatProjectionSnapshot> for ChatProjection {
    fn from(snapshot: ChatProjectionSnapshot) -> Self {
        Self {
            conversations: snapshot.conversations,
            consumed_drafts: snapshot.consumed_drafts,
        }
    }
}

impl From<&ChatProjection> for ChatProjectionSnapshot {
    fn from(projection: &ChatProjection) -> Self {
        Self {
            conversations: projection.conversations.clone(),
            consumed_drafts: projection.consumed_drafts.clone(),
        }
    }
}

impl ChatProjection {
    pub fn conversation(&self, chat_id: &str) -> Option<&Conversation> {
        self.conversations
            .iter()
            .find(|conversation| conversation.id == chat_id)
    }
    pub fn apply(&mut self, fact: &ChatFact) {
        match &fact.change {
            Some(chat_fact::Change::Started(started)) => {
                self.consumed_drafts
                    .entry(started.pending_scope_id.clone())
                    .and_modify(|current| *current = (*current).max(started.pending_revision))
                    .or_insert(started.pending_revision);
                self.apply_messages(&fact.chat_id, &started.messages);
            }
            Some(chat_fact::Change::Sent(sent)) => {
                self.apply_messages(&fact.chat_id, &sent.messages);
            }
            Some(chat_fact::Change::Renamed(renamed)) => {
                if let Some(conversation) = self
                    .conversations
                    .iter_mut()
                    .find(|conversation| conversation.id == fact.chat_id)
                {
                    conversation.title = Some(renamed.title.clone());
                }
            }
            Some(chat_fact::Change::Deleted(_)) => self
                .conversations
                .retain(|conversation| conversation.id != fact.chat_id),
            None => {}
        }
    }

    fn apply_messages(&mut self, chat_id: &str, messages: &[arut_protocol::chat::v1::ChatMessage]) {
        match self
            .conversations
            .iter_mut()
            .find(|conversation| conversation.id == chat_id)
        {
            Some(conversation) => conversation.messages.extend(messages.iter().cloned()),
            None => self.conversations.push(Conversation {
                id: chat_id.to_owned(),
                messages: messages.to_vec(),
                title: None,
            }),
        }
    }
}

impl Projection for ChatProjection {
    type Fact = ChatFact;
    type Snapshot = ChatProjectionSnapshot;

    fn restore(snapshot: Self::Snapshot) -> Self {
        snapshot.into()
    }

    fn snapshot(&self) -> Self::Snapshot {
        self.into()
    }

    fn reduce(&mut self, fact: &ChatFact) {
        self.apply(fact);
    }
}
