//! What a person does, as a second `#[export] impl` block per scope handle.
//!
//! Rust allows several inherent impls on one type and BoltFFI's scanner reads
//! each of them as source, so the repeated half of a handle is generated
//! (`src/generated/handles.rs`) and this half stays visible and reviewable.

use crate::generated::handles::{
    AvailabilityHandle, ChatHandle, ComposerHandle, ConversationsHandle, ProductSessionHandle,
};
use crate::{BrowserIds, Features, HostIds, SessionAvailability};
use arut_feature_chat::ChatState;
use arut_feature_chat::composer::ComposerState;
use arut_feature_chat::ports::{Clock, IdSource};
use arut_product_session::ProductSession;
use arut_product_session::feature::ComposeSet;
use boltffi::export;
use std::sync::Arc;

#[export]
impl ProductSessionHandle {
    /// Load the conversations the node already holds. `false` means the node
    /// did not answer; the projections stay as they were.
    pub async fn initialize(&self) -> bool {
        self.inner.initialize().await.is_ok()
    }
    /// A pending conversation, replacing the previous one only once it has
    /// been accepted and named.
    pub fn new_chat(&self) -> ChatHandle {
        ChatHandle {
            inner: self.inner.new_chat(),
        }
    }
    pub fn select_chat(&self, id: String) -> Option<ChatHandle> {
        self.inner
            .select_chat(&id)
            .map(|inner| ChatHandle { inner })
    }
}

#[export]
impl ConversationsHandle {
    /// Record which conversation this session is showing. Selecting one also
    /// marks it read.
    pub fn select(&self, id: Option<String>) {
        self.inner.select(id);
    }
    /// Narrow the list. One casing rule for every surface; `state()` returns
    /// what matches and where.
    pub fn set_query(&self, query: String) {
        self.inner.set_query(query);
    }
    /// Rename one conversation durably. `false` means the node did not accept it.
    pub async fn rename(&self, chat_id: String, title: String) -> bool {
        self.inner.rename(chat_id, title).await
    }
    /// Delete one conversation durably. `false` means the node did not accept it.
    pub async fn delete(&self, chat_id: String) -> bool {
        self.inner.delete(chat_id).await
    }
}

#[export]
impl AvailabilityHandle {
    pub async fn refresh(&self) -> SessionAvailability {
        self.inner.refresh_capabilities().await
    }
}

#[export]
impl ChatHandle {
    pub async fn send(&self, text: String) -> ChatState {
        self.inner.send(text).await
    }
}

#[export]
impl ComposerHandle {
    pub async fn follow(&self) {
        self.inner.follow().await;
    }
    pub async fn initialize(&self) -> ComposerState {
        self.inner.initialize().await
    }
    pub async fn replace(&self, text: String) -> ComposerState {
        self.inner.replace(text).await
    }
}

/// For hosts with a clock and entropy of their own. BoltFFI exports every
/// annotated item on every target, so wasm keeps the symbol and refuses it here
/// rather than compiling an ID source that cannot work; browsers call
/// `create_browser_session`.
#[export]
pub fn create_product_session(pending_scope_id: String) -> ProductSessionHandle {
    #[cfg(not(target_arch = "wasm32"))]
    {
        session(
            pending_scope_id,
            Arc::new(arut_runtime_host_polled::NativeIds),
            Arc::new(arut_runtime_host_polled::NativeClock),
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = pending_scope_id;
        panic!("a wasm host supplies time and IDs through create_browser_session")
    }
}

/// Browser hosts supply wall time and UUIDv7 identities.
#[export]
pub fn create_browser_session(
    pending_scope_id: String,
    ids: Arc<dyn HostIds>,
) -> ProductSessionHandle {
    let host = Arc::new(BrowserIds(ids));
    session(pending_scope_id, host.clone(), host)
}

fn session(
    pending_scope_id: String,
    ids: Arc<dyn IdSource>,
    clock: Arc<dyn Clock + Send + Sync>,
) -> ProductSessionHandle {
    let runtime = Arc::new(arut_runtime_host_polled::MemoryRuntime::new(
        ids.clone(),
        clock,
    ));
    let composed = <Features as ComposeSet<_>>::compose(&runtime).expect("compose memory features");
    ProductSessionHandle {
        inner: Arc::new(ProductSession::local(
            composed.clients,
            arut_product_session::SessionScope {
                node_id: "local".into(),
                workspace_id: "default".into(),
                pending_scope_id,
            },
            ids,
        )),
    }
}
