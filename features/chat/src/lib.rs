//! # Chat
//!
//! Chat owns transcript projections, composer drafts, commands, and services. The
//! mock response is unchanged. Accepted exchanges append an atomic batch of message
//! and operation lifecycle facts. Send command IDs are UUIDv7 and retries return the
//! stored outcome. Projections replay facts on restart.
//!
//! Composer drafts are ephemeral snapshots streamed per scope. KeyValue recovers
//! the local draft, including a pending draft. Drafts are never transcript facts.
//! Projection data carries BoltFFI attributes here and is re-exported by bindings.

pub mod command;
pub mod composer;
pub mod domain;
pub mod facts;
pub mod ports;
pub mod product;
pub mod service;
