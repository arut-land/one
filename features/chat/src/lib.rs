//! Chat commands, transcript projections, composer drafts, and services.
//!
//! `compose` requires the IdSource, Persist, Drafts, and Clock capability
//! bundle. One command appends its conversation fact atomically,
//! stamped by the Clock port inside the authority transaction. UUIDv7 command
//! IDs deduplicate retries; projections replay on restart.
//!
//! Drafts stream as ephemeral snapshots per scope and recover locally through
//! KeyValue, including the pending draft. They never become transcript facts
//! (ADR 0018). Projection types declare their FFI data once here. Typed errors
//! leave sentence selection to surfaces (ADR 0016); tracing records stream
//! cursors and never draft content.

mod authority;
mod client;
mod command;
pub mod composer;
pub mod errors;
mod facts;
pub mod ports;
mod projection;
mod service;
mod title;
pub use client::{ChatClient, ChatObserver};
// The crate root is the one name list a surface reaches these through: product
// re-exports this crate as `arut_product_session::chat`, and nothing above
// restates a type name.
pub use composer::{ComposerClient, ComposerState, ComposerStatus};
pub use errors::{ChatError, ComposerError, NodeFailure};
#[cfg(test)]
pub(crate) use facts::ChatProjection;
pub use projection::{ChatMessage, ChatRole, ChatState, ChatStatus};
#[cfg(test)]
pub(crate) use service::ChatServiceImpl;
pub use title::{ConversationTitle, InvalidConversationTitle};

mod compose;
pub use compose::{ChatClients, ChatFeature, ChatServices, ComposeError, compose};
pub use ports::ChatRuntime;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

#[cfg(test)]
#[path = "tests/recovery.rs"]
mod recovery_tests;
#[cfg(test)]
#[path = "tests/send.rs"]
mod send_tests;
