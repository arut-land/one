//! Chat commands, transcript projections, composer drafts, and services.
//! `compose` requires the IdSource, Persist, Drafts, and Clock capability bundle.
//!
//! Commands format mock replies and append message and operation lifecycle facts atomically,
//! stamped by the Clock port inside the authority transaction.
//! UUIDv7 command IDs deduplicate retries; projections replay on restart. Clients
//! keep immutable messages keyed by ID and expose exclusive range reads separately
//! from watched status and error metadata.
//!
//! Drafts stream as ephemeral snapshots per scope and recover locally through
//! KeyValue, including the pending draft. They never become transcript facts.
//! Projection types declare their FFI data once here. Typed errors leave sentence
//! selection to surfaces; tracing records stream cursors without draft content.

mod authority;
mod client;
mod command;
pub mod composer;
pub mod errors;
mod facts;
pub mod ports;
mod projection;
mod service;
pub use client::{ChatClient, ChatStarted};
#[cfg(test)]
pub(crate) use facts::ChatProjection;
pub use projection::{ChatMessage, ChatRole, ChatState, ChatStatus};
#[cfg(test)]
pub(crate) use service::ChatServiceImpl;

mod compose;
pub use compose::{ChatClients, ChatFeature, ComposeError, compose};
pub use ports::ChatRuntime;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

#[cfg(test)]
#[path = "tests/recovery.rs"]
mod recovery_tests;
#[cfg(test)]
#[path = "tests/send.rs"]
mod send_tests;
