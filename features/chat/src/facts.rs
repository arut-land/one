use arut_protocol::chat::v1::{
    ChatFact, ChatMessage, ChatRole, Conversation, OperationFact, OperationPhase,
};

/// The durable projection of accepted chat facts; snapshots persist it as a row.
#[derive(Clone, PartialEq, prost::Message)]
pub struct ChatProjection {
    #[prost(message, repeated, tag = "1")]
    pub conversations: Vec<Conversation>,
    #[prost(string, repeated, tag = "2")]
    pub completed_operations: Vec<String>,
}

impl ChatProjection {
    pub fn conversation(&self, chat_id: &str) -> Option<&Conversation> {
        self.conversations
            .iter()
            .find(|conversation| conversation.id == chat_id)
    }
    pub fn apply(&mut self, fact: &ChatFact) {
        match self
            .conversations
            .iter_mut()
            .find(|conversation| conversation.id == fact.chat_id)
        {
            Some(conversation) => conversation.messages.extend(fact.messages.iter().cloned()),
            None => self.conversations.push(Conversation {
                id: fact.chat_id.clone(),
                messages: fact.messages.clone(),
            }),
        }
        self.completed_operations.extend(
            fact.operations
                .iter()
                .filter(|operation| operation.phase() == OperationPhase::Completed)
                .map(|operation| operation.operation_id.clone()),
        );
    }
    /// One command commits an atomic batch of transcript and operation facts.
    pub fn exchange(
        &self,
        chat_id: String,
        pending_scope_id: Option<String>,
        text: String,
        operation_id: String,
    ) -> ChatFact {
        let next = self
            .conversation(&chat_id)
            .map_or(0, |conversation| conversation.messages.len()) as u64;
        ChatFact {
            chat_id,
            pending_scope_id: pending_scope_id.unwrap_or_default(),
            messages: vec![
                message(next + 1, ChatRole::User, text.clone()),
                message(next + 2, ChatRole::Assistant, crate::domain::respond(&text)),
            ],
            operations: vec![
                operation(&operation_id, OperationPhase::Started),
                operation(&operation_id, OperationPhase::Completed),
            ],
        }
    }
}

fn message(id: u64, role: ChatRole, text: String) -> ChatMessage {
    ChatMessage {
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
