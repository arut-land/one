//! Converts generated service requests and outcomes at the chat boundary.
#[cfg(test)]
use crate::ChatProjection;
use crate::authority::{ChatAuthority, CommitError};
use crate::command::Rejection;
use crate::composer::ComposerAuthority;
use crate::ports::{Clock, IdSource};
use arut_protocol::chat::v1::{
    ChatFact, ChatService, DeleteConversationRequest, DeleteConversationResponse,
    ListConversationsRequest, ListConversationsResponse, RenameConversationRequest,
    RenameConversationResponse, SendMessageRequest, SendMessageResponse, StartChatRequest,
    StartChatResponse, chat_fact,
};
use arut_rpc::{Code, Request, Response, RpcFuture, Status};
use arut_storage::{FactLog, StorageError};
use std::sync::Arc;

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
        let (code, message) = match error {
            CommitError::Storage(error) => return Self::new(Code::Internal, error.to_string()),
            CommitError::RevisionConflict => (Code::Aborted, "conversation revision changed"),
            CommitError::AuthorityChanged => {
                (Code::FailedPrecondition, "conversation authority changed")
            }
            CommitError::Superseded => (Code::Aborted, "command superseded"),
            CommitError::Rejected(Rejection::ConversationMissing) => {
                (Code::NotFound, "conversation not found")
            }
            CommitError::Rejected(Rejection::ConversationExists) => {
                (Code::AlreadyExists, "conversation already started")
            }
            CommitError::Rejected(Rejection::CommandConflict) => (
                Code::AlreadyExists,
                "command ID belongs to another operation",
            ),
            CommitError::Rejected(Rejection::PendingRevisionConflict) => {
                (Code::Aborted, "pending composer revision changed")
            }
            CommitError::Rejected(Rejection::PendingTextMismatch) => {
                (Code::FailedPrecondition, "pending composer text changed")
            }
        };
        Self::new(code, message)
    }
}

/// A generated method's answer, already decided, as the future it must return.
fn ready<T: Send + 'static>(result: Result<Response<T>, Status>) -> RpcFuture<Response<T>> {
    Box::pin(std::future::ready(result))
}

impl ChatService for ChatServiceImpl {
    fn list_conversations(
        &self,
        _: Request<ListConversationsRequest>,
    ) -> RpcFuture<Response<ListConversationsResponse>> {
        let conversations = self.authority.conversations();
        ready(Ok(Response::new(ListConversationsResponse {
            conversations,
        })))
    }
    fn send_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> RpcFuture<Response<SendMessageResponse>> {
        let message = request.message;
        ready((|| {
            let fact = self.authority.send(
                message.command_id.try_into()?,
                message.chat_id,
                message.text,
            )?;
            let Some(chat_fact::Change::Sent(sent)) = fact.change else {
                return Err(Status::new(
                    Code::Internal,
                    "send committed a non-message fact",
                ));
            };
            Ok(Response::new(SendMessageResponse {
                messages: sent.messages,
            }))
        })())
    }
    fn start_chat(
        &self,
        request: Request<StartChatRequest>,
    ) -> RpcFuture<Response<StartChatResponse>> {
        let message = request.message;
        ready((|| {
            if message.pending_scope_id.is_empty() {
                return Err(Status::invalid_argument("pending scope is required"));
            }
            let (fact, snapshot) = self.authority.start(
                message.command_id.try_into()?,
                message.pending_scope_id,
                message.expected_revision,
                message.text,
            )?;
            let Some(chat_fact::Change::Started(started)) = fact.change else {
                return Err(Status::new(
                    Code::Internal,
                    "start committed a non-message fact",
                ));
            };
            Ok(Response::new(StartChatResponse {
                chat_id: fact.chat_id,
                messages: started.messages,
                composer: Some(snapshot.into()),
            }))
        })())
    }
    fn rename_conversation(
        &self,
        request: Request<RenameConversationRequest>,
    ) -> RpcFuture<Response<RenameConversationResponse>> {
        let message = request.message;
        ready((|| {
            let fact = self.authority.rename(
                message.command_id.try_into()?,
                message.chat_id,
                message
                    .title
                    .try_into()
                    .map_err(|_| Status::invalid_argument("conversation title is empty"))?,
            )?;
            let Some(chat_fact::Change::Renamed(_)) = fact.change else {
                return Err(Status::new(
                    Code::Internal,
                    "rename committed a different fact",
                ));
            };
            Ok(Response::new(RenameConversationResponse {}))
        })())
    }
    fn delete_conversation(
        &self,
        request: Request<DeleteConversationRequest>,
    ) -> RpcFuture<Response<DeleteConversationResponse>> {
        let message = request.message;
        ready((|| {
            let _fact = self
                .authority
                .delete(message.command_id.try_into()?, message.chat_id)?;
            Ok(Response::new(DeleteConversationResponse {}))
        })())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::composer::{ComposerScope, ReplaceComposer};
    use crate::test_support::TestIds;
    use arut_protocol::chat::v1::ChatServiceClient;
    use arut_rpc::Request;
    use arut_storage::MemoryLog;
    use futures_executor::block_on;
    use uuid::Uuid;

    /// A composer holding `hello` in the pending scope at revision 1.
    fn pending_draft() -> Arc<ComposerAuthority> {
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
        composer
    }

    fn service(composer: Arc<ComposerAuthority>) -> ChatServiceImpl {
        ChatServiceImpl::new(composer, Arc::new(MemoryLog::default()), Arc::new(TestIds))
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
            block_on(service.start_chat(Request::new(StartChatRequest {
                text: "different".into(),
                ..request.clone()
            })))
            .unwrap_err()
            .code,
            Code::AlreadyExists,
            "a retry ID cannot name different start content"
        );
        assert_eq!(
            block_on(service.start_chat(Request::new(StartChatRequest {
                expected_revision: request.expected_revision + 1,
                ..request.clone()
            })))
            .unwrap_err()
            .code,
            Code::AlreadyExists,
            "a retry ID cannot name a different draft revision"
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
        let composer = pending_draft();
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
                .snapshot(&ComposerScope::chat(&response.chat_id))
                .unwrap()
                .revision,
            0
        );
    }

    #[test]
    fn conversation_mutations_are_durable_typed_and_retry_safe() {
        let service = service(pending_draft());
        let started = block_on(service.start_chat(Request::new(StartChatRequest {
            pending_scope_id: "pending".into(),
            command_id: Uuid::now_v7().to_string(),
            expected_revision: 1,
            text: "hello".into(),
        })))
        .unwrap()
        .message;
        let rename = RenameConversationRequest {
            chat_id: started.chat_id.clone(),
            title: "  Project   notes  ".into(),
            command_id: Uuid::now_v7().to_string(),
        };

        let renamed = block_on(service.rename_conversation(Request::new(rename.clone())))
            .unwrap()
            .message;
        assert_eq!(
            block_on(service.rename_conversation(Request::new(rename.clone())))
                .unwrap()
                .message,
            renamed,
            "an uncertain response can retry the same durable command"
        );
        assert_eq!(
            block_on(
                service.delete_conversation(Request::new(DeleteConversationRequest {
                    chat_id: started.chat_id.clone(),
                    command_id: rename.command_id,
                }))
            )
            .unwrap_err()
            .code,
            Code::AlreadyExists,
            "one command ID cannot cross mutation variants"
        );

        let delete = DeleteConversationRequest {
            chat_id: started.chat_id,
            command_id: Uuid::now_v7().to_string(),
        };
        block_on(service.delete_conversation(Request::new(delete.clone()))).unwrap();
        block_on(service.delete_conversation(Request::new(delete))).unwrap();
        assert!(service.projection().conversations.is_empty());
    }
}
