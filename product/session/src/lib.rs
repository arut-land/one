//! Sessions own typed scopes over feature clients supplied by composition roots.
//!
//! The session owns pending and established chats, the conversation list with
//! its selection, and availability. A weak registration callback avoids a cycle
//! between the registry and its chats. Local feature and remote channel
//! constructors assemble the required capability client.

mod availability;
pub use availability::{FeatureAvailability, SessionAvailability};
pub mod hosting;
/// Why a session-level call produced no usable answer.
///
/// The same reasons a feature scope reports, because they are the same node
/// answering; kept under this name while surfaces move to `NodeFailure`.
pub use arut_feature_chat::errors::NodeFailure as SessionError;
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
use arut_feature_chat::{ChatClient, ChatClients, ChatObserver};
use arut_protocol::capability::v1::CapabilityServiceClient;
use arut_protocol::capability_manifest::capability_client;
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
    pending: Mutex<ChatClient>,
    ids: Arc<dyn IdSource>,
    established: Established,
    workspace: Arc<scopes::Workspace<ChatClients>>,
    pending_scope_id: String,
    conversations: Arc<Watch<Conversations>>,
    capability_service: CapabilityServiceClient,
    availability: Watch<SessionAvailability>,
}

type Established = Arc<Mutex<HashMap<String, ChatClient>>>;

/// The conversation list a surface renders, newest first, and which of them it
/// is showing. Selection lives here so one revision covers both.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Conversations {
    summaries: Vec<ChatSummary>,
    selected_id: Option<String>,
}

#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    /// Latest accepted message, normalized and bounded for navigation lists.
    pub preview: String,
}

/// Registration is weak: an established chat holds the callback that registered it.
struct RegisterChat {
    established: Weak<Mutex<HashMap<String, ChatClient>>>,
    conversations: Arc<Watch<Conversations>>,
}
impl RegisterChat {
    fn new(established: &Established, conversations: &Arc<Watch<Conversations>>) -> Arc<Self> {
        Arc::new(Self {
            established: Arc::downgrade(established),
            conversations: conversations.clone(),
        })
    }
}
impl ChatObserver for RegisterChat {
    fn chat_changed(&self, chat_id: &str, chat: &ChatClient) {
        let Some(established) = self.established.upgrade() else {
            return;
        };
        let summary = ChatSummary {
            id: chat_id.to_owned(),
            title: summary_text(chat, true, 48),
            preview: summary_text(chat, false, 160),
        };
        established
            .lock()
            .expect("chat registry poisoned")
            .insert(chat_id.to_owned(), chat.clone());
        self.conversations.update(|conversations| {
            match conversations
                .summaries
                .iter_mut()
                .find(|known| known.id == chat_id)
            {
                Some(known) => known.preview = summary.preview,
                None => conversations.summaries.insert(0, summary),
            }
        });
    }
}

/// Bounded single-line text from this chat's first or latest message, for a
/// navigation list that cannot show line breaks.
fn summary_text(chat: &ChatClient, first: bool, limit: usize) -> String {
    let read = |message: Option<&arut_feature_chat::ChatMessage>| {
        message
            .map(|message| {
                message
                    .text
                    .split_whitespace()
                    .flat_map(|word| std::iter::once(' ').chain(word.chars()))
                    .skip(1)
                    .take(limit)
                    .collect()
            })
            .unwrap_or_default()
    };
    match first {
        true => chat.read_first_message(read),
        false => chat.read_last_message(read),
    }
}

impl ProductSession {
    /// Compose a session from a local feature and its served capability descriptors.
    pub fn from_chat(
        feature: &arut_feature_chat::ChatFeature,
        scope: SessionScope,
        ids: Arc<dyn IdSource>,
    ) -> Self {
        Self::new(
            feature.clients(),
            capability_client::<arut_feature_chat::ChatServices>(),
            scope,
            ids,
        )
    }

    /// Bind the session's clients to the composition root's chosen route.
    pub fn remote(
        channel: Arc<dyn arut_rpc::RpcChannel>,
        scope: SessionScope,
        ids: Arc<dyn IdSource>,
    ) -> Self {
        Self::new(
            ChatClients::remote(channel.clone()),
            CapabilityServiceClient::remote(channel),
            scope,
            ids,
        )
    }

    pub async fn initialize(&self) -> Result<(), SessionError> {
        let response = self
            .workspace
            .chat_service()
            .list_conversations(Request::new(
                arut_protocol::chat::v1::ListConversationsRequest {},
            ))
            .await?;
        let registration = self.registration();
        for conversation in response.message.conversations {
            if self.established(&conversation.id).is_some() {
                continue;
            }
            let client = ChatClient::established(
                self.workspace.chat_service(),
                self.workspace.composer_service(),
                conversation.id.clone(),
                conversation.messages,
                self.ids.clone(),
                self.workspace.conversation_cancellation(),
            )
            .with_observer(registration.clone());
            registration.chat_changed(&conversation.id, &client);
        }
        Ok(())
    }

    fn new(
        clients: ChatClients,
        capabilities: CapabilityServiceClient,
        scope: SessionScope,
        ids: Arc<dyn IdSource>,
    ) -> Self {
        let runtime = Arc::new(clients);
        let workspace = scopes::Node::new(scope.node_id, runtime).workspace(scope.workspace_id);
        let established = Established::default();
        let conversations = Arc::new(Watch::new(Conversations::default()));
        let pending = ChatClient::pending(
            workspace.chat_service(),
            workspace.composer_service(),
            scope.pending_scope_id.clone(),
            Some(RegisterChat::new(&established, &conversations)),
            ids.clone(),
            workspace.conversation_cancellation(),
        );
        Self {
            pending: Mutex::new(pending),
            ids,
            established,
            workspace,
            pending_scope_id: scope.pending_scope_id,
            conversations,
            capability_service: capabilities,
            availability: Watch::new(SessionAvailability {
                composer: FeatureAvailability::Unknown,
            }),
        }
    }

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

    pub fn pending_scope_id(&self) -> &str {
        &self.pending_scope_id
    }

    /// The node scope every workspace, conversation, and operation hangs from.
    /// Cancelling it stops the whole session's outstanding work.
    pub fn cancellation(&self) -> &Cancellation {
        self.workspace.node().cancellation()
    }

    pub fn chat(&self) -> ChatClient {
        self.pending.lock().expect("pending chat poisoned").clone()
    }

    pub fn new_chat(&self) -> ChatClient {
        let mut pending = self.pending.lock().expect("pending chat poisoned");
        if pending.id().is_some() {
            *pending = self.new_pending();
        }
        pending.clone()
    }

    pub fn select_chat(&self, chat_id: &str) -> Option<ChatClient> {
        self.established(chat_id)
    }

    /// Records which conversation surfaces are showing, so every surface on
    /// this session follows the same selection through one revision.
    pub fn select(&self, chat_id: Option<String>) {
        self.conversations
            .update(|conversations| conversations.selected_id = chat_id);
    }

    pub fn selected_id(&self) -> Option<String> {
        self.conversations
            .read(|conversations| conversations.selected_id.clone())
    }

    /// The conversation list a surface renders, newest first.
    pub fn chat_summaries(&self) -> Vec<ChatSummary> {
        self.conversations
            .read(|conversations| conversations.summaries.clone())
    }

    pub fn conversations_changes(&self) -> Arc<Subscription<u64>> {
        self.conversations.subscribe()
    }

    pub fn availability(&self) -> SessionAvailability {
        self.availability.get()
    }

    pub fn availability_changes(&self) -> Arc<Subscription<u64>> {
        self.availability.subscribe()
    }

    pub async fn refresh_capabilities(&self) -> SessionAvailability {
        self.availability
            .set(availability::read(&self.capability_service).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arut_feature_chat::composer::ComposerScope;
    use arut_feature_chat::test_support::TestIds;
    use futures_executor::block_on;

    fn session() -> ProductSession {
        let ids: Arc<dyn IdSource> = Arc::new(TestIds);
        let feature = arut_feature_chat::compose(Arc::new(
            arut_feature_chat::test_support::MemoryPorts::new(ids.clone()),
        ))
        .unwrap();
        ProductSession::from_chat(
            &feature,
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
            registry: session.established.clone(),
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
    fn a_pending_chat_becomes_a_conversation_with_its_own_draft_and_summary() {
        let session = session();
        let first = session.chat();
        assert_eq!(first.id(), None);
        assert_eq!(
            first.composer().scope(),
            ComposerScope::pending("account:one")
        );
        assert!(session.chat_summaries().is_empty());

        block_on(first.composer().replace("first\n  conversation".into()));
        let sent = block_on(first.send("first\n  conversation".into()));
        let first_id = sent.id.unwrap();

        assert_eq!(titles(&session), ["first conversation"]);
        assert!(session.select_chat(&first_id).is_some());
        assert_eq!(first.messages_after(0).len(), 2);
        assert_eq!(first.composer().state().text, "");

        let second = session.new_chat();
        assert_eq!(second.id(), None);
        assert_eq!(
            second.composer().scope(),
            ComposerScope::pending("account:one")
        );
        block_on(second.send("second conversation".into()));
        assert_eq!(
            titles(&session),
            ["second conversation", "first conversation"],
            "newest first"
        );

        block_on(first.composer().replace("send me".into()));
        block_on(second.composer().replace("keep me".into()));
        let changes = session.conversations_changes();
        let revision = block_on(changes.changed()).unwrap();

        block_on(first.send("send me".into()));

        let summaries = session.chat_summaries();
        let summary = summaries
            .iter()
            .find(|summary| summary.id == first_id)
            .unwrap();
        assert_eq!(
            summary.title, "first conversation",
            "the title is the first message"
        );
        assert_eq!(summary.preview, "You said: send me");
        assert_eq!(summaries[0].preview, "You said: second conversation");
        assert!(block_on(changes.changed()).unwrap() > revision);
        assert_eq!(first.composer().state().text, "");
        assert_eq!(second.composer().state().text, "keep me");
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
    fn initialized_unvisited_chats_have_previews_and_keep_updating() {
        let existing = session();
        let id = block_on(existing.chat().send("stored\n message".into()))
            .id
            .unwrap();
        let restored = ProductSession::new(
            ChatClients {
                chat: existing.workspace.chat_service(),
                composer: existing.workspace.composer_service(),
            },
            existing.capability_service.clone(),
            SessionScope {
                node_id: "local".into(),
                workspace_id: "default".into(),
                pending_scope_id: "restored".into(),
            },
            Arc::new(TestIds),
        );
        block_on(restored.initialize()).unwrap();

        assert_eq!(
            restored.chat_summaries()[0].preview,
            "You said: stored message"
        );
        let chat = restored.select_chat(&id).unwrap();
        restored.select(Some(id.clone()));
        assert_eq!(restored.selected_id(), Some(id));
        block_on(chat.send("updated".into()));
        assert_eq!(restored.chat_summaries()[0].preview, "You said: updated");

        let registry = Arc::downgrade(&restored.established);
        drop(restored);
        assert!(
            registry.upgrade().is_none(),
            "a retained chat must not retain its session"
        );
    }

    #[test]
    fn preview_text_is_bounded_without_splitting_unicode_scalars() {
        let session = session();
        block_on(session.chat().send("🦀".repeat(200)));
        let summary = session.chat_summaries().remove(0);
        assert_eq!(summary.title.chars().count(), 48);
        assert_eq!(summary.preview.chars().count(), 160);
        assert!(summary.preview.starts_with("You said: 🦀"));
    }

    #[test]
    fn a_cancelled_conversation_scope_refuses_to_send() {
        use arut_feature_chat::errors::ChatError;
        let session = session();
        let chat = session.chat();
        chat.cancellation().cancel();

        let state = block_on(chat.send("after cancellation".into()));

        assert_eq!(state.error, Some(ChatError::Cancelled));
        assert!(!state.can_send);
    }
}
