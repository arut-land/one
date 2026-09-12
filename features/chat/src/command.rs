use crate::facts::ChatProjection;
use arut_authority::{Command, Projection};
use arut_protocol::chat::v1::ChatFact;

pub struct ChatCommand {
    pub command_id: String,
    pub chat_id: String,
    pub pending_scope: Option<String>,
    pub text: String,
}
#[derive(Debug)]
pub enum Rejection {
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
    fn apply(self, current: &ChatProjection) -> Result<ChatFact, Rejection> {
        let exists = current.conversation(&self.chat_id).is_some();
        if self.pending_scope.is_none() && !exists {
            return Err(Rejection::ConversationMissing);
        }
        if self.pending_scope.is_some() && exists {
            return Err(Rejection::ConversationExists);
        }
        Ok(current.exchange(self.chat_id, self.pending_scope, self.text, self.command_id))
    }
}
impl Projection for ChatProjection {
    type Fact = ChatFact;
    type Scope = String;
    fn reduce(&mut self, fact: &ChatFact) {
        self.apply(fact);
    }
    fn revision(&self, scope: &String) -> u64 {
        self.conversation(scope)
            .map_or(0, |conversation| conversation.messages.len() as u64 / 2)
    }
}
