use crate::composer::authority::{ComposerAuthority, PromoteError};
use crate::domain;
use arut_protocol::chat::composer::v1::ComposerSnapshot as WireComposerSnapshot;
use arut_protocol::chat::v1::{
    ChatMessage, ChatRole, ChatService, SendMessageRequest, SendMessageResponse, StartChatRequest,
    StartChatResponse,
};
use arut_rpc::{Code, Request, Response, RpcFuture, Status};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Default)]
pub struct ChatServiceImpl {
    composer: Arc<ComposerAuthority>,
    chats: Mutex<HashMap<String, ChatRecord>>,
    starts: Mutex<HashMap<(String, String), StartChatResponse>>,
    start_lock: Mutex<()>,
}

#[derive(Default)]
struct ChatRecord {
    messages: Vec<ChatMessage>,
    next_message_id: u64,
}

impl ChatRecord {
    fn append_exchange(&mut self, text: String) -> Vec<ChatMessage> {
        let user = ChatMessage {
            id: self.next_id(),
            role: ChatRole::User as i32,
            text: text.clone(),
        };
        let assistant = ChatMessage {
            id: self.next_id(),
            role: ChatRole::Assistant as i32,
            text: domain::respond(&text),
        };
        self.messages.extend([user.clone(), assistant.clone()]);
        vec![user, assistant]
    }

    fn next_id(&mut self) -> u64 {
        self.next_message_id += 1;
        self.next_message_id
    }
}

impl ChatServiceImpl {
    pub fn new(composer: Arc<ComposerAuthority>) -> Self {
        Self {
            composer,
            chats: Mutex::new(HashMap::new()),
            starts: Mutex::new(HashMap::new()),
            start_lock: Mutex::new(()),
        }
    }

    pub fn composer_authority(&self) -> Arc<ComposerAuthority> {
        Arc::clone(&self.composer)
    }
}

impl ChatService for ChatServiceImpl {
    fn send_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> RpcFuture<Response<SendMessageResponse>> {
        let message = request.message;
        let result = self
            .chats
            .lock()
            .expect("chat authority lock poisoned")
            .get_mut(&message.chat_id)
            .map(|chat| chat.append_exchange(message.text));
        Box::pin(async move {
            let messages = result.ok_or_else(|| Status::new(Code::NotFound, "chat not found"))?;
            Ok(Response::new(SendMessageResponse { messages }))
        })
    }

    fn start_chat(
        &self,
        request: Request<StartChatRequest>,
    ) -> RpcFuture<Response<StartChatResponse>> {
        let message = request.message;
        let _start = self.start_lock.lock().expect("start chat lock poisoned");
        let key = (message.pending_scope_id.clone(), message.command_id.clone());
        if let Some(response) = self
            .starts
            .lock()
            .expect("start command lock poisoned")
            .get(&key)
            .cloned()
        {
            return Box::pin(async move { Ok(Response::new(response)) });
        }
        let chat_id = Uuid::new_v4().to_string();
        let text = message.text.clone();
        let result = self.composer.promote_pending(
            &message.pending_scope_id,
            message.expected_revision,
            &message.text,
            &chat_id,
            || {
                let mut chats = self.chats.lock().expect("chat authority lock poisoned");
                let mut chat = ChatRecord::default();
                let messages = chat.append_exchange(text);
                chats.insert(chat_id.clone(), chat);
                Ok::<_, core::convert::Infallible>(messages)
            },
        );
        let response = match result.expect("infallible chat commit failed") {
            Ok((messages, composer)) => {
                let response = StartChatResponse {
                    chat_id: chat_id.clone(),
                    messages,
                    composer: Some(WireComposerSnapshot::from(composer)),
                };
                self.starts
                    .lock()
                    .expect("start command lock poisoned")
                    .insert(key, response.clone());
                Ok(Response::new(response))
            }
            Err(PromoteError::RevisionConflict(_)) => Err(Status::new(
                Code::Aborted,
                "pending composer revision changed",
            )),
            Err(PromoteError::TextMismatch(_)) => Err(Status::new(
                Code::FailedPrecondition,
                "pending composer text does not match",
            )),
        };
        Box::pin(async move { response })
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
