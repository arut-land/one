//! Connect unary Protobuf and server streams over HTTP, and the same router
//! over a Unix socket.
//!
//! Identity compression is supported; messages are bounded to 8 MiB.
//! Request-streaming methods return Unimplemented.

#![cfg(not(target_arch = "wasm32"))]
mod client;
mod framing;
#[cfg(unix)]
pub mod ipc;
mod metadata;
mod server;
mod streaming;

pub use client::HttpRpcChannel;
pub use framing::MAX_MESSAGE;
pub use server::router;
