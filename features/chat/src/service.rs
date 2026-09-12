use crate::command::{ChatCommand, Rejection};
use crate::composer::authority::{ComposerAuthority, PromoteError};
use crate::ports::IdSource;
use crate::{
    composer::{ComposerScope, ComposerSnapshot},
    facts::ChatProjection,
};
use arut_authority::{Authority, Outcome};
use arut_protocol::chat::v1::{
    ChatFact, ChatService, ListConversationsRequest, ListConversationsResponse, SendMessageRequest,
    SendMessageResponse, StartChatRequest, StartChatResponse,
};
use arut_rpc::{Code, Request, Response, RpcFuture, Status};
use arut_storage::{FactLog, StorageError};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct ChatServiceImpl {
    composer: Arc<ComposerAuthority>,
    authority: Authority<ChatCommand>,
    start_gate: Mutex<()>,
    ids: Arc<dyn IdSource>,
}
impl ChatServiceImpl {
    pub fn new(
        composer: Arc<ComposerAuthority>,
        log: Arc<dyn FactLog<ChatFact>>,
        ids: Arc<dyn IdSource>,
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
    fn commit(&self, command: ChatCommand) -> Result<ChatFact, CommitError> {
        match self.authority.execute(command)? {
            Outcome::Applied(record) | Outcome::Duplicate(record) => Ok(record.fact),
            Outcome::RevisionConflict { .. } => Err(CommitError::RevisionConflict),
            Outcome::AuthorityMismatch { .. } => Err(CommitError::AuthorityMismatch),
            Outcome::Superseded => Err(CommitError::Superseded),
            Outcome::Rejected(rejection) => Err(CommitError::Rejected(rejection)),
        }
    }
}

/// Why an accepted command could not become a fact, before any surface wording.
#[derive(Debug)]
enum CommitError {
    Storage(StorageError),
    Rejected(Rejection),
    RevisionConflict,
    AuthorityMismatch,
    Superseded,
}
impl From<StorageError> for CommitError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}
impl From<CommitError> for Status {
    fn from(error: CommitError) -> Self {
        match error {
            CommitError::Storage(error) => storage(error),
            CommitError::Rejected(Rejection::ConversationMissing) => {
                Self::new(Code::NotFound, "conversation not found")
            }
            CommitError::Rejected(Rejection::ConversationExists) => {
                Self::new(Code::AlreadyExists, "conversation already started")
            }
            CommitError::RevisionConflict => {
                Self::new(Code::Aborted, "conversation revision changed")
            }
            CommitError::AuthorityMismatch => {
                Self::new(Code::FailedPrecondition, "conversation authority changed")
            }
            CommitError::Superseded => Self::new(Code::Aborted, "command superseded"),
        }
    }
}
fn start_response(fact: ChatFact) -> StartChatResponse {
    StartChatResponse {
        composer: Some(ComposerSnapshot::empty(ComposerScope::chat(&fact.chat_id)).into()),
        chat_id: fact.chat_id,
        messages: fact.messages,
    }
}
fn storage(error: StorageError) -> Status {
    Status::new(Code::Internal, error.to_string())
}
impl ChatService for ChatServiceImpl {
    fn list_conversations(
        &self,
        _: Request<ListConversationsRequest>,
    ) -> RpcFuture<Response<ListConversationsResponse>> {
        let conversations = self.projection().conversations;
        Box::pin(async move { Ok(Response::new(ListConversationsResponse { conversations })) })
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
                    messages: record.fact.messages,
                }));
            }
            let fact = self.commit(ChatCommand {
                command_id: message.command_id,
                chat_id: message.chat_id,
                pending_scope: None,
                text: message.text,
            })?;
            Ok(Response::new(SendMessageResponse {
                messages: fact.messages,
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
            let result = self.composer.promote_pending(
                &message.pending_scope_id,
                message.expected_revision,
                &message.text,
                &chat_id,
                || self.commit(command),
            )?;
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
    use crate::ports::NativeIds;
    use arut_protocol::chat::v1::ChatServiceClient;
    use arut_rpc::Request;
    use arut_storage::MemoryLog;
    use futures_executor::block_on;

    fn service(composer: Arc<ComposerAuthority>) -> ChatServiceImpl {
        ChatServiceImpl::new(
            composer,
            Arc::new(MemoryLog::default()),
            Arc::new(NativeIds),
        )
        .expect("empty memory log")
    }

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
        let client = ChatServiceClient::direct(Arc::new(service(Arc::clone(&composer))));

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
        let client = ChatServiceClient::direct(Arc::new(service(composer)));
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
