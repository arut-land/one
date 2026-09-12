use crate::composer::ComposerScope;
use crate::composer::product::{ComposerClient, ComposerStatus};
use arut_protocol::chat::composer::v1::ComposerServiceClient;
use arut_protocol::chat::v1::{
    ChatMessage as WireMessage, ChatRole as WireRole, ChatServiceClient, SendMessageRequest,
    StartChatRequest,
};
use arut_rpc::Request;
use arut_watch::{Subscription, Watch};
use futures_util::lock::Mutex as AsyncMutex;
use std::sync::Arc;
use uuid::Uuid;

#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChatRole {
    User,
    Assistant,
}

#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub id: u64,
    pub role: ChatRole,
    pub text: String,
}

#[boltffi::data]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChatStatus {
    #[default]
    Idle,
    Sending,
    Failed,
}

#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatState {
    pub id: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub status: ChatStatus,
    pub error: String,
}

#[doc(hidden)]
pub trait ChatStarted: Send + Sync {
    fn chat_started(&self, chat_id: String, chat: ChatClient);
}

#[derive(Clone)]
pub struct ChatClient {
    service: ChatServiceClient,
    composer: ComposerClient,
    state: Arc<Watch<ChatState>>,
    send_lock: Arc<AsyncMutex<()>>,
    pending_scope_id: Option<String>,
    on_started: Option<Arc<dyn ChatStarted>>,
    start_command_id: String,
}

impl ChatClient {
    pub fn established(
        service: ChatServiceClient,
        composer_service: ComposerServiceClient,
        id: String,
        messages: Vec<WireMessage>,
    ) -> Self {
        let mut client = Self::pending(service, composer_service.clone(), "unused".into(), None);
        client.composer = ComposerClient::new(composer_service, ComposerScope::chat(&id));
        client.state.set(ChatState {
            id: Some(id),
            messages: messages.into_iter().filter_map(from_wire).collect(),
            ..Default::default()
        });
        client
    }

    #[doc(hidden)]
    pub fn pending(
        service: ChatServiceClient,
        composer_service: ComposerServiceClient,
        pending_scope_id: String,
        on_started: Option<Arc<dyn ChatStarted>>,
    ) -> Self {
        Self {
            service,
            composer: ComposerClient::new(
                composer_service,
                ComposerScope::pending(pending_scope_id.clone()),
            ),
            state: Arc::new(Watch::new(ChatState::default())),
            send_lock: Arc::new(AsyncMutex::new(())),
            pending_scope_id: Some(pending_scope_id),
            on_started,
            start_command_id: Uuid::now_v7().to_string(),
        }
    }

    pub fn id(&self) -> Option<String> {
        self.state.get().id
    }

    pub fn composer(&self) -> ComposerClient {
        self.composer.clone()
    }

    pub fn state(&self) -> ChatState {
        self.state.get()
    }

    pub fn changes(&self) -> Arc<Subscription<u64>> {
        self.state.subscribe()
    }

    pub async fn send(&self, text: String) -> ChatState {
        let text = text.trim().to_owned();
        if text.is_empty() {
            return self.state.get();
        }
        let _send = self.send_lock.lock().await;
        let operations = self.composer.operations();
        let _composer_operation = operations.lock().await;
        self.state.update(|state| {
            state.status = ChatStatus::Sending;
            state.error.clear();
        });

        if let Some(chat_id) = self.id() {
            return self.send_established(chat_id, text).await;
        }
        self.start(text).await
    }

    async fn start(&self, text: String) -> ChatState {
        if self.composer.state().text != text {
            let composer = self.composer.replace_unlocked(text.clone()).await;
            if composer.status == ComposerStatus::Failed {
                return self.fail(composer.error);
            }
        }
        let composer = self.composer.state();
        let response = self
            .service
            .start_chat(Request::new(StartChatRequest {
                pending_scope_id: self
                    .pending_scope_id
                    .clone()
                    .expect("pending chat must have a pending scope"),
                command_id: self.start_command_id.clone(),
                expected_revision: composer.revision,
                text,
            }))
            .await;
        match response {
            Ok(response) => {
                let response = response.message;
                let chat_id = response.chat_id;
                if chat_id.is_empty() {
                    return self.fail("start chat omitted its chat ID".into());
                }
                let Some(snapshot) = response.composer else {
                    return self.fail("start chat omitted its composer snapshot".into());
                };
                if let Err(error) = self.composer.promote(&chat_id, snapshot) {
                    return self.fail(error);
                }
                let state = self.state.update(|state| {
                    state.id = Some(chat_id.clone());
                    state.messages = response
                        .messages
                        .into_iter()
                        .filter_map(from_wire)
                        .collect();
                    state.status = ChatStatus::Idle;
                    state.error.clear();
                });
                if let Some(callback) = &self.on_started {
                    callback.chat_started(chat_id, self.clone());
                }
                state
            }
            Err(error) => self.fail(error.to_string()),
        }
    }

    async fn send_established(&self, chat_id: String, text: String) -> ChatState {
        match self
            .service
            .send_message(Request::new(SendMessageRequest {
                chat_id,
                text,
                command_id: Uuid::now_v7().to_string(),
            }))
            .await
        {
            Ok(response) => {
                let cleared = self.composer.replace_unlocked(String::new()).await;
                if cleared.status == ComposerStatus::Failed {
                    return self.fail(cleared.error);
                }
                self.state.update(|state| {
                    state
                        .messages
                        .extend(response.message.messages.into_iter().filter_map(from_wire));
                    state.status = ChatStatus::Idle;
                    state.error.clear();
                })
            }
            Err(error) => self.fail(error.to_string()),
        }
    }

    fn fail(&self, error: String) -> ChatState {
        self.state.update(|state| {
            state.status = ChatStatus::Failed;
            state.error = error;
        })
    }
}

fn from_wire(message: WireMessage) -> Option<ChatMessage> {
    let role = match WireRole::try_from(message.role).ok()? {
        WireRole::User => ChatRole::User,
        WireRole::Assistant => ChatRole::Assistant,
        WireRole::Unspecified => return None,
    };
    Some(ChatMessage {
        id: message.id,
        role,
        text: message.text,
    })
}
