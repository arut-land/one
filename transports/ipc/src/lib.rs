//! # IPC
//!
//! Connect over HTTP on a Unix socket. The local daemon creates a private socket;
//! the channel is the Connect one, reached through reqwest's Unix-socket
//! connector, so no framing or dispatch is written twice.

#![cfg(unix)]
use arut_transport_connect_http::HttpRpcChannel;
use std::path::Path;

pub fn unix_socket(path: &Path) -> Result<HttpRpcChannel, reqwest::Error> {
    let client = reqwest::Client::builder()
        .unix_socket(path.to_owned())
        .build()?;
    Ok(HttpRpcChannel::with_client("http://localhost", client))
}
