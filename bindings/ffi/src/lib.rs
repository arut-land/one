//! Foreign bindings over shared scope projections and host-supplied time and IDs.
//!
//! The handles are generated. `bindings/ffi/handles.toml` declares each scope
//! and `arut-dev generate` writes `src/generated/handles.rs`, because BoltFFI's
//! source scanner expands no macros: a handle behind a macro disappears from
//! the bindings, even with `--deny-skipped`. What a person does stays
//! hand-written in `src/intents.rs`, as a second `#[export] impl` block on the
//! same handle (ADR 0021).
//!
//! An error crosses as its Fluent message id and its arguments, never as a
//! sentence: the surface resolves the id in its own resource system, so the
//! core still learns no locale (ADR 0016, ADR 0022).

pub use arut_feature_chat::composer::{ComposerState, ComposerStatus};
pub use arut_feature_chat::errors::{ChatError, ComposerError, NodeFailure};
use arut_feature_chat::ports::{Clock, IdSource};
pub use arut_feature_chat::{ChatMessage, ChatRole, ChatState, ChatStatus};
pub use arut_product_session::{ChatSummary, FeatureAvailability, MatchRange, SessionAvailability};
use boltffi::export;
use std::sync::Arc;

mod generated;
mod intents;
mod observation;

pub use generated::handles::{
    AvailabilityHandle, ChatHandle, ComposerHandle, ConversationsHandle, ProductSessionHandle,
};
pub use intents::{create_browser_session, create_product_session};

/// What a session composes. One line per feature: the manifest it advertises,
/// the routers it serves and the clients its scopes hold all follow from it
/// (ADR 0006, ADR 0025).
pub(crate) type Features = (arut_product_session::feature::Chat,);

/// One argument of the message a typed error names: the Fluent name that
/// selects it, and the value it carries.
///
/// The order is the order the generated resources interpolate, so a surface
/// that formats positionally passes the values straight through, and one that
/// formats by name does not have to know the order at all.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorArg {
    pub name: String,
    pub value: String,
}

impl From<(String, String)> for ErrorArg {
    fn from((name, value): (String, String)) -> Self {
        Self { name, value }
    }
}

/// Wall time and identities a host supplies, for platforms whose runtime owns
/// both.
#[export]
pub trait HostIds: Send + Sync {
    fn new_id(&self) -> String;
    fn now(&self) -> u64;
}

pub(crate) struct BrowserIds(pub Arc<dyn HostIds>);
impl IdSource for BrowserIds {
    fn new_id(&self) -> String {
        self.0.new_id()
    }
}
impl Clock for BrowserIds {
    fn now(&self) -> u64 {
        self.0.now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intents::create_product_session;

    #[test]
    fn shared_projections_expose_conversations_drafts_and_send() {
        let session = create_product_session("test".into());
        let list = session.conversations();
        let chat = session.chat();
        let composer = chat.composer();
        futures_executor::block_on(composer.replace("hello".into()));
        assert_eq!(composer.state().text, "hello");
        futures_executor::block_on(chat.send("hello".into()));
        assert_eq!(chat.messages_after(0).len(), 2);
        assert!(
            chat.messages_after(0)
                .iter()
                .all(|message| message.accepted_at_ms > 0)
        );
        assert_eq!(list.state().len(), 1);
        assert_eq!(composer.state().text, "");
    }

    #[test]
    fn a_typed_error_crosses_as_a_key_and_named_arguments() {
        let session = create_product_session("test".into());
        let chat = session.chat();
        assert_eq!(chat.error_key(), None);
        assert!(chat.error_args().is_empty());

        chat.inner.cancellation().cancel();
        futures_executor::block_on(chat.send("after cancellation".into()));
        assert_eq!(chat.error_key(), Some("chat-error-cancelled".into()));
        assert!(chat.error_args().is_empty());
    }

    #[test]
    fn the_conversation_list_carries_its_own_search_and_selection() {
        let session = create_product_session("test".into());
        let list = session.conversations();
        futures_executor::block_on(session.chat().send("Roadmap".into()));
        let id = list.state()[0].id.clone();

        list.select(Some(id.clone()));
        assert_eq!(list.selected_id(), Some(id));
        assert_eq!(list.title(), Some("Roadmap".to_owned()));

        list.set_query("road".into());
        assert_eq!(list.query(), "road");
        assert_eq!(
            list.state()[0].match_ranges,
            [MatchRange { start: 0, end: 4 }]
        );
        list.set_query("nothing here".into());
        assert!(list.state().is_empty());
    }
}
