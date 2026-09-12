use crate::composer::ComposerScope;
use crate::composer::product::ComposerClient;
use crate::errors::{ChatError, ComposerError};
use crate::ports::IdSource;
use arut_protocol::chat::composer::v1::ComposerServiceClient;
use arut_protocol::chat::v1::{
    ChatMessage as WireMessage, ChatRole as WireRole, ChatServiceClient, SendMessageRequest,
    StartChatRequest,
};
use arut_rpc::{Cancellation, Request};
use arut_watch::{Subscription, Watch};
use futures_util::lock::Mutex as AsyncMutex;
use std::sync::Arc;

#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Set exactly when `status` is `Failed`; a surface reads the variant.
    pub error: Option<ChatError>,
}

/// The session learns here that a pending chat became a conversation.
pub trait ChatStarted: Send + Sync {
    fn chat_started(&self, chat_id: String, chat: ChatClient);
}

/// What a chat that has not started yet needs, and an established one does not.
#[derive(Clone)]
struct PendingStart {
    scope_id: String,
    command_id: String,
    on_started: Option<Arc<dyn ChatStarted>>,
}

#[derive(Clone)]
pub struct ChatClient {
    service: ChatServiceClient,
    composer: ComposerClient,
    state: Arc<Watch<ChatState>>,
    send_lock: Arc<AsyncMutex<()>>,
    start: Option<PendingStart>,
    ids: Arc<dyn IdSource>,
    cancellation: Arc<Cancellation>,
}

impl ChatClient {
    pub fn established(
        service: ChatServiceClient,
        composer_service: ComposerServiceClient,
        id: String,
        messages: Vec<WireMessage>,
        ids: Arc<dyn IdSource>,
        cancellation: Arc<Cancellation>,
    ) -> Self {
        Self {
            service,
            composer: ComposerClient::new(
                composer_service,
                ComposerScope::chat(&id),
                ids.clone(),
                cancellation.clone(),
            ),
            state: Arc::new(Watch::new(ChatState {
                id: Some(id),
                messages: messages.into_iter().filter_map(from_wire).collect(),
                ..Default::default()
            })),
            send_lock: Arc::new(AsyncMutex::new(())),
            start: None,
            ids,
            cancellation,
        }
    }

    pub fn pending(
        service: ChatServiceClient,
        composer_service: ComposerServiceClient,
        pending_scope_id: String,
        on_started: Option<Arc<dyn ChatStarted>>,
        ids: Arc<dyn IdSource>,
        cancellation: Arc<Cancellation>,
    ) -> Self {
        Self {
            service,
            composer: ComposerClient::new(
                composer_service,
                ComposerScope::pending(&pending_scope_id),
                ids.clone(),
                cancellation.clone(),
            ),
            state: Arc::new(Watch::new(ChatState::default())),
            send_lock: Arc::new(AsyncMutex::new(())),
            start: Some(PendingStart {
                scope_id: pending_scope_id,
                command_id: ids.new_id(),
                on_started,
            }),
            ids,
            cancellation,
        }
    }

    pub fn id(&self) -> Option<String> {
        self.state.read(|state| state.id.clone())
    }

    pub fn composer(&self) -> ComposerClient {
        self.composer.clone()
    }

    pub fn state(&self) -> ChatState {
        self.state.get()
    }

    /// Reads part of the state without cloning the transcript to get at it.
    pub fn read_state<R>(&self, read: impl FnOnce(&ChatState) -> R) -> R {
        self.state.read(read)
    }

    pub fn changes(&self) -> Arc<Subscription<u64>> {
        self.state.subscribe()
    }

    /// Stops when the conversation scope this chat belongs to is cancelled.
    pub fn cancellation(&self) -> &Arc<Cancellation> {
        &self.cancellation
    }

    pub async fn send(&self, text: String) -> ChatState {
        let text = text.trim().to_owned();
        if text.is_empty() {
            return self.state.get();
        }
        if self.cancellation.is_cancelled() {
            return self.fail(ChatError::Cancelled);
        }
        let _send = self.send_lock.lock().await;
        let operations = self.composer.operations();
        let _composer_operation = operations.lock().await;
        self.state.update(|state| {
            state.status = ChatStatus::Sending;
            state.error = None;
        });

        match (self.id(), &self.start) {
            (Some(chat_id), _) => self.send_established(chat_id, text).await,
            (None, Some(start)) => self.start(start, text).await,
            (None, None) => self.fail(ChatError::NoConversation),
        }
    }

    async fn start(&self, start: &PendingStart, text: String) -> ChatState {
        if self.composer.state().text != text {
            let composer = self.composer.replace_unlocked(text.clone()).await;
            if let Some(error) = composer.error {
                return self.fail(error.into());
            }
        }
        let composer = self.composer.state();
        let response = self
            .service
            .start_chat(Request::new(StartChatRequest {
                pending_scope_id: start.scope_id.clone(),
                command_id: start.command_id.clone(),
                expected_revision: composer.revision,
                text,
            }))
            .await;
        match response {
            Ok(response) => {
                let response = response.message;
                let chat_id = response.chat_id;
                if chat_id.is_empty() {
                    return self.fail(ChatError::ChatIdMissing);
                }
                let Some(snapshot) = response.composer else {
                    return self.fail(ComposerError::SnapshotMissing.into());
                };
                if let Err(error) = self.composer.promote(&chat_id, snapshot) {
                    return self.fail(error.into());
                }
                self.state.update(|state| {
                    state.id = Some(chat_id.clone());
                    state.messages = response
                        .messages
                        .into_iter()
                        .filter_map(from_wire)
                        .collect();
                    state.status = ChatStatus::Idle;
                    state.error = None;
                });
                if let Some(callback) = &start.on_started {
                    callback.chat_started(chat_id, self.clone());
                }
                self.state.get()
            }
            Err(error) => self.fail(error.into()),
        }
    }

    async fn send_established(&self, chat_id: String, text: String) -> ChatState {
        match self
            .service
            .send_message(Request::new(SendMessageRequest {
                chat_id,
                text,
                command_id: self.ids.new_id(),
            }))
            .await
        {
            Ok(response) => {
                let cleared = self.composer.replace_unlocked(String::new()).await;
                if let Some(error) = cleared.error {
                    return self.fail(error.into());
                }
                self.state.update(|state| {
                    state
                        .messages
                        .extend(response.message.messages.into_iter().filter_map(from_wire));
                    state.status = ChatStatus::Idle;
                    state.error = None;
                });
                self.state.get()
            }
            Err(error) => self.fail(error.into()),
        }
    }

    fn fail(&self, error: ChatError) -> ChatState {
        self.state.update(|state| {
            state.status = ChatStatus::Failed;
            state.error = Some(error);
        });
        self.state.get()
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
