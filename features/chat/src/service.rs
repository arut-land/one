use crate::command::ChatCommand;
use crate::composer::authority::{ComposerAuthority, PromoteError};
use crate::{
    composer::{ComposerScope, ComposerSnapshot},
    facts::{ChatFact, ChatProjection},
};
use arut_authority::{Authority, Outcome};
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
    authority: Authority<ChatCommand>,
    start_gate: Mutex<()>,
    ids: Arc<dyn crate::ports::IdSource>,
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
        Self::with_log_and_ids(composer, log, Arc::new(crate::ports::NativeIds))
    }
    pub fn with_log_and_ids(
        composer: Arc<ComposerAuthority>,
        log: Arc<dyn FactLog<ChatFact>>,
        ids: Arc<dyn crate::ports::IdSource>,
    ) -> Result<Self, StorageError> {
        Ok(Self {
            ids,
            composer,
            authority: Authority::open(log, 1)?,
            start_gate: Mutex::new(()),
        })
    }
    pub fn composer_authority(&self) -> Arc<ComposerAuthority> {
        self.composer.clone()
    }
    pub fn projection(&self) -> ChatProjection {
        self.authority.projection()
    }
    fn commit(&self, command: ChatCommand) -> Result<ChatFact, StorageError> {
        match self.authority.execute(command)? {
            Outcome::Applied(record) | Outcome::Duplicate(record) => Ok(record.fact),
            Outcome::RevisionConflict { current } => {
                Err(StorageError::Conflict { actual: current })
            }
            Outcome::AuthorityMismatch { current_epoch } => Err(StorageError::Epoch {
                current: current_epoch,
            }),
            outcome => Err(StorageError::Corrupt(format!(
                "command rejected: {outcome:?}"
            ))),
        }
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

            if let Some(record) = self
                .authority
                .outcome_of(&message.command_id)
                .map_err(storage)?
            {
                return Ok(Response::new(SendMessageResponse {
                    messages: wire(record.fact.messages),
                }));
            }
            if !self
                .authority
                .projection()
                .conversations
                .contains_key(&message.chat_id)
            {
                return Err(Status::new(Code::NotFound, "conversation not found"));
            }
            let fact = self
                .commit(ChatCommand {
                    command_id: message.command_id,
                    chat_id: message.chat_id,
                    pending_scope: None,
                    text: message.text,
                })
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
        let _start = self.start_gate.lock().unwrap();
        let result = (|| {
            if let Some(record) = self
                .authority
                .outcome_of(&message.command_id)
                .map_err(storage)?
            {
                return Ok(Response::new(start_response(record.fact)));
            }
            let chat_id = self.ids.new_id();
            let command = ChatCommand {
                command_id: message.command_id.clone(),
                chat_id: chat_id.clone(),
                pending_scope: Some(message.pending_scope_id.clone()),
                text: message.text.clone(),
            };
            let result = self
                .composer
                .promote_pending(
                    &message.pending_scope_id,
                    message.expected_revision,
                    &message.text,
                    &chat_id,
                    || self.commit(command),
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
