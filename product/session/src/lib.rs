//! Sessions own typed scopes over feature clients supplied by composition roots.
//!
//! The session owns pending and established chats, conversation summaries, and
//! availability. A weak registration callback avoids a cycle between the registry
//! and its chats. Surfaces own selection and disposable observers.

mod failure;
pub mod hosting;
pub use failure::SessionError;
/// Feature handles and renderable projections exposed by a product session.
pub mod chat {
    pub use arut_feature_chat::composer::{ComposerClient, ComposerState, ComposerStatus};
    pub use arut_feature_chat::errors::{ChatError, ComposerError, NodeFailure};
    pub use arut_feature_chat::{
        ChatClient, ChatClients, ChatMessage, ChatRole, ChatState, ChatStatus,
    };
}
/// Observation contract shared by the session and its feature handles.
pub use arut_watch::Subscription;
pub mod scopes;
use arut_feature_chat::ports::IdSource;
use arut_feature_chat::{ChatClient, ChatClients, ChatStarted};
pub use arut_protocol::capability::v1::CapabilityServiceClient;
use arut_protocol::capability::v1::{GetCapabilitiesRequest, ServiceCapability};
pub use arut_protocol::capability_manifest::capability_client;
use arut_protocol::chat::composer::v1::COMPOSER_SERVICE_DESCRIPTOR;
use arut_rpc::{Cancellation, Request};
use arut_watch::Watch;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

/// Scope identities supplied by the composition root.
pub struct SessionScope {
    pub node_id: String,
    pub workspace_id: String,
    pub pending_scope_id: String,
}

pub struct ProductSession {
    workspace: Arc<scopes::Workspace<scopes::Services>>,
    chats: SessionChats,
    capability_service: CapabilityServiceClient,
    availability: Watch<SessionAvailability>,
}

type Established = Arc<Mutex<HashMap<String, ChatClient>>>;

struct SessionChats {
    ids: Arc<dyn IdSource>,
    established: Established,
    pending: Mutex<ChatClient>,
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
        established
            .lock()
            .expect("chat registry poisoned")
            .insert(chat_id.clone(), chat);
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
    }
}

fn title_of(chat: &ChatClient) -> String {
    chat.read_first_message(|message| {
        message
            .map(|message| {
                message
                    .text
                    .split_whitespace()
                    .flat_map(|word| std::iter::once(' ').chain(word.chars()))
                    .skip(1)
                    .take(48)
                    .collect()
            })
            .unwrap_or_default()
    })
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
    pub async fn initialize(&self) -> Result<(), SessionError> {
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

    pub fn new(
        clients: ChatClients,
        capabilities: CapabilityServiceClient,
        scope: SessionScope,
        ids: Arc<dyn IdSource>,
    ) -> Self {
        let ChatClients { chat, composer } = clients;
        let pending_scope_id = scope.pending_scope_id;
        assert!(
            !pending_scope_id.is_empty(),
            "pending scope ID must not be empty"
        );
        let runtime = Arc::new(scopes::Services { chat, composer });
        let workspace = scopes::Node::new(scope.node_id, runtime).workspace(scope.workspace_id);
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
        let chats = SessionChats {
            ids,
            conversations,
            established,
            pending: Mutex::new(pending),
            workspace: workspace.clone(),
            pending_scope_id,
        };

        Self {
            workspace,
            chats,
            capability_service: capabilities,
            availability: Watch::new(SessionAvailability {
                composer: FeatureAvailability::Unknown,
            }),
        }
    }

    pub fn pending_scope_id(&self) -> &str {
        &self.chats.pending_scope_id
    }

    /// The node scope every workspace, conversation, and operation hangs from.
    /// Cancelling it stops the whole session's outstanding work.
    pub fn cancellation(&self) -> &Cancellation {
        self.workspace.node().cancellation()
    }

    pub fn chat(&self) -> ChatClient {
        self.chats
            .pending
            .lock()
            .expect("pending chat poisoned")
            .clone()
    }

    pub fn new_chat(&self) -> ChatClient {
        let mut pending = self.chats.pending.lock().expect("pending chat poisoned");
        if pending.id().is_some() {
            *pending = self.chats.new_pending();
        }
        pending.clone()
    }

    pub fn select_chat(&self, chat_id: &str) -> Option<ChatClient> {
        self.chats.established(chat_id)
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
        let service = &self.capability_service;
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
    struct NativeIds;
    impl IdSource for NativeIds {
        fn new_id(&self) -> String {
            uuid::Uuid::now_v7().to_string()
        }
    }
    use futures_executor::block_on;

    fn session() -> ProductSession {
        let ids: Arc<dyn IdSource> = Arc::new(NativeIds);
        let feature = arut_feature_chat::compose(Arc::new(
            arut_feature_chat::test_support::MemoryPorts::new(ids.clone()),
        ))
        .unwrap();
        ProductSession::new(
            feature.clients(),
            capability_client(feature.registrations()),
            SessionScope {
                node_id: "local".into(),
                workspace_id: "default".into(),
                pending_scope_id: "account:one".into(),
            },
            ids,
        )
    }

    fn titles(session: &ProductSession) -> Vec<String> {
        session
            .chat_summaries()
            .into_iter()
            .map(|summary| summary.title)
            .collect()
    }

    #[test]
    fn a_published_summary_already_has_a_selectable_handle() {
        use std::{
            future::Future,
            sync::atomic::{AtomicBool, Ordering},
            task::{Context, Wake, Waker},
        };
        struct Observer {
            registry: Established,
            registered: AtomicBool,
        }
        impl Wake for Observer {
            fn wake(self: Arc<Self>) {
                self.registered
                    .store(!self.registry.lock().unwrap().is_empty(), Ordering::Relaxed);
            }
        }
        let session = session();
        let changes = session.conversations_changes();
        block_on(changes.changed());
        let observer = Arc::new(Observer {
            registry: session.chats.established.clone(),
            registered: AtomicBool::new(false),
        });
        let waker = Waker::from(observer.clone());
        let mut future = std::pin::pin!(changes.changed());
        assert!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        block_on(session.chat().send("published".into()));
        assert!(observer.registered.load(Ordering::Relaxed));
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
        assert_eq!(pending.messages_after(0).len(), 2);
        assert_eq!(pending.composer().state().text, "");
        let next = session.new_chat();
        assert_eq!(next.id(), None);
        assert_eq!(
            next.composer().scope(),
            ComposerScope::pending("account:one")
        );
    }

    #[test]
    fn transcript_reads_only_the_requested_key_range() {
        let session = session();
        let chat = session.chat();
        let first = block_on(chat.send("first".into()));
        let initial = chat.messages_after(0);
        assert_eq!(initial.len(), 2);
        assert_eq!(first.last_message_id, initial.last().unwrap().id);
        assert!(chat.messages_after(first.last_message_id).is_empty());
        let second = block_on(chat.send("second".into()));
        let added = chat.messages_after(first.last_message_id);
        assert_eq!(added.len(), 2);
        assert_eq!(added[0].text, "second");
        assert_eq!(second.last_message_id, added.last().unwrap().id);
        assert_eq!(&chat.messages_after(0)[..2], initial);
        assert!(chat.messages_after(u64::MAX).is_empty());
        block_on(chat.composer().replace("unfinished".into()));
        assert!(chat.messages_after(second.last_message_id).is_empty());
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
    fn a_refused_send_reaches_the_projection_as_a_typed_reason() {
        use arut_feature_chat::errors::{ChatError, NodeFailure};
        let session = session();
        let started = block_on(session.chat().send("first".into()));
        assert_eq!(started.error, None);

        // The node has no such conversation, so it answers NotFound; the
        // projection keeps the reason and drops the status message.
        let unknown = ChatClient::established(
            session.workspace.chat_service(),
            session.workspace.composer_service(),
            "no-such-conversation".into(),
            Vec::new(),
            Arc::new(NativeIds),
            session.workspace.conversation_cancellation(),
        );
        let failed = block_on(unknown.send("nowhere".into()));

        assert_eq!(failed.error, Some(ChatError::Node(NodeFailure::Missing)));
    }

    #[test]
    fn a_cancelled_conversation_scope_refuses_to_send() {
        use arut_feature_chat::errors::ChatError;
        let session = session();
        let chat = session.chat();
        chat.cancellation().cancel();

        let state = block_on(chat.send("after cancellation".into()));

        assert_eq!(state.error, Some(ChatError::Cancelled));
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
