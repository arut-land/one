use crate::composer::authority::{ComposerAuthority, PromoteError};
use crate::{
    composer::{ComposerScope, ComposerSnapshot},
    facts::{ChatFact, ChatProjection},
};
use arut_protocol::chat::v1::{
    ChatMessage, ChatRole, ChatService, SendMessageRequest, SendMessageResponse, StartChatRequest,
    StartChatResponse,
};
use arut_rpc::{Code, Request, Response, RpcFuture, Status};
use arut_storage::{FactLog, MemoryLog, StorageError};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct ChatServiceImpl {
    composer: Arc<ComposerAuthority>,
    log: Arc<dyn FactLog<ChatFact>>,
    state: Mutex<Stored>,
}
struct Stored {
    projection: ChatProjection,
    sequence: u64,
}
impl Default for ChatServiceImpl {
    fn default() -> Self {
        Self::new(Arc::new(ComposerAuthority::default()))
    }
}
impl ChatServiceImpl {
    pub fn new(composer: Arc<ComposerAuthority>) -> Self {
        Self::with_log(composer, Arc::new(MemoryLog::default())).expect("empty memory log")
    }
    pub fn with_log(
        composer: Arc<ComposerAuthority>,
        log: Arc<dyn FactLog<ChatFact>>,
    ) -> Result<Self, StorageError> {
        let snapshot = log.snapshot()?;
        let mut projection: ChatProjection = snapshot
            .as_ref()
            .map(|s| serde_json::from_slice(&s.data))
            .transpose()?
            .unwrap_or_default();
        let mut sequence = snapshot.as_ref().map_or(0, |s| s.sequence);
        for record in log.read_from(sequence)? {
            projection.apply(&record.fact);
            sequence = record.sequence;
        }
        Ok(Self {
            composer,
            log,
            state: Mutex::new(Stored {
                projection,
                sequence,
            }),
        })
    }
    pub fn composer_authority(&self) -> Arc<ComposerAuthority> {
        self.composer.clone()
    }
    pub fn projection(&self) -> ChatProjection {
        self.state.lock().unwrap().projection.clone()
    }
    fn commit(
        &self,
        state: &mut Stored,
        id: &str,
        fact: ChatFact,
    ) -> Result<ChatFact, StorageError> {
        let record = self.log.append(state.sequence, 1, id, fact)?;
        if record.sequence > state.sequence {
            state.projection.apply(&record.fact);
            state.sequence = record.sequence;
        }
        Ok(record.fact)
    }
}
fn wire(messages: Vec<crate::product::ChatMessage>) -> Vec<ChatMessage> {
    messages
        .into_iter()
        .map(|m| ChatMessage {
            id: m.id,
            text: m.text,
            role: match m.role {
                crate::product::ChatRole::User => ChatRole::User,
                crate::product::ChatRole::Assistant => ChatRole::Assistant,
            } as i32,
        })
        .collect()
}
fn start_response(fact: ChatFact) -> StartChatResponse {
    StartChatResponse {
        composer: Some(ComposerSnapshot::empty(ComposerScope::chat(&fact.chat_id)).into()),
        chat_id: fact.chat_id,
        messages: wire(fact.messages),
    }
}
fn storage(error: StorageError) -> Status {
    Status::new(Code::Internal, error.to_string())
}
impl ChatService for ChatServiceImpl {
    fn list_conversations(
        &self,
        _: Request<arut_protocol::chat::v1::ListConversationsRequest>,
    ) -> RpcFuture<Response<arut_protocol::chat::v1::ListConversationsResponse>> {
        let conversations = self
            .projection()
            .conversations
            .into_iter()
            .map(|(id, messages)| arut_protocol::chat::v1::Conversation {
                id,
                messages: wire(messages),
            })
            .collect();
        Box::pin(async move {
            Ok(Response::new(
                arut_protocol::chat::v1::ListConversationsResponse { conversations },
            ))
        })
    }

    fn send_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> RpcFuture<Response<SendMessageResponse>> {
        let message = request.message;
        let result = (|| {
            if Uuid::parse_str(&message.command_id)
                .ok()
                .is_none_or(|id| id.get_version_num() != 7)
            {
                return Err(Status::invalid_argument(
                    "send requires a UUIDv7 command ID",
                ));
            }
            let mut state = self.state.lock().unwrap();
            if let Some(record) = self.log.outcome_of(&message.command_id).map_err(storage)? {
                return Ok(Response::new(SendMessageResponse {
                    messages: wire(record.fact.messages),
                }));
            }
            if !state
                .projection
                .conversations
                .contains_key(&message.chat_id)
            {
                return Err(Status::new(Code::NotFound, "conversation not found"));
            }
            let fact = state.projection.exchange(
                message.chat_id,
                None,
                message.text,
                message.command_id.clone(),
            );
            let fact = self
                .commit(&mut state, &message.command_id, fact)
                .map_err(storage)?;
            Ok(Response::new(SendMessageResponse {
                messages: wire(fact.messages),
            }))
        })();
        Box::pin(async move { result })
    }
    fn start_chat(
        &self,
        request: Request<StartChatRequest>,
    ) -> RpcFuture<Response<StartChatResponse>> {
        let message = request.message;
        let result = (|| {
            let mut state = self.state.lock().unwrap();
            if let Some(record) = self.log.outcome_of(&message.command_id).map_err(storage)? {
                return Ok(Response::new(start_response(record.fact)));
            }
            let chat_id = Uuid::now_v7().to_string();
            let fact = state.projection.exchange(
                chat_id.clone(),
                Some(message.pending_scope_id.clone()),
                message.text.clone(),
                message.command_id.clone(),
            );
            let result = self
                .composer
                .promote_pending(
                    &message.pending_scope_id,
                    message.expected_revision,
                    &message.text,
                    &chat_id,
                    || self.commit(&mut state, &message.command_id, fact),
                )
                .map_err(storage)?;
            match result {
                Ok((fact, _)) => Ok(Response::new(start_response(fact))),
                Err(PromoteError::RevisionConflict(_)) => Err(Status::new(
                    Code::Aborted,
                    "pending composer revision changed",
                )),
                Err(PromoteError::TextMismatch(_)) => Err(Status::new(
                    Code::FailedPrecondition,
                    "pending composer text changed",
                )),
            }
        })();
        Box::pin(async move { result })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composer::{ComposerScope, ReplaceComposer};
    use arut_protocol::chat::v1::ChatServiceClient;
    use arut_rpc::Request;
    use futures_executor::block_on;

    #[test]
    fn first_send_atomically_promotes_pending_composer() {
        let composer = Arc::new(ComposerAuthority::default());
        composer.replace(ReplaceComposer {
            scope: ComposerScope::pending("pending"),
            command_id: "draft".into(),
            authority_epoch: 1,
            base_revision: 0,
            text: "hello".into(),
        });
        let client =
            ChatServiceClient::direct(Arc::new(ChatServiceImpl::new(Arc::clone(&composer))));

        let response = block_on(client.start_chat(Request::new(StartChatRequest {
            pending_scope_id: "pending".into(),
            command_id: "start".into(),
            expected_revision: 1,
            text: "hello".into(),
        })))
        .unwrap()
        .message;

        assert!(!response.chat_id.is_empty());
        assert_eq!(response.messages.len(), 2);
        assert_eq!(
            composer
                .snapshot(&ComposerScope::pending("pending"))
                .revision,
            0
        );
        assert_eq!(
            composer
                .snapshot(&ComposerScope::chat(response.chat_id))
                .revision,
            0
        );
    }

    #[test]
    fn retrying_a_start_command_returns_the_original_chat_and_messages() {
        let composer = Arc::new(ComposerAuthority::default());
        composer.replace(ReplaceComposer {
            scope: ComposerScope::pending("pending"),
            command_id: "draft".into(),
            authority_epoch: 1,
            base_revision: 0,
            text: "hello".into(),
        });
        let client = ChatServiceClient::direct(Arc::new(ChatServiceImpl::new(composer)));
        let request = StartChatRequest {
            pending_scope_id: "pending".into(),
            command_id: "stable-start".into(),
            expected_revision: 1,
            text: "hello".into(),
        };

        let first = block_on(client.start_chat(Request::new(request.clone())))
            .unwrap()
            .message;
        let retry = block_on(client.start_chat(Request::new(request)))
            .unwrap()
            .message;

        assert_eq!(retry, first);
    }
}
