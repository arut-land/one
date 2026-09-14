//! Connect over HTTP on a Unix socket.
//!
//! The local daemon creates a private socket; the channel is the Connect one
//! reached through reqwest's Unix-socket connector, so no framing or dispatch
//! is written twice.

use crate::HttpRpcChannel;
use std::path::Path;

/// A Connect channel that dials `path` instead of a TCP address.
///
/// # Errors
/// Returns the reqwest error if the client could not be built.
pub fn unix_socket(path: &Path) -> Result<HttpRpcChannel, reqwest::Error> {
    let client = reqwest::Client::builder()
        .unix_socket(path.to_owned())
        .build()?;
    Ok(HttpRpcChannel::with_client("http://localhost", client))
}
