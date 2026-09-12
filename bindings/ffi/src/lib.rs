//! Foreign exports over shared scope projections. Hosts are supplied by roots.
pub use arut_feature_chat::composer::product::{ComposerState, ComposerStatus};
pub use arut_feature_chat::product::{ChatMessage, ChatRole, ChatState, ChatStatus};
use arut_feature_chat::{composer::product::ComposerClient, product::ChatClient};
use arut_product_session::ProductSession;
pub use arut_product_session::{ChatSummary, FeatureAvailability, SessionAvailability};
use arut_watch::Subscription;
use boltffi::{EventSubscription, export};
use std::sync::Arc;

pub struct ProductSessionHandle {
    session: Arc<ProductSession>,
}
pub struct ChatHandle {
    client: ChatClient,
}
pub struct ComposerHandle {
    client: ComposerClient,
}
pub struct ConversationsHandle {
    session: Arc<ProductSession>,
}
pub struct AvailabilityHandle {
    session: Arc<ProductSession>,
}

impl ProductSessionHandle {
    pub fn from_session(session: ProductSession) -> Self {
        Self {
            session: Arc::new(session),
        }
    }
}
#[export]
impl ProductSessionHandle {
    pub async fn initialize(&self) -> bool {
        self.session.initialize().await.is_ok()
    }
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
    pub fn select_chat(&self, id: String) -> Option<ChatHandle> {
        self.session
            .select_chat(&id)
            .map(|client| ChatHandle { client })
    }
    pub fn conversations(&self) -> ConversationsHandle {
        ConversationsHandle {
            session: self.session.clone(),
        }
    }
    pub fn availability(&self) -> AvailabilityHandle {
        AvailabilityHandle {
            session: self.session.clone(),
        }
    }
}
#[export]
impl ConversationsHandle {
    pub fn state(&self) -> Vec<ChatSummary> {
        self.session.chat_summaries()
    }
    #[ffi_stream(item = u64, mode = "callback")]
    pub fn list_changes(&self) -> Arc<EventSubscription<u64>> {
        ffi_subscription(self.session.conversations_changes())
    }
}
#[export]
impl AvailabilityHandle {
    pub fn state(&self) -> SessionAvailability {
        self.session.availability()
    }
    pub async fn refresh(&self) -> SessionAvailability {
        self.session.refresh_capabilities().await
    }
    #[ffi_stream(item = u64, mode = "callback")]
    pub fn availability_changes(&self) -> Arc<EventSubscription<u64>> {
        ffi_subscription(self.session.availability_changes())
    }
}
#[export]
impl ChatHandle {
    pub fn id(&self) -> String {
        self.client.id().unwrap_or_default()
    }
    pub fn state(&self) -> ChatState {
        self.client.state()
    }
    pub fn composer(&self) -> ComposerHandle {
        ComposerHandle {
            client: self.client.composer(),
        }
    }
    pub async fn send(&self, text: String) -> ChatState {
        self.client.send(text).await
    }
    #[ffi_stream(item = u64, mode = "callback")]
    pub fn chat_changes(&self) -> Arc<EventSubscription<u64>> {
        ffi_subscription(self.client.changes())
    }
}
#[export]
impl ComposerHandle {
    pub async fn follow(&self) {
        self.client.follow().await;
    }
    pub fn state(&self) -> ComposerState {
        self.client.state()
    }
    pub async fn initialize(&self) -> ComposerState {
        self.client.initialize().await
    }
    pub async fn replace(&self, text: String) -> ComposerState {
        self.client.replace(text).await
    }
    #[ffi_stream(item = u64, mode = "callback")]
    pub fn composer_changes(&self) -> Arc<EventSubscription<u64>> {
        ffi_subscription(self.client.changes())
    }
}
#[export]
pub fn create_product_session(pending_scope_id: String) -> ProductSessionHandle {
    ProductSessionHandle::from_session(ProductSession::local_with_pending_scope(pending_scope_id))
}
fn ffi_subscription(source: Arc<Subscription<u64>>) -> Arc<EventSubscription<u64>> {
    let target = Arc::new(EventSubscription::new(1));
    let weak = Arc::downgrade(&target);
    arut_observation::observe(source, move |revision| {
        let Some(target) = weak.upgrade() else {
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_projections_expose_conversations_drafts_and_send() {
        let session = create_product_session("test".into());
        let list = session.conversations();
        let chat = session.chat();
        let composer = chat.composer();
        futures_executor::block_on(composer.replace("hello".into()));
        assert_eq!(composer.state().text, "hello");
        assert_eq!(
            futures_executor::block_on(chat.send("hello".into()))
                .messages
                .len(),
            2
        );
        assert_eq!(list.state().len(), 1);
        assert_eq!(composer.state().text, "");
    }
}
