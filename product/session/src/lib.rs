//! Sessions own typed scopes over feature clients supplied by composition roots.
//!
//! The session owns pending and established chats, the conversation list with
//! its selection, and availability. A weak registration callback avoids a cycle
//! between the registry and its chats. Local feature and remote channel
//! constructors assemble the required capability client.

mod availability;
pub use availability::{FeatureAvailability, SessionAvailability};
pub mod hosting;
/// Feature handles and renderable projections a product session exposes.
///
/// The feature crate is the one place these are named; a session re-exports it
/// whole so a surface reaches them without a dependency on `features/`.
pub use arut_feature_chat as chat;
/// Why a session-level call produced no usable answer.
///
/// The same reasons a feature scope reports, because they are the same node
/// answering; kept under this name while surfaces move to `NodeFailure`.
pub use arut_feature_chat::errors::NodeFailure as SessionError;
pub mod feature;
/// Observation contract shared by the session and its feature handles.
pub use arut_watch::Subscription;
pub mod scopes;
use arut_feature_chat::ports::IdSource;
use arut_feature_chat::{ChatClient, ChatObserver, ConversationTitle};
use arut_protocol::capability::v1::CapabilityServiceClient;
use arut_protocol::capability_manifest::capability_client;
use arut_protocol::chat::{composer::v1::ComposerServiceClient, v1::ChatServiceClient};
use arut_rpc::{Cancellation, Request, RpcChannel};
use arut_watch::Watch;
use feature::{FeatureSet, HasChat, Services};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

/// Scope identities supplied by the composition root.
pub struct SessionScope {
    pub node_id: String,
    pub workspace_id: String,
    pub pending_scope_id: String,
}

/// A session over the features `F` names. A root writes
/// `ProductSession<(Chat, Approvals)>` and nothing else changes.
pub struct ProductSession<F: FeatureSet> {
    pending: Mutex<ChatClient>,
    ids: Arc<dyn IdSource>,
    established: Established,
    workspace: Arc<scopes::Workspace<F::Clients>>,
    pending_scope_id: String,
    conversations: Arc<Watch<Conversations>>,
    pending_mutations: Mutex<HashMap<ConversationMutation, String>>,
    capability_service: CapabilityServiceClient,
    availability: Watch<SessionAvailability>,
}

#[derive(Clone, Hash, PartialEq, Eq)]
enum ConversationMutation {
    Rename {
        chat_id: String,
        title: ConversationTitle,
    },
    Delete {
        chat_id: String,
    },
}

type Established = Arc<Mutex<HashMap<String, ChatClient>>>;

/// The conversation list a surface renders, newest first, which of them it is
/// showing, and what it is searching for. All three live here so one revision
/// covers them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Conversations {
    entries: Vec<Entry>,
    selection: Selection,
    query: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum Selection {
    #[default]
    Pending,
    Established(String),
}

impl Selection {
    fn id(&self) -> Option<&str> {
        match self {
            Self::Pending => None,
            Self::Established(id) => Some(id),
        }
    }
}

/// One conversation in the list, beside the two keys that decide whether it is
/// unread: where its transcript has reached, and where it had reached the last
/// time it was the selected conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    summary: ChatSummary,
    last_message_id: u64,
    seen_message_id: u64,
}

impl Entry {
    /// Recomputes what the summary derives, the way `ChatState::derive` does,
    /// so no surface restates the rule.
    fn derive(&mut self) {
        self.summary.unread = self.last_message_id > self.seen_message_id;
    }
}

#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    /// Latest accepted message, normalized and bounded for navigation lists.
    pub preview: String,
    /// Messages arrived since this conversation was last the selected one.
    ///
    /// Derived from the transcript key and the selection this session owns;
    /// selecting the conversation clears it.
    pub unread: bool,
    /// Where the current query matches `title`, so a surface highlights without
    /// repeating the matching rule.
    pub match_ranges: Vec<MatchRange>,
}

/// Half-open character range of one query match in a title.
///
/// Characters are Unicode scalars, not UTF-16 units or grapheme clusters: a
/// surface whose string type is indexed otherwise converts once.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchRange {
    pub start: u32,
    pub end: u32,
}

/// Registration is weak: an established chat holds the callback that registered it.
struct RegisterChat {
    established: Weak<Mutex<HashMap<String, ChatClient>>>,
    conversations: Arc<Watch<Conversations>>,
    /// A pending chat the person is typing into becomes the selected
    /// conversation the moment it is established; chats found at startup do not.
    adopt_selection: bool,
}
impl RegisterChat {
    fn new(
        established: &Established,
        conversations: &Arc<Watch<Conversations>>,
        adopt_selection: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            established: Arc::downgrade(established),
            conversations: conversations.clone(),
            adopt_selection,
        })
    }
}
impl ChatObserver for RegisterChat {
    fn chat_changed(&self, chat_id: &str, chat: &ChatClient) {
        let Some(established) = self.established.upgrade() else {
            return;
        };
        let last_message_id = chat.state().last_message_id;
        let summary = ChatSummary {
            id: chat_id.to_owned(),
            title: summary_text(chat, true, 48),
            preview: summary_text(chat, false, 160),
            unread: false,
            match_ranges: Vec::new(),
        };
        established
            .lock()
            .expect("chat registry poisoned")
            .insert(chat_id.to_owned(), chat.clone());
        self.conversations.update(|conversations| {
            let known = conversations
                .entries
                .iter()
                .position(|known| known.summary.id == chat_id);
            if known.is_none() && self.adopt_selection {
                conversations.selection = Selection::Established(chat_id.to_owned());
            }
            let selected = conversations.selection.id() == Some(chat_id);
            let entry = match known {
                Some(index) => {
                    let entry = &mut conversations.entries[index];
                    entry.summary.preview = summary.preview;
                    entry
                }
                // A conversation joins the list at whatever it has already
                // accepted: what arrives after that is what is unread.
                None => {
                    conversations.entries.insert(
                        0,
                        Entry {
                            summary,
                            last_message_id,
                            seen_message_id: last_message_id,
                        },
                    );
                    &mut conversations.entries[0]
                }
            };
            entry.last_message_id = last_message_id;
            if selected {
                entry.seen_message_id = last_message_id;
            }
            entry.derive();
        });
    }
}

/// Bounded single-line text from this chat's first or latest message, for a
/// navigation list that cannot show line breaks. A conversation the person
/// renamed keeps that title instead of a derived one.
fn summary_text(chat: &ChatClient, first: bool, limit: usize) -> String {
    if first && let Some(title) = chat.title() {
        return title.into();
    }
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

/// One casing rule for every surface: Unicode's language-neutral lowercase,
/// with the source character each folded character came from, so a match in the
/// folded text maps back to the text a surface renders.
///
/// `char::to_lowercase` is not locale-tailored: it does not know that Turkish
/// lowercases `I` to `\u{131}`, which ICU would. Taking ICU for that would put a
/// locale in the core, which ADR 0016 keeps out, and both the query and the
/// title are the same person's own text. If a surface reports a language where
/// this reads wrongly, the tailoring belongs behind a port, not here.
fn folded(text: &str) -> (Vec<char>, Vec<usize>) {
    let mut characters = Vec::new();
    let mut origins = Vec::new();
    for (index, character) in text.chars().enumerate() {
        for lowered in character.to_lowercase() {
            characters.push(lowered);
            origins.push(index);
        }
    }
    (characters, origins)
}

/// Every place `query` matches `text`, as `(start, end)` character offsets into
/// `text`. `query` is already folded by [`folded`].
fn match_ranges(text: &str, query: &[char]) -> Vec<MatchRange> {
    if query.is_empty() {
        return Vec::new();
    }
    let (characters, origins) = folded(text);
    let mut ranges = Vec::new();
    let mut start = 0;
    while start + query.len() <= characters.len() {
        if characters[start..start + query.len()] == *query {
            let first = origins[start];
            let last = origins[start + query.len() - 1];
            ranges.push(MatchRange {
                start: u32::try_from(first).unwrap_or(u32::MAX),
                end: u32::try_from(last + 1).unwrap_or(u32::MAX),
            });
            start += query.len();
        } else {
            start += 1;
        }
    }
    ranges
}

impl<F: HasChat> ProductSession<F> {
    /// Compose a session over the clients a local root built, advertising
    /// exactly what the set serves.
    pub fn local(clients: F::Clients, scope: SessionScope, ids: Arc<dyn IdSource>) -> Self {
        Self::new(clients, capability_client::<Services<F>>(), scope, ids)
    }

    /// Bind the session's clients to the composition root's chosen route.
    pub fn remote(
        channel: Arc<dyn RpcChannel>,
        scope: SessionScope,
        ids: Arc<dyn IdSource>,
    ) -> Self {
        Self::new(
            F::remote(&channel),
            CapabilityServiceClient::remote(channel),
            scope,
            ids,
        )
    }

    fn chat_service(&self) -> ChatServiceClient {
        F::chat(self.workspace.clients()).chat.clone()
    }

    fn composer_service(&self) -> ComposerServiceClient {
        F::chat(self.workspace.clients()).composer.clone()
    }

    pub async fn initialize(&self) -> Result<(), SessionError> {
        let response = self
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
                self.chat_service(),
                self.composer_service(),
                conversation.id.clone(),
                conversation.messages,
                self.ids.clone(),
                self.workspace.conversation_cancellation(),
            )
            .with_observer(registration.clone());
            if let Some(title) = conversation
                .title
                .and_then(|title| ConversationTitle::try_from(title).ok())
            {
                client.set_title(title);
            }
            registration.chat_changed(&conversation.id, &client);
        }
        Ok(())
    }

    fn new(
        clients: F::Clients,
        capabilities: CapabilityServiceClient,
        scope: SessionScope,
        ids: Arc<dyn IdSource>,
    ) -> Self {
        let runtime = Arc::new(clients);
        let workspace = scopes::Node::new(scope.node_id, runtime).workspace(scope.workspace_id);
        let established = Established::default();
        let conversations = Arc::new(Watch::new(Conversations::default()));
        let pending = ChatClient::pending(
            F::chat(workspace.clients()).chat.clone(),
            F::chat(workspace.clients()).composer.clone(),
            scope.pending_scope_id.clone(),
            Some(RegisterChat::new(&established, &conversations, true)),
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
            pending_mutations: Mutex::default(),
            capability_service: capabilities,
            availability: Watch::new(SessionAvailability {
                composer: FeatureAvailability::Unknown,
            }),
        }
    }

    fn registration(&self) -> Arc<RegisterChat> {
        RegisterChat::new(&self.established, &self.conversations, false)
    }

    fn new_pending(&self) -> ChatClient {
        ChatClient::pending(
            self.chat_service(),
            self.composer_service(),
            self.pending_scope_id.clone(),
            Some(RegisterChat::new(
                &self.established,
                &self.conversations,
                true,
            )),
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
    /// this session follows the same selection through one revision. Selecting
    /// a conversation also marks it read.
    pub fn select(&self, chat_id: Option<String>) {
        let selection = match chat_id {
            Some(id) if self.established(&id).is_some() => Selection::Established(id),
            _ => Selection::Pending,
        };
        self.conversations.update(|conversations| {
            conversations.selection.clone_from(&selection);
            let Selection::Established(chat_id) = &selection else {
                return;
            };
            if let Some(entry) = conversations
                .entries
                .iter_mut()
                .find(|entry| entry.summary.id == chat_id.as_str())
            {
                entry.seen_message_id = entry.last_message_id;
                entry.derive();
            }
        });
    }

    pub fn selected_id(&self) -> Option<String> {
        self.conversations
            .read(|conversations| conversations.selection.id().map(str::to_owned))
    }

    /// The title of the selected conversation, or `None` when the pending one
    /// is showing and the surface names it in its own words.
    pub fn selected_title(&self) -> Option<String> {
        self.conversations.read(|conversations| {
            let selected = conversations.selection.id()?;
            conversations
                .entries
                .iter()
                .find(|entry| entry.summary.id == selected)
                .map(|entry| entry.summary.title.clone())
        })
    }

    /// What the list is being searched for. Empty means the whole list.
    pub fn query(&self) -> String {
        self.conversations
            .read(|conversations| conversations.query.clone())
    }

    /// Narrows the conversation list. One casing rule for every surface.
    pub fn set_query(&self, query: String) {
        self.conversations
            .update(|conversations| conversations.query = query);
    }

    /// Renames one conversation, durably: the title becomes `title` and stays
    /// that way across sessions. Returns `false` when the node did not accept it.
    pub async fn rename(&self, chat_id: String, title: String) -> bool {
        let Ok(title) = ConversationTitle::try_from(title) else {
            return false;
        };
        let mutation = ConversationMutation::Rename {
            chat_id: chat_id.clone(),
            title: title.clone(),
        };
        let command_id = self.mutation_id(&mutation);
        let request_title = title.clone().into();
        let accepted = self
            .chat_service()
            .rename_conversation(Request::new(
                arut_protocol::chat::v1::RenameConversationRequest {
                    chat_id: chat_id.clone(),
                    title: request_title,
                    command_id,
                },
            ))
            .await
            .is_ok();
        if accepted {
            self.pending_mutations.lock().unwrap().remove(&mutation);
            if let Some(chat) = self.established(&chat_id) {
                chat.set_title(title.clone());
            }
            self.conversations.update(|conversations| {
                if let Some(entry) = conversations
                    .entries
                    .iter_mut()
                    .find(|entry| entry.summary.id == chat_id)
                {
                    entry.summary.title = title.into();
                }
            });
        }
        accepted
    }

    /// Deletes one conversation durably: its transcript and summary leave the
    /// list for every surface on this session, and asking for it again returns
    /// nothing. Returns `false` when the node did not accept it.
    pub async fn delete(&self, chat_id: String) -> bool {
        let mutation = ConversationMutation::Delete {
            chat_id: chat_id.clone(),
        };
        let command_id = self.mutation_id(&mutation);
        let accepted = self
            .chat_service()
            .delete_conversation(Request::new(
                arut_protocol::chat::v1::DeleteConversationRequest {
                    chat_id: chat_id.clone(),
                    command_id,
                },
            ))
            .await
            .is_ok();
        if accepted {
            self.pending_mutations.lock().unwrap().remove(&mutation);
            let mut pending = self.pending.lock().expect("pending chat poisoned");
            if pending.id().as_deref() == Some(chat_id.as_str()) {
                *pending = self.new_pending();
            }
            drop(pending);
            self.established
                .lock()
                .expect("chat registry poisoned")
                .remove(&chat_id);
            self.conversations.update(|conversations| {
                conversations
                    .entries
                    .retain(|entry| entry.summary.id != chat_id);
                if conversations.selection.id() == Some(chat_id.as_str()) {
                    conversations.selection = Selection::Pending;
                }
            });
        }
        accepted
    }

    fn mutation_id(&self, mutation: &ConversationMutation) -> String {
        self.pending_mutations
            .lock()
            .unwrap()
            .entry(mutation.clone())
            .or_insert_with(|| self.ids.new_id())
            .clone()
    }

    /// The conversation list a surface renders, newest first, narrowed to the
    /// current query and carrying where each title matched it.
    pub fn chat_summaries(&self) -> Vec<ChatSummary> {
        self.conversations.read(|conversations| {
            let query = folded(&conversations.query).0;
            conversations
                .entries
                .iter()
                .filter_map(|entry| {
                    if query.is_empty() {
                        return Some(entry.summary.clone());
                    }
                    let ranges = match_ranges(&entry.summary.title, &query);
                    if ranges.is_empty() && match_ranges(&entry.summary.preview, &query).is_empty()
                    {
                        return None;
                    }
                    let mut summary = entry.summary.clone();
                    summary.match_ranges = ranges;
                    Some(summary)
                })
                .collect()
        })
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
    use crate::feature::Chat;
    use arut_feature_chat::composer::ComposerScope;
    use arut_feature_chat::test_support::TestIds;
    use futures_executor::block_on;

    fn session() -> ProductSession<(Chat,)> {
        let ids: Arc<dyn IdSource> = Arc::new(TestIds);
        let runtime = Arc::new(arut_feature_chat::test_support::MemoryPorts::new(
            ids.clone(),
        ));
        let composed = <(Chat,) as feature::ComposeSet<_>>::compose(&runtime).unwrap();
        ProductSession::local(
            composed.clients,
            SessionScope {
                node_id: "local".into(),
                workspace_id: "default".into(),
                pending_scope_id: "account:one".into(),
            },
            ids,
        )
    }

    fn titles(session: &ProductSession<(Chat,)>) -> Vec<String> {
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
    fn a_conversation_is_unread_only_while_another_one_is_selected() {
        let session = session();
        let first = session.chat();
        let first_id = block_on(first.send("first".into())).id.unwrap();
        assert!(
            !session.chat_summaries()[0].unread,
            "a conversation joins the list at what it has already accepted"
        );

        let second_id = block_on(session.new_chat().send("second".into()))
            .id
            .unwrap();
        session.select(Some(second_id));
        block_on(first.send("later".into()));

        let unread = |id: &str| {
            session
                .chat_summaries()
                .into_iter()
                .find(|summary| summary.id == id)
                .unwrap()
                .unread
        };
        assert!(unread(&first_id));
        session.select(Some(first_id.clone()));
        assert!(!unread(&first_id));
        assert_eq!(session.selected_title(), Some("first".to_owned()));
        session.select(None);
        assert_eq!(session.selected_title(), None);
    }

    #[test]
    fn a_query_narrows_the_list_and_says_where_each_title_matched() {
        let session = session();
        block_on(session.chat().send("Grocery list".into()));
        block_on(session.new_chat().send("Roadmap".into()));

        session.set_query("ro".into());
        assert_eq!(session.query(), "ro");
        assert_eq!(
            session
                .chat_summaries()
                .iter()
                .map(|summary| (summary.title.clone(), summary.match_ranges.clone()))
                .collect::<Vec<_>>(),
            [
                ("Roadmap".to_owned(), vec![MatchRange { start: 0, end: 2 }]),
                (
                    "Grocery list".to_owned(),
                    vec![MatchRange { start: 1, end: 3 }]
                ),
            ],
            "matching ignores case and reports character offsets into the title"
        );

        session.set_query("roadmap".into());
        assert_eq!(session.chat_summaries().len(), 1);
        session.set_query("you said".into());
        assert_eq!(
            session.chat_summaries().len(),
            2,
            "a preview match keeps the row without highlighting its title"
        );
        assert!(session.chat_summaries()[0].match_ranges.is_empty());
        session.set_query(String::new());
        assert_eq!(session.chat_summaries().len(), 2);
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
        let restored = ProductSession::<(Chat,)>::new(
            existing.workspace.clients().clone(),
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
    fn rename_and_delete_are_durable_and_update_the_live_session() {
        let original = session();
        let chat = original.chat();
        let id = block_on(chat.send("derived title".into())).id.unwrap();

        assert!(block_on(
            original.rename(id.clone(), "  Project   notes  ".into())
        ));
        assert_eq!(original.selected_title(), Some("Project notes".into()));
        block_on(chat.send("later message".into()));
        assert_eq!(original.selected_title(), Some("Project notes".into()));

        let restored = ProductSession::<(Chat,)>::new(
            original.workspace.clients().clone(),
            original.capability_service.clone(),
            SessionScope {
                node_id: "local".into(),
                workspace_id: "default".into(),
                pending_scope_id: "restored-rename".into(),
            },
            Arc::new(TestIds),
        );
        block_on(restored.initialize()).unwrap();
        restored.select(Some(id.clone()));
        assert_eq!(restored.selected_title(), Some("Project notes".into()));

        assert!(block_on(restored.delete(id.clone())));
        assert!(restored.chat_summaries().is_empty());
        assert_eq!(restored.selected_id(), None);
        assert!(restored.select_chat(&id).is_none());
        assert_eq!(restored.chat().id(), None);

        let after_delete = ProductSession::<(Chat,)>::new(
            original.workspace.clients().clone(),
            original.capability_service.clone(),
            SessionScope {
                node_id: "local".into(),
                workspace_id: "default".into(),
                pending_scope_id: "restored-delete".into(),
            },
            Arc::new(TestIds),
        );
        block_on(after_delete.initialize()).unwrap();
        assert!(after_delete.chat_summaries().is_empty());
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
