use crate::product::{ChatMessage, ChatRole};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One command commits an atomic batch of transcript and operation facts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatFact {
    pub chat_id: String,
    pub pending_scope: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub operation: Vec<OperationFact>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OperationFact {
    Started { id: String },
    Completed { id: String },
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatProjection {
    pub conversations: BTreeMap<String, Vec<ChatMessage>>,
    pub completed_operations: Vec<String>,
}
impl ChatProjection {
    pub fn apply(&mut self, fact: &ChatFact) {
        self.conversations
            .entry(fact.chat_id.clone())
            .or_default()
            .extend(fact.messages.clone());
        for operation in &fact.operation {
            if let OperationFact::Completed { id } = operation {
                self.completed_operations.push(id.clone());
            }
        }
    }
    pub fn exchange(
        &self,
        chat_id: String,
        pending_scope: Option<String>,
        text: String,
        operation_id: String,
    ) -> ChatFact {
        let next = self
            .conversations
            .get(&chat_id)
            .map_or(0, |messages| messages.len()) as u64;
        ChatFact {
            chat_id,
            pending_scope,
            messages: vec![
                ChatMessage {
                    id: next + 1,
                    role: ChatRole::User,
                    text: text.clone(),
                },
                ChatMessage {
                    id: next + 2,
                    role: ChatRole::Assistant,
                    text: crate::domain::respond(&text),
                },
            ],
            operation: vec![
                OperationFact::Started {
                    id: operation_id.clone(),
                },
                OperationFact::Completed { id: operation_id },
            ],
        }
    }
}
