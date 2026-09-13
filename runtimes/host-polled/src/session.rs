//! In-memory composition retained behind the existing session factory ABI.
//! Native hosting completion in ROADMAP.md will replace these factory exports
//! with host injection. ProductSession itself only composes supplied services.
use arut_feature_chat::{
    ChatServiceImpl,
    composer::{ComposerAuthority, ComposerServiceImpl},
    ports::IdSource,
};
use arut_product_session::{ProductSession, SessionScope};
use arut_protocol::{
    capability::v1::CapabilityServiceClient,
    capability_manifest::CapabilityServiceImpl,
    chat::{
        composer::v1::{COMPOSER_SERVICE_DESCRIPTOR, ComposerServiceClient},
        v1::{CHAT_SERVICE_DESCRIPTOR, ChatServiceClient},
    },
};
use arut_rpc::{ServiceMetadata, ServiceRegistration};
use std::sync::Arc;
pub fn in_memory_session(pending_scope_id: String, ids: Arc<dyn IdSource>) -> ProductSession {
    let authority = Arc::new(ComposerAuthority::default());
    let chat = ChatServiceClient::direct(Arc::new(
        ChatServiceImpl::new(
            Arc::clone(&authority),
            Arc::new(arut_storage::MemoryLog::default()),
            ids.clone(),
        )
        .expect("empty local log"),
    ));
    let composer = ComposerServiceClient::direct(Arc::new(ComposerServiceImpl::new(authority)));
    let capabilities = CapabilityServiceClient::direct(Arc::new(CapabilityServiceImpl::new([
        ServiceRegistration::new(&CHAT_SERVICE_DESCRIPTOR, ServiceMetadata::default()),
        ServiceRegistration::new(&COMPOSER_SERVICE_DESCRIPTOR, ServiceMetadata::default()),
    ])));
    ProductSession::new(
        chat,
        composer,
        SessionScope {
            node_id: "local".into(),
            workspace_id: "default".into(),
            pending_scope_id,
        },
        ids,
    )
    .with_capability_service(capabilities)
}

pub fn native_session(pending_scope_id: String) -> ProductSession {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = pending_scope_id;
        panic!("a wasm host supplies its own IDs through create_browser_session")
    }
    #[cfg(not(target_arch = "wasm32"))]
    in_memory_session(pending_scope_id, Arc::new(crate::NativeIds))
}
