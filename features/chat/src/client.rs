//! Chat intent handling over the generated service contract.
use crate::ConversationTitle;
use crate::composer::ComposerClient;
use crate::composer::ComposerScope;
use crate::errors::{ChatError, ComposerError};
use crate::ports::IdSource;
use crate::projection::{ChatMessage, ChatRole, ChatState, ChatStatus, transcript_after};
use arut_protocol::chat::composer::v1::ComposerServiceClient;
use arut_protocol::chat::v1::{
    ChatMessage as WireMessage, ChatRole as WireRole, ChatServiceClient, SendMessageRequest,
    StartChatRequest,
};
use arut_rpc::{Cancellation, Request};
use arut_watch::{Subscription, Watch};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// The session projection that follows accepted transcript changes.
///
/// One callback for both the first accepted message and every later one: the
/// registry decides insert or update from the chat ID it already keys by.
pub trait ChatObserver: Send + Sync {
    fn chat_changed(&self, chat_id: &str, chat: &ChatClient);
}

/// What a chat that has not started yet needs, and an established one does not.
#[derive(Clone)]
struct PendingStart {
    scope_id: String,
    command_id: String,
}

/// Whether this handle was opened on a conversation the node already has, or on
/// a pending composer scope that one send will turn into a conversation.
#[derive(Clone)]
enum Stage {
    Pending(PendingStart),
    Established(String),
}

#[derive(Clone)]
pub struct ChatClient {
    service: ChatServiceClient,
    composer: ComposerClient,
    state: Arc<Watch<ChatState>>,
    messages: Arc<Mutex<BTreeMap<u64, ChatMessage>>>,
    /// The durable conversation title, when the person renamed it. `None` means
    /// the title is the first message, as the transcript defines it.
    title: Arc<Mutex<Option<ConversationTitle>>>,
    stage: Stage,
    observer: Option<Arc<dyn ChatObserver>>,
    pending_send: Arc<Mutex<Option<SendMessageRequest>>>,
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
        let messages: BTreeMap<_, _> = messages
            .into_iter()
            .filter_map(|message| ChatMessage::try_from(message).ok())
            .map(|message| (message.id, message))
            .collect();
        Self {
            service,
            composer: ComposerClient::new(
                composer_service,
                ComposerScope::chat(&id),
                ids.clone(),
                cancellation.clone(),
            ),
            state: Arc::new(Watch::new(derived(ChatState {
                last_message_id: messages.last_key_value().map_or(0, |(id, _)| *id),
                id: Some(id.clone()),
                ..Default::default()
            }))),
            messages: Arc::new(Mutex::new(messages)),
            title: Arc::default(),
            stage: Stage::Established(id),
            observer: None,
            pending_send: Arc::default(),
            ids,
            cancellation,
        }
    }

    pub fn pending(
        service: ChatServiceClient,
        composer_service: ComposerServiceClient,
        pending_scope_id: String,
        observer: Option<Arc<dyn ChatObserver>>,
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
            state: Arc::new(Watch::new(derived(ChatState::default()))),
            messages: Arc::default(),
            title: Arc::default(),
            pending_send: Arc::default(),
            stage: Stage::Pending(PendingStart {
                scope_id: pending_scope_id,
                command_id: ids.new_id(),
            }),
            observer,
            ids,
            cancellation,
        }
    }

    pub fn id(&self) -> Option<String> {
        self.state.read(|state| state.id.clone())
    }

    /// The durable title the person gave this conversation, when it has one.
    pub fn title(&self) -> Option<ConversationTitle> {
        self.title.lock().unwrap().clone()
    }

    /// Caches the authority-accepted title for summary projection.
    pub fn set_title(&self, title: ConversationTitle) {
        *self.title.lock().unwrap() = Some(title);
    }

    /// Attach the session projection before publishing an established client.
    pub fn with_observer(mut self, observer: Arc<dyn ChatObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    pub fn composer(&self) -> ComposerClient {
        self.composer.clone()
    }

    pub fn state(&self) -> ChatState {
        self.state.get()
    }

    /// Reads only newly accepted messages, in key order. A key never changes or
    /// disappears: operation streaming is separate from the durable transcript.
    pub fn messages_after(&self, after_id: u64) -> Vec<ChatMessage> {
        transcript_after(&self.messages.lock().unwrap(), after_id)
    }

    /// Reads first-message content without cloning it. Grouping flags are populated
    /// by `messages_after`, not by this content-only access used for chat titles.
    pub fn read_first_message<R>(&self, read: impl FnOnce(Option<&ChatMessage>) -> R) -> R {
        read(
            self.messages
                .lock()
                .unwrap()
                .first_key_value()
                .map(|(_, message)| message),
        )
    }

    /// Reads latest content without cloning the transcript.
    pub fn read_last_message<R>(&self, read: impl FnOnce(Option<&ChatMessage>) -> R) -> R {
        read(
            self.messages
                .lock()
                .unwrap()
                .last_key_value()
                .map(|(_, message)| message),
        )
    }

    fn accept_messages(&self, messages: Vec<WireMessage>) -> u64 {
        let mut stored = self.messages.lock().unwrap();
        for message in messages
            .into_iter()
            .filter_map(|message| ChatMessage::try_from(message).ok())
        {
            stored.entry(message.id).or_insert(message);
        }
        stored.last_key_value().map_or(0, |(id, _)| *id)
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
        // The composer is the draft this send commits, so every edit a person
        // made is written before the send reads its revision.
        self.composer.flush().await;
        let operations = self.composer.operations();
        let _composer_operation = operations.lock().await;
        if self.cancellation.is_cancelled() {
            return self.fail(ChatError::Cancelled);
        }
        self.write(|state| {
            state.status = ChatStatus::Sending;
            state.error = None;
        });

        match (&self.stage, self.id()) {
            (Stage::Established(chat_id), _) => self.send_established(chat_id.clone(), text).await,
            (Stage::Pending(_), Some(chat_id)) => self.send_established(chat_id, text).await,
            (Stage::Pending(start), None) => self.start(start, text).await,
        }
    }

    async fn start(&self, start: &PendingStart, text: String) -> ChatState {
        if self.composer.state().text != text {
            let composer = self.composer.write_locked(text.clone()).await;
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
                let last_message_id = self.accept_messages(response.messages);
                let state = self.write(|state| {
                    state.id = Some(chat_id.clone());
                    state.last_message_id = last_message_id;
                    state.status = ChatStatus::Idle;
                    state.error = None;
                });
                self.publish(&chat_id);
                state
            }
            Err(error) => self.fail(error.into()),
        }
    }

    async fn send_established(&self, chat_id: String, text: String) -> ChatState {
        let request = {
            let mut pending = self.pending_send.lock().unwrap();
            if pending
                .as_ref()
                .is_none_or(|request| request.chat_id != chat_id || request.text != text)
            {
                *pending = None;
            }
            pending
                .get_or_insert_with(|| SendMessageRequest {
                    chat_id: chat_id.clone(),
                    text,
                    command_id: self.ids.new_id(),
                })
                .clone()
        };
        match self.service.send_message(Request::new(request)).await {
            Ok(response) => {
                self.pending_send.lock().unwrap().take();
                let last_message_id = self.accept_messages(response.message.messages);
                self.write(|state| {
                    state.last_message_id = last_message_id;
                    state.status = ChatStatus::Idle;
                    state.error = None;
                });
                self.publish(&chat_id);
                // Draft cleanup has its own error state. Acceptance is already durable.
                self.composer.write_locked(String::new()).await;
                self.state.get()
            }
            Err(error) => self.fail(error.into()),
        }
    }

    fn publish(&self, chat_id: &str) {
        if let Some(observer) = &self.observer {
            observer.chat_changed(chat_id, self);
        }
    }

    /// Edits the projection and recomputes what a surface derives from it.
    fn write(&self, edit: impl FnOnce(&mut ChatState)) -> ChatState {
        let cancelled = self.cancellation.is_cancelled();
        self.state.update(|state| {
            edit(state);
            state.derive(cancelled);
        });
        self.state.get()
    }

    fn fail(&self, error: ChatError) -> ChatState {
        self.write(|state| {
            state.status = ChatStatus::Failed;
            state.error = Some(error);
        })
    }
}

/// A projection with its derived fields already computed, for a fresh handle.
fn derived(mut state: ChatState) -> ChatState {
    state.derive(false);
    state
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidChatRole;

impl TryFrom<WireMessage> for ChatMessage {
    type Error = InvalidChatRole;

    fn try_from(message: WireMessage) -> Result<Self, Self::Error> {
        let role = match WireRole::try_from(message.role).map_err(|_| InvalidChatRole)? {
            WireRole::User => ChatRole::User,
            WireRole::Assistant => ChatRole::Assistant,
            WireRole::Unspecified => return Err(InvalidChatRole),
        };
        Ok(Self {
            id: message.id,
            role,
            text: message.text,
            accepted_at_ms: message.accepted_at_ms,
            starts_time_group: false,
            previous_time_group_at_ms: None,
            starts_speaker_group: false,
            ends_speaker_group: false,
        })
    }
}
