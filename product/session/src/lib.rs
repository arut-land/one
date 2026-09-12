//! # Session
//!
//! Typed Node and Workspace scopes hold their parents, and ChatRuntime is the
//! compile-time capability bundle a workspace needs before it can reach chat.
//! A session constructs its default node and workspace and reads its service
//! clients through them; configuration falls back from workspace to node.
//!
//! One conversation registry holds the established chats and one watch holds the
//! ordered summaries a surface renders. Registration arrives through a weak
//! callback, so an established chat may hold the client that registered it.

pub mod hosting;
pub mod scopes;
use arut_feature_chat::composer::authority::ComposerAuthority;
use arut_feature_chat::composer::service::ComposerServiceImpl;
use arut_feature_chat::ports::IdSource;
use arut_feature_chat::product::{ChatClient, ChatStarted};
use arut_feature_chat::service::ChatServiceImpl;
use arut_protocol::capability::v1::{
    CapabilityServiceClient, GetCapabilitiesRequest, ServiceCapability,
};
use arut_protocol::capability_manifest::CapabilityServiceImpl;
use arut_protocol::chat::composer::v1::{COMPOSER_SERVICE_DESCRIPTOR, ComposerServiceClient};
use arut_protocol::chat::v1::{CHAT_SERVICE_DESCRIPTOR, ChatServiceClient};
use arut_rpc::{Cancellation, Request, RpcChannel, ServiceMetadata, ServiceRegistration};
use arut_watch::{Subscription, Watch};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

pub struct ProductSession {
    pub workspace: Arc<scopes::Workspace<scopes::Services>>,
    chats: Arc<SessionChats>,
    capability_service: Option<CapabilityServiceClient>,
    availability: Arc<Watch<SessionAvailability>>,
}

type Established = Arc<Mutex<HashMap<String, ChatClient>>>;

struct SessionChats {
    ids: Arc<dyn IdSource>,
    established: Established,
    pending: Mutex<ChatClient>,
    current: Mutex<ChatClient>,
    workspace: Arc<scopes::Workspace<scopes::Services>>,
    pending_scope_id: String,
    conversations: Arc<Watch<Vec<ChatSummary>>>,
}

#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
}

/// Registration is weak: an established chat holds the callback that registered it.
struct RegisterChat {
    established: Weak<Mutex<HashMap<String, ChatClient>>>,
    conversations: Arc<Watch<Vec<ChatSummary>>>,
}
impl RegisterChat {
    fn new(established: &Established, conversations: &Arc<Watch<Vec<ChatSummary>>>) -> Arc<Self> {
        Arc::new(Self {
            established: Arc::downgrade(established),
            conversations: conversations.clone(),
        })
    }
}
impl ChatStarted for RegisterChat {
    fn chat_started(&self, chat_id: String, chat: ChatClient) {
        let Some(established) = self.established.upgrade() else {
            return;
        };
        let title = title_of(&chat);
        self.conversations.update(|summaries| {
            if !summaries.iter().any(|summary| summary.id == chat_id) {
                summaries.insert(
                    0,
                    ChatSummary {
                        id: chat_id.clone(),
                        title,
                    },
                );
            }
        });
        established
            .lock()
            .expect("chat registry poisoned")
            .insert(chat_id, chat);
    }
}

fn title_of(chat: &ChatClient) -> String {
    chat.state()
        .messages
        .first()
        .map(|message| {
            message
                .text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(48)
                .collect()
        })
        .unwrap_or_default()
}

impl SessionChats {
    fn registration(&self) -> Arc<RegisterChat> {
        RegisterChat::new(&self.established, &self.conversations)
    }
    fn new_pending(&self) -> ChatClient {
        ChatClient::pending(
            self.workspace.chat_service(),
            self.workspace.composer_service(),
            self.pending_scope_id.clone(),
            Some(self.registration()),
            self.ids.clone(),
            self.workspace.conversation_cancellation(),
        )
    }
    fn established(&self, chat_id: &str) -> Option<ChatClient> {
        self.established
            .lock()
            .expect("chat registry poisoned")
            .get(chat_id)
            .cloned()
    }
}

#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionAvailability {
    pub composer: FeatureAvailability,
}

/// Typed availability with a typed reason; surfaces own every word of it.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureAvailability {
    /// No manifest has been read yet.
    Unknown,
    Available,
    /// The node advertises the service but reports it as unavailable now.
    ReportedUnavailable,
    /// The node's manifest does not carry the service.
    NotAdvertised,
    /// The manifest could not be read from the node.
    ManifestUnreachable,
}

impl ProductSession {
    pub async fn initialize(&self) -> Result<(), arut_rpc::Status> {
        let response = self
            .chats
            .workspace
            .chat_service()
            .list_conversations(Request::new(
                arut_protocol::chat::v1::ListConversationsRequest {},
            ))
            .await?;
        let registration = self.chats.registration();
        for conversation in response.message.conversations {
            if self.chats.established(&conversation.id).is_some() {
                continue;
            }
            let client = ChatClient::established(
                self.chats.workspace.chat_service(),
                self.chats.workspace.composer_service(),
                conversation.id.clone(),
                conversation.messages,
                self.chats.ids.clone(),
                self.chats.workspace.conversation_cancellation(),
            );
            registration.chat_started(conversation.id, client);
        }
        Ok(())
    }

    /// Hosts every service in this process; the composition root supplies IDs.
    pub fn local(pending_scope_id: impl Into<String>, ids: Arc<dyn IdSource>) -> Self {
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
        Self::new(chat, composer, pending_scope_id, ids).with_capability_service(capabilities)
    }

    pub fn remote(
        channel: Arc<dyn RpcChannel>,
        pending_scope_id: impl Into<String>,
        ids: Arc<dyn IdSource>,
    ) -> Self {
        Self::new(
            ChatServiceClient::remote(channel.clone()),
            ComposerServiceClient::remote(channel.clone()),
            pending_scope_id,
            ids,
        )
        .with_capability_service(CapabilityServiceClient::remote(channel))
    }

    pub fn new(
        chat: ChatServiceClient,
        composer: ComposerServiceClient,
        pending_scope_id: impl Into<String>,
        ids: Arc<dyn IdSource>,
    ) -> Self {
        let pending_scope_id = pending_scope_id.into();
        assert!(
            !pending_scope_id.is_empty(),
            "pending scope ID must not be empty"
        );
        let runtime = Arc::new(scopes::Services { chat, composer });
        let workspace = scopes::Node::new("local".into(), runtime).workspace("default".into());
        let conversations = Arc::new(Watch::new(Vec::new()));
        let established = Established::default();
        let pending = ChatClient::pending(
            workspace.chat_service(),
            workspace.composer_service(),
            pending_scope_id.clone(),
            Some(RegisterChat::new(&established, &conversations)),
            ids.clone(),
            workspace.conversation_cancellation(),
        );
        let chats = Arc::new(SessionChats {
            ids,
            conversations,
            established,
            pending: Mutex::new(pending.clone()),
            current: Mutex::new(pending),
            workspace: workspace.clone(),
            pending_scope_id,
        });

        Self {
            workspace,
            chats,
            capability_service: None,
            availability: Arc::new(Watch::new(SessionAvailability {
                composer: FeatureAvailability::Available,
            })),
        }
    }

    pub fn pending_scope_id(&self) -> &str {
        &self.chats.pending_scope_id
    }

    /// The node scope every workspace, conversation, and operation hangs from.
    /// Cancelling it stops the whole session's outstanding work.
    pub fn cancellation(&self) -> &Cancellation {
        self.workspace.node.cancellation()
    }

    fn with_capability_service(mut self, service: CapabilityServiceClient) -> Self {
        self.capability_service = Some(service);
        self.availability.set(SessionAvailability {
            composer: FeatureAvailability::Unknown,
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
            *pending = self.chats.new_pending();
        }
        *self.chats.current.lock().expect("current chat poisoned") = pending.clone();
        pending.clone()
    }

    pub fn select_chat(&self, chat_id: &str) -> Option<ChatClient> {
        let chat = self.chats.established(chat_id)?;
        *self.chats.current.lock().expect("current chat poisoned") = chat.clone();
        Some(chat)
    }

    /// The conversation list a surface renders, newest first.
    pub fn chat_summaries(&self) -> Vec<ChatSummary> {
        self.chats.conversations.get()
    }

    pub fn conversations_changes(&self) -> Arc<Subscription<u64>> {
        self.chats.conversations.subscribe()
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
        let composer = match service
            .get_capabilities(Request::new(GetCapabilitiesRequest {}))
            .await
        {
            Ok(response) => response
                .message
                .manifest
                .and_then(|manifest| manifest.services.into_iter().find(is_composer_service))
                .map_or(FeatureAvailability::NotAdvertised, feature_availability),
            Err(_) => FeatureAvailability::ManifestUnreachable,
        };
        self.availability.set(SessionAvailability { composer })
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
        FeatureAvailability::ReportedUnavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arut_feature_chat::composer::ComposerScope;
    use arut_feature_chat::ports::NativeIds;
    use futures_executor::block_on;

    fn session() -> ProductSession {
        ProductSession::local("account:one", Arc::new(NativeIds))
    }

    fn titles(session: &ProductSession) -> Vec<String> {
        session
            .chat_summaries()
            .into_iter()
            .map(|summary| summary.title)
            .collect()
    }

    #[test]
    fn no_chat_exists_until_the_stable_pending_draft_is_sent() {
        let session = session();
        let pending = session.chat();
        assert_eq!(pending.id(), None);
        assert_eq!(
            pending.composer().scope(),
            ComposerScope::pending("account:one")
        );
        assert!(session.chat_summaries().is_empty());

        block_on(pending.composer().replace("first".into()));
        let sent = block_on(pending.send("first".into()));
        let chat_id = sent.id.unwrap();

        assert_eq!(titles(&session), ["first"]);
        assert!(session.select_chat(&chat_id).is_some());
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
        let session = session();
        block_on(session.chat().send("first\n  conversation".into()));
        block_on(session.new_chat().send("second conversation".into()));

        assert_eq!(
            titles(&session),
            ["second conversation", "first conversation"]
        );
    }

    #[test]
    fn established_chats_keep_independent_drafts() {
        let session = session();
        let first = session.chat();
        let first_id = block_on(first.send("one".into())).id.unwrap();
        block_on(first.composer().replace("draft one".into()));

        let second = session.new_chat();
        let second_id = block_on(second.send("two".into())).id.unwrap();
        block_on(second.composer().replace("draft two".into()));

        assert_ne!(first_id, second_id);
        assert_eq!(titles(&session), ["two", "one"]);
        assert_eq!(first.composer().state().text, "draft one");
        assert_eq!(second.composer().state().text, "draft two");
    }

    #[test]
    fn successful_established_send_clears_only_that_chats_draft() {
        let session = session();
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
