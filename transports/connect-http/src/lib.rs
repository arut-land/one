//! Connect unary Protobuf and server streams over HTTP.
//!
//! Identity compression is supported; messages are bounded to 8 MiB.
//! Request-streaming methods return Unimplemented. IPC reuses this router.

#![cfg(not(target_arch = "wasm32"))]
mod client;
mod framing;
mod metadata;
mod server;
mod streaming;

pub use client::HttpRpcChannel;
pub use framing::MAX_MESSAGE;
pub use server::router;
