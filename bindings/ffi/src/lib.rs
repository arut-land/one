//! # FFI
//!
//! Foreign exports over shared scope projections. Hosts are supplied by roots.
//!
//! One handle per scope, each an `#[export] impl` written out longhand. That
//! repetition is deliberate and must stay: BoltFFI's scanner reads this file
//! with `syn` and never expands macros, so a handle produced by a `macro_rules!`
//! compiles, links, and is simply absent from every generated binding --
//! `boltffi pack wasm --deny-skipped` reports success and skips nothing, because
//! it never saw the item to skip it. That was measured, not assumed. What can be
//! shared without hiding an export is shared: one `From` impl per handle for its
//! construction, and one `ffi_subscription` for the watch-to-event bridge every
//! scope repeats.
pub use arut_feature_chat::composer::product::{ComposerState, ComposerStatus};
pub use arut_feature_chat::errors::{ChatError, ComposerError, NodeFailure};
use arut_feature_chat::ports::IdSource;
pub use arut_feature_chat::product::{ChatMessage, ChatRole, ChatState, ChatStatus};
use arut_feature_chat::{composer::product::ComposerClient, product::ChatClient};
use arut_product_session::ProductSession;
pub use arut_product_session::{ChatSummary, FeatureAvailability, SessionAvailability};
mod observation;
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

/// Construction, once per handle, so the exported bodies stay one line each.
impl From<Arc<ProductSession>> for ConversationsHandle {
    fn from(session: Arc<ProductSession>) -> Self {
        Self { session }
    }
}
impl From<Arc<ProductSession>> for AvailabilityHandle {
    fn from(session: Arc<ProductSession>) -> Self {
        Self { session }
    }
}
impl From<ChatClient> for ChatHandle {
    fn from(client: ChatClient) -> Self {
        Self { client }
    }
}
impl From<ComposerClient> for ComposerHandle {
    fn from(client: ComposerClient) -> Self {
        Self { client }
    }
}

#[export]
impl ProductSessionHandle {
    pub async fn initialize(&self) -> bool {
        self.session.initialize().await.is_ok()
    }
    pub fn chat(&self) -> ChatHandle {
        self.session.chat().into()
    }
    pub fn new_chat(&self) -> ChatHandle {
        self.session.new_chat().into()
    }
    pub fn select_chat(&self, id: String) -> Option<ChatHandle> {
        self.session.select_chat(&id).map(Into::into)
    }
    pub fn conversations(&self) -> ConversationsHandle {
        self.session.clone().into()
    }
    pub fn availability(&self) -> AvailabilityHandle {
        self.session.clone().into()
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
        self.client.composer().into()
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
/// For hosts with a clock and entropy of their own. BoltFFI exports every
/// annotated item on every target, so wasm keeps the symbol and refuses it here
/// rather than compiling an ID source that cannot work; browsers call
/// `create_browser_session`.
#[export]
pub fn create_product_session(pending_scope_id: String) -> ProductSessionHandle {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = pending_scope_id;
        panic!("a wasm host supplies its own IDs through create_browser_session")
    }
    #[cfg(not(target_arch = "wasm32"))]
    session(
        pending_scope_id,
        Arc::new(arut_feature_chat::ports::NativeIds),
    )
}
fn ffi_subscription(source: Arc<Subscription<u64>>) -> Arc<EventSubscription<u64>> {
    let target = Arc::new(EventSubscription::new(1));
    let weak = Arc::downgrade(&target);
    observation::observe(source, move |revision| {
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
#[export]
pub trait HostIds: Send + Sync {
    fn new_id(&self) -> String;
}
struct BrowserIds(Arc<dyn HostIds>);
impl IdSource for BrowserIds {
    fn new_id(&self) -> String {
        self.0.new_id()
    }
}
/// Hosts without a clock or entropy, such as a browser, supply their own IDs.
#[export]
pub fn create_browser_session(
    pending_scope_id: String,
    ids: Arc<dyn HostIds>,
) -> ProductSessionHandle {
    session(pending_scope_id, Arc::new(BrowserIds(ids)))
}

fn session(pending_scope_id: String, ids: Arc<dyn IdSource>) -> ProductSessionHandle {
    ProductSessionHandle {
        session: Arc::new(ProductSession::local(pending_scope_id, ids)),
    }
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
