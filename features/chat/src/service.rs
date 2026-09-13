//! Converts generated service requests and outcomes at the chat boundary.
#[cfg(test)]
use crate::ChatProjection;
use crate::authority::{ChatAuthority, CommitError};
use crate::command::Rejection;
use crate::composer::ComposerAuthority;
use crate::ports::{Clock, IdSource};
use arut_protocol::chat::v1::{
    ChatFact, ChatService, ListConversationsRequest, ListConversationsResponse, SendMessageRequest,
    SendMessageResponse, StartChatRequest, StartChatResponse,
};
use arut_rpc::{Code, Request, Response, RpcFuture, Status};
use arut_storage::{FactLog, StorageError};
use std::sync::Arc;
use uuid::Uuid;

pub(crate) struct ChatServiceImpl {
    authority: ChatAuthority,
}
impl ChatServiceImpl {
    pub fn new<R: IdSource + Clock + Send + Sync>(
        composer: Arc<ComposerAuthority>,
        log: Arc<dyn FactLog<ChatFact>>,
        ids: Arc<R>,
    ) -> Result<Self, StorageError> {
        Ok(Self {
            authority: ChatAuthority::new(composer, log, ids)?,
        })
    }
    #[cfg(test)]
    pub fn projection(&self) -> ChatProjection {
        self.authority.projection()
    }
}
impl From<CommitError> for Status {
    fn from(error: CommitError) -> Self {
        match error {
            CommitError::Storage(error) => Status::new(Code::Internal, error.to_string()),
            CommitError::PendingRevisionConflict => {
                Status::new(Code::Aborted, "pending composer revision changed")
            }
            CommitError::PendingTextMismatch => {
                Status::new(Code::FailedPrecondition, "pending composer text changed")
            }
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
            CommitError::CommandConflict => Self::new(
                Code::AlreadyExists,
                "command ID belongs to another operation",
            ),
        }
    }
}
fn validate_command_id(value: &str) -> Result<(), Status> {
    if Uuid::parse_str(value)
        .ok()
        .is_none_or(|id| id.get_version_num() != 7 || id.to_string() != value)
    {
        return Err(Status::invalid_argument(
            "command requires a canonical UUIDv7 ID",
        ));
    }
    Ok(())
}

impl ChatService for ChatServiceImpl {
    fn list_conversations(
        &self,
        _: Request<ListConversationsRequest>,
    ) -> RpcFuture<Response<ListConversationsResponse>> {
        let conversations = self.authority.conversations();
        Box::pin(async move { Ok(Response::new(ListConversationsResponse { conversations })) })
    }
    fn send_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> RpcFuture<Response<SendMessageResponse>> {
        let message = request.message;
        let result = (|| {
            validate_command_id(&message.command_id)?;
            let fact = self
                .authority
                .send(message.command_id, message.chat_id, message.text)?;
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
        let result = (|| {
            validate_command_id(&message.command_id)?;
            if message.pending_scope_id.is_empty() {
                return Err(Status::invalid_argument("pending scope is required"));
            }
            let (fact, snapshot) = self.authority.start(
                message.command_id,
                message.pending_scope_id,
                message.expected_revision,
                message.text,
            )?;
            Ok(Response::new(StartChatResponse {
                chat_id: fact.chat_id,
                messages: fact.messages,
                composer: Some(snapshot.into()),
            }))
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
    fn start_requires_canonical_identity_and_ids_cannot_cross_scopes_or_methods() {
        let service = service(Arc::new(ComposerAuthority::default()));
        let request = StartChatRequest {
            pending_scope_id: "one".into(),
            command_id: Uuid::now_v7().to_string(),
            expected_revision: 0,
            text: String::new(),
        };
        for id in [
            String::new(),
            "not-a-uuid".into(),
            request.command_id.replace('-', ""),
        ] {
            assert_eq!(
                block_on(service.start_chat(Request::new(StartChatRequest {
                    command_id: id,
                    ..request.clone()
                })))
                .unwrap_err()
                .code,
                Code::InvalidArgument
            );
        }
        let first = block_on(service.start_chat(Request::new(request.clone())))
            .unwrap()
            .message;
        assert_eq!(
            block_on(service.start_chat(Request::new(StartChatRequest {
                pending_scope_id: "other".into(),
                ..request.clone()
            })))
            .unwrap_err()
            .code,
            Code::AlreadyExists
        );
        assert_eq!(
            block_on(service.send_message(Request::new(SendMessageRequest {
                command_id: request.command_id,
                chat_id: first.chat_id,
                text: "again".into()
            })))
            .unwrap_err()
            .code,
            Code::AlreadyExists
        );
        assert_eq!(service.projection().conversations[0].messages.len(), 2);
    }

    #[test]
    fn first_send_atomically_promotes_pending_composer() {
        let composer = Arc::new(ComposerAuthority::default());
        composer
            .replace(ReplaceComposer {
                scope: ComposerScope::pending("pending"),
                command_id: "draft".into(),
                authority_epoch: 1,
                base_revision: 0,
                text: "hello".into(),
            })
            .unwrap();
        let client = ChatServiceClient::direct(Arc::new(service(Arc::clone(&composer))));

        let response = block_on(client.start_chat(Request::new(StartChatRequest {
            pending_scope_id: "pending".into(),
            command_id: "01900000-0000-7000-8000-000000000001".into(),
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
                .unwrap()
                .revision,
            2
        );
        assert_eq!(
            composer
                .snapshot(&ComposerScope::chat(response.chat_id))
                .unwrap()
                .revision,
            0
        );
    }

    #[test]
    fn retrying_a_start_command_returns_the_original_chat_and_messages() {
        let composer = Arc::new(ComposerAuthority::default());
        composer
            .replace(ReplaceComposer {
                scope: ComposerScope::pending("pending"),
                command_id: "draft".into(),
                authority_epoch: 1,
                base_revision: 0,
                text: "hello".into(),
            })
            .unwrap();
        let client = ChatServiceClient::direct(Arc::new(service(composer)));
        let request = StartChatRequest {
            pending_scope_id: "pending".into(),
            command_id: "01900000-0000-7000-8000-000000000001".into(),
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
