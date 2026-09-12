use arut_feature_chat::composer::product::{
    ComposerClient, ComposerState as ProductComposerState, ComposerStatus as ProductComposerStatus,
};
use arut_feature_chat::product::{
    ChatClient, ChatMessage as ProductChatMessage, ChatRole as ProductChatRole,
    ChatState as ProductChatState, ChatStatus as ProductChatStatus,
};
use arut_product_session::{
    ChatSummary as ProductChatSummary, FeatureAvailability as ProductFeatureAvailability,
    ProductSession, SessionAvailability as ProductSessionAvailability,
};
#[cfg(not(target_arch = "wasm32"))]
use arut_rpc::RpcChannel;
use arut_watch::Subscription;
use boltffi::{EventSubscription, data, export};
use std::sync::Arc;

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: u64,
    pub role: ChatRole,
    pub text: String,
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatStatus {
    Idle,
    Sending,
    Failed,
}

#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub status: ChatStatus,
    pub error: String,
}

pub struct ChatHandle {
    client: ChatClient,
}

pub struct ProductSessionHandle {
    session: ProductSession,
}

#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureAvailability {
    Unknown,
    Available,
    Unavailable,
}

#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAvailability {
    pub composer: FeatureAvailability,
    pub composer_unavailable_reason: String,
    pub negotiation_error: String,
}

#[export]
impl ProductSessionHandle {
    pub fn chat(&self) -> ChatHandle {
        ChatHandle {
            client: self.session.chat(),
        }
    }

    pub fn new_chat(&self) -> ChatHandle {
        ChatHandle {
            client: self.session.new_chat(),
        }
    }

    pub fn select_chat(&self, chat_id: String) -> ChatHandle {
        ChatHandle {
            client: self
                .session
                .select_chat(&chat_id)
                .unwrap_or_else(|| panic!("chat {chat_id} is not part of this session")),
        }
    }

    pub fn chat_ids(&self) -> Vec<String> {
        self.session.chat_ids()
    }

    pub fn chat_summaries(&self) -> Vec<ChatSummary> {
        self.session
            .chat_summaries()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub fn availability(&self) -> SessionAvailability {
        self.session.availability().into()
    }

    #[ffi_stream(item = u64, mode = "callback")]
    pub fn availability_changes(&self) -> Arc<EventSubscription<u64>> {
        ffi_subscription(self.session.availability_changes())
    }

    pub async fn refresh_capabilities(&self) -> SessionAvailability {
        self.session.refresh_capabilities().await.into()
    }
}

#[export]
impl ChatHandle {
    pub fn id(&self) -> String {
        self.client.id().unwrap_or_default()
    }

    pub fn composer(&self) -> ComposerHandle {
        ComposerHandle {
            client: self.client.composer(),
        }
    }

    pub fn state(&self) -> ChatState {
        self.client.state().into()
    }

    #[ffi_stream(item = u64, mode = "callback")]
    pub fn chat_changes(&self) -> Arc<EventSubscription<u64>> {
        ffi_subscription(self.client.changes())
    }

    pub async fn send(&self, text: String) -> ChatState {
        self.client.send(text).await.into()
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerStatus {
    Connecting,
    Synced,
    Failed,
}

#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerState {
    pub text: String,
    pub revision: u64,
    pub status: ComposerStatus,
    pub error: String,
}

pub struct ComposerHandle {
    client: ComposerClient,
}

#[export]
impl ComposerHandle {
    pub fn state(&self) -> ComposerState {
        self.client.state().into()
    }

    #[ffi_stream(item = u64, mode = "callback")]
    pub fn composer_changes(&self) -> Arc<EventSubscription<u64>> {
        ffi_subscription(self.client.changes())
    }

    pub async fn initialize(&self) -> ComposerState {
        self.client.initialize().await.into()
    }

    pub async fn replace(&self, text: String) -> ComposerState {
        self.client.replace(text).await.into()
    }

    pub async fn sync_once(&self) -> ComposerState {
        self.client.sync_once().await.into()
    }
}

#[export]
pub fn create_product_session(
    backend_url: String,
    pending_scope_id: String,
) -> ProductSessionHandle {
    assert!(
        !pending_scope_id.is_empty(),
        "pending scope ID must not be empty"
    );
    ProductSessionHandle {
        session: product_session(&backend_url, pending_scope_id),
    }
}

fn product_session(backend_url: &str, pending_scope_id: String) -> ProductSession {
    if backend_url.is_empty() {
        ProductSession::local_with_pending_scope(pending_scope_id)
    } else {
        remote_product_session(backend_url, pending_scope_id)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn remote_product_session(backend_url: &str, pending_scope_id: String) -> ProductSession {
    let channel: Arc<dyn RpcChannel> =
        Arc::new(arut_transport_http::HttpRpcChannel::new(backend_url));
    ProductSession::remote(channel, pending_scope_id)
}

#[cfg(target_arch = "wasm32")]
fn remote_product_session(_backend_url: &str, _pending_scope_id: String) -> ProductSession {
    panic!("browser RPC transport is not implemented")
}

fn ffi_subscription(source: Arc<Subscription<u64>>) -> Arc<EventSubscription<u64>> {
    let target = Arc::new(EventSubscription::new(64));
    let weak_target = Arc::downgrade(&target);
    arut_observation::observe(source, move |revision| {
        let Some(target) = weak_target.upgrade() else {
            return false;
        };
        if !target.is_active() {
            return false;
        }
        target.push_event(revision);
        true
    });
    target
}

impl From<ProductChatRole> for ChatRole {
    fn from(role: ProductChatRole) -> Self {
        match role {
            ProductChatRole::User => Self::User,
            ProductChatRole::Assistant => Self::Assistant,
        }
    }
}

impl From<ProductChatSummary> for ChatSummary {
    fn from(summary: ProductChatSummary) -> Self {
        Self {
            id: summary.id,
            title: summary.title,
        }
    }
}

impl From<ProductChatMessage> for ChatMessage {
    fn from(message: ProductChatMessage) -> Self {
        Self {
            id: message.id,
            role: message.role.into(),
            text: message.text,
        }
    }
}

impl From<ProductChatStatus> for ChatStatus {
    fn from(status: ProductChatStatus) -> Self {
        match status {
            ProductChatStatus::Idle => Self::Idle,
            ProductChatStatus::Sending => Self::Sending,
            ProductChatStatus::Failed => Self::Failed,
        }
    }
}

impl From<ProductChatState> for ChatState {
    fn from(state: ProductChatState) -> Self {
        Self {
            messages: state.messages.into_iter().map(Into::into).collect(),
            status: state.status.into(),
            error: state.error,
        }
    }
}

impl From<ProductComposerStatus> for ComposerStatus {
    fn from(status: ProductComposerStatus) -> Self {
        match status {
            ProductComposerStatus::Connecting => Self::Connecting,
            ProductComposerStatus::Synced => Self::Synced,
            ProductComposerStatus::Failed => Self::Failed,
        }
    }
}

impl From<ProductComposerState> for ComposerState {
    fn from(state: ProductComposerState) -> Self {
        Self {
            text: state.text,
            revision: state.revision,
            status: state.status.into(),
            error: state.error,
        }
    }
}

impl From<ProductSessionAvailability> for SessionAvailability {
    fn from(availability: ProductSessionAvailability) -> Self {
        let negotiation_error = availability.error;
        match availability.composer {
            ProductFeatureAvailability::Unknown => Self {
                composer: FeatureAvailability::Unknown,
                composer_unavailable_reason: String::new(),
                negotiation_error,
            },
            ProductFeatureAvailability::Available => Self {
                composer: FeatureAvailability::Available,
                composer_unavailable_reason: String::new(),
                negotiation_error,
            },
            ProductFeatureAvailability::Unavailable { reason } => Self {
                composer: FeatureAvailability::Unavailable,
                composer_unavailable_reason: reason,
                negotiation_error,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_feature_handles_from_one_session() {
        let session = create_product_session(String::new(), "test-owner".into());
        assert_eq!(
            futures_executor::block_on(session.chat().send("session".into())).messages[1].text,
            "You said: session"
        );
        assert!(!session.chat().id().is_empty());
        assert_eq!(session.chat_ids().len(), 1);
        assert_eq!(session.chat_summaries()[0].title, "session");
        assert_eq!(
            futures_executor::block_on(session.refresh_capabilities()).composer,
            FeatureAvailability::Available
        );
    }
}
