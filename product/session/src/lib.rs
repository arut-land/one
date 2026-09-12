pub mod hosting;
pub mod scopes;
use arut_feature_chat::composer::authority::ComposerAuthority;
use arut_feature_chat::composer::service::ComposerServiceImpl;
use arut_feature_chat::product::{ChatClient, ChatStarted};
use arut_feature_chat::service::ChatServiceImpl;
use arut_protocol::capability::v1::{
    CapabilityServiceClient, GetCapabilitiesRequest, ServiceCapability,
};
use arut_protocol::capability_manifest::CapabilityServiceImpl;
use arut_protocol::chat::composer::v1::{COMPOSER_SERVICE_DESCRIPTOR, ComposerServiceClient};
use arut_protocol::chat::v1::{CHAT_SERVICE_DESCRIPTOR, ChatServiceClient};
use arut_rpc::{Request, RpcChannel, ServiceMetadata, ServiceRegistration};
use arut_watch::{Subscription, Watch};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

pub struct ProductSession {
    pub workspace: Arc<scopes::Workspace<scopes::Services>>,
    chats: Arc<SessionChats>,
    capability_service: Option<CapabilityServiceClient>,
    availability: Arc<Watch<SessionAvailability>>,
}

struct SessionChats {
    state: Arc<Mutex<ChatRegistry>>,
    pending: Mutex<ChatClient>,
    current: Mutex<ChatClient>,
    chat_service: ChatServiceClient,
    composer_service: ComposerServiceClient,
    pending_scope_id: String,
}

#[derive(Default)]
struct ChatRegistry {
    established: HashMap<String, ChatClient>,
    order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
}

struct RegisterChat(Weak<Mutex<ChatRegistry>>);
impl ChatStarted for RegisterChat {
    fn chat_started(&self, chat_id: String, chat: ChatClient) {
        if let Some(registry) = self.0.upgrade() {
            let mut state = registry.lock().expect("chat registry poisoned");
            if !state.established.contains_key(&chat_id) {
                state.order.push(chat_id.clone());
            }
            state.established.insert(chat_id, chat);
        }
    }
}

impl SessionChats {
    fn pending(&self) -> ChatClient {
        ChatClient::pending(
            self.chat_service.clone(),
            self.composer_service.clone(),
            self.pending_scope_id.clone(),
            Some(Arc::new(RegisterChat(Arc::downgrade(&self.state)))),
        )
    }
    fn with<T>(&self, read: impl FnOnce(&ChatRegistry) -> T) -> T {
        read(&self.state.lock().expect("chat registry poisoned"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAvailability {
    pub composer: FeatureAvailability,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureAvailability {
    Unknown,
    Available,
    Unavailable { reason: String },
}

impl ProductSession {
    pub fn local() -> Self {
        Self::local_with_pending_scope("local-demo")
    }

    pub fn local_with_pending_scope(pending_scope_id: impl Into<String>) -> Self {
        let authority = Arc::new(ComposerAuthority::default());
        let chat =
            ChatServiceClient::direct(Arc::new(ChatServiceImpl::new(Arc::clone(&authority))));
        let composer = ComposerServiceClient::direct(Arc::new(ComposerServiceImpl::new(authority)));
        let capabilities = CapabilityServiceClient::direct(Arc::new(CapabilityServiceImpl::new([
            ServiceRegistration::new(&CHAT_SERVICE_DESCRIPTOR, ServiceMetadata::default()),
            ServiceRegistration::new(&COMPOSER_SERVICE_DESCRIPTOR, ServiceMetadata::default()),
        ])));
        Self::new(chat, composer, pending_scope_id).with_capability_service(capabilities)
    }

    pub fn remote(channel: Arc<dyn RpcChannel>, pending_scope_id: impl Into<String>) -> Self {
        Self::new(
            ChatServiceClient::remote(channel.clone()),
            ComposerServiceClient::remote(channel.clone()),
            pending_scope_id,
        )
        .with_capability_service(CapabilityServiceClient::remote(channel))
    }

    pub fn new(
        chat: ChatServiceClient,
        composer: ComposerServiceClient,
        pending_scope_id: impl Into<String>,
    ) -> Self {
        let pending_scope_id = pending_scope_id.into();
        assert!(
            !pending_scope_id.is_empty(),
            "pending scope ID must not be empty"
        );
        let runtime = Arc::new(scopes::Services {
            chat: chat.clone(),
            composer: composer.clone(),
        });
        let node = scopes::Node::new("local".into(), runtime, Default::default());
        let workspace = node.workspace("default".into(), Default::default());
        let state = Arc::new(Mutex::new(ChatRegistry::default()));
        let pending = ChatClient::pending(
            chat.clone(),
            composer.clone(),
            pending_scope_id.clone(),
            Some(Arc::new(RegisterChat(Arc::downgrade(&state)))),
        );
        let chats = Arc::new(SessionChats {
            state,
            pending: Mutex::new(pending.clone()),
            current: Mutex::new(pending),
            chat_service: chat,
            composer_service: composer,
            pending_scope_id,
        });

        Self {
            workspace,
            chats,
            capability_service: None,
            availability: Arc::new(Watch::new(SessionAvailability {
                composer: FeatureAvailability::Available,
                error: String::new(),
            })),
        }
    }

    pub fn pending_scope_id(&self) -> &str {
        &self.chats.pending_scope_id
    }

    pub fn with_capability_service(mut self, service: CapabilityServiceClient) -> Self {
        self.capability_service = Some(service);
        self.availability.set(SessionAvailability {
            composer: FeatureAvailability::Unknown,
            error: String::new(),
        });
        self
    }

    pub fn chat(&self) -> ChatClient {
        self.chats
            .current
            .lock()
            .expect("current chat poisoned")
            .clone()
    }

    pub fn new_chat(&self) -> ChatClient {
        let mut pending = self.chats.pending.lock().expect("pending chat poisoned");
        if pending.id().is_some() {
            *pending = self.chats.pending();
        }
        *self.chats.current.lock().expect("current chat poisoned") = pending.clone();
        pending.clone()
    }

    pub fn select_chat(&self, chat_id: &str) -> Option<ChatClient> {
        let chat = self.established_chat(chat_id)?;
        *self.chats.current.lock().expect("current chat poisoned") = chat.clone();
        Some(chat)
    }

    pub fn established_chat(&self, chat_id: &str) -> Option<ChatClient> {
        self.chats
            .with(|state| state.established.get(chat_id).cloned())
    }

    pub fn chat_ids(&self) -> Vec<String> {
        self.chats
            .with(|state| state.order.iter().rev().cloned().collect())
    }

    pub fn chat_summaries(&self) -> Vec<ChatSummary> {
        self.chats.with(|state| {
            state
                .order
                .iter()
                .rev()
                .filter_map(|id| {
                    let chat = state.established.get(id)?;
                    let chat_state = chat.state();
                    let text = chat_state
                        .messages
                        .first()?
                        .text
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    let mut title = text.chars().take(48).collect::<String>();
                    if text.chars().count() > 48 {
                        title.push_str("...");
                    }
                    Some(ChatSummary {
                        id: id.clone(),
                        title,
                    })
                })
                .collect()
        })
    }

    pub fn availability(&self) -> SessionAvailability {
        self.availability.get()
    }

    pub fn availability_changes(&self) -> Arc<Subscription<u64>> {
        self.availability.subscribe()
    }

    pub async fn refresh_capabilities(&self) -> SessionAvailability {
        let Some(service) = &self.capability_service else {
            return self.availability.get();
        };
        let (composer, error) = match service
            .get_capabilities(Request::new(GetCapabilitiesRequest {}))
            .await
        {
            Ok(response) => (
                response
                    .message
                    .manifest
                    .and_then(|manifest| manifest.services.into_iter().find(is_composer_service))
                    .map(feature_availability)
                    .unwrap_or_else(|| FeatureAvailability::Unavailable {
                        reason: "composer service is not advertised by this runtime".into(),
                    }),
                String::new(),
            ),
            Err(error) => (FeatureAvailability::Unknown, error.to_string()),
        };
        self.availability
            .set(SessionAvailability { composer, error })
    }
}

fn is_composer_service(service: &ServiceCapability) -> bool {
    service.package == COMPOSER_SERVICE_DESCRIPTOR.package
        && service.service == COMPOSER_SERVICE_DESCRIPTOR.name
}

fn feature_availability(service: ServiceCapability) -> FeatureAvailability {
    if service.available {
        FeatureAvailability::Available
    } else {
        FeatureAvailability::Unavailable {
            reason: service.unavailable_reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arut_feature_chat::composer::ComposerScope;
    use futures_executor::block_on;

    #[test]
    fn no_chat_exists_until_the_stable_pending_draft_is_sent() {
        let session = ProductSession::local_with_pending_scope("account:one");
        let pending = session.chat();
        assert_eq!(pending.id(), None);
        assert_eq!(
            pending.composer().scope(),
            ComposerScope::pending("account:one")
        );
        assert!(session.chat_ids().is_empty());

        block_on(pending.composer().replace("first".into()));
        let sent = block_on(pending.send("first".into()));
        let chat_id = sent.id.unwrap();

        assert_eq!(session.chat_ids(), vec![chat_id.clone()]);
        assert_eq!(session.chat_summaries()[0].title, "first");
        assert!(session.established_chat(&chat_id).is_some());
        assert_eq!(sent.messages.len(), 2);
        assert_eq!(pending.composer().state().text, "");
        let next = session.new_chat();
        assert_eq!(next.id(), None);
        assert_eq!(
            next.composer().scope(),
            ComposerScope::pending("account:one")
        );
    }

    #[test]
    fn chat_summaries_are_newest_first_and_safe_for_single_line_navigation() {
        let session = ProductSession::local_with_pending_scope("account:one");
        block_on(session.chat().send("first\n  conversation".into()));
        block_on(session.new_chat().send("second conversation".into()));

        let summaries = session.chat_summaries();
        assert_eq!(summaries[0].title, "second conversation");
        assert_eq!(summaries[1].title, "first conversation");
    }

    #[test]
    fn established_chats_keep_independent_drafts() {
        let session = ProductSession::local_with_pending_scope("account:one");
        let first = session.chat();
        let first_id = block_on(first.send("one".into())).id.unwrap();
        block_on(first.composer().replace("draft one".into()));

        let second = session.new_chat();
        let second_id = block_on(second.send("two".into())).id.unwrap();
        block_on(second.composer().replace("draft two".into()));

        assert_ne!(first_id, second_id);
        assert_eq!(
            session
                .chat_summaries()
                .into_iter()
                .map(|summary| summary.title)
                .collect::<Vec<_>>(),
            ["two", "one"]
        );
        assert_eq!(first.composer().state().text, "draft one");
        assert_eq!(second.composer().state().text, "draft two");
    }

    #[test]
    fn successful_established_send_clears_only_that_chats_draft() {
        let session = ProductSession::local_with_pending_scope("account:one");
        let first = session.chat();
        block_on(first.send("first".into()));
        let second = session.new_chat();
        block_on(second.send("second".into()));
        block_on(first.composer().replace("send me".into()));
        block_on(second.composer().replace("keep me".into()));

        block_on(first.send("send me".into()));

        assert_eq!(first.composer().state().text, "");
        assert_eq!(second.composer().state().text, "keep me");
    }
}
