//! Chat commands, transcript projections, composer drafts, and services.
//!
//! Accepted mock exchanges append message and operation lifecycle facts atomically.
//! UUIDv7 command IDs deduplicate retries; projections replay on restart. Clients
//! keep immutable messages keyed by ID and expose exclusive range reads separately
//! from watched status and error metadata.
//!
//! Drafts stream as ephemeral snapshots per scope and recover locally through
//! KeyValue, including the pending draft. They never become transcript facts.
//! Projection types declare their FFI data once here. Typed errors leave sentence
//! selection to surfaces; tracing records stream cursors without draft content.

pub mod command;
pub mod composer;
pub mod domain;
pub mod errors;
pub mod facts;
pub mod ports;
pub mod product;
pub mod service;
