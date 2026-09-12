---
status: accepted
amended-by: 0019 (an iroh bi-directional stream is one more RpcChannel; QUIC and WebRTC channels of our own are not built)
---

# Connect protocol framing over `RpcChannel`; transports are adapters beneath it

Surfaces include browsers, editors, and messaging integrations that cannot all speak a custom framing, and a solo maintainer cannot own one. We decided to keep `RpcChannel` as the object-safe boundary and to use the Connect protocol as the framing for every HTTP-carried route: unary and streaming envelopes, standard content types, and a JSON error envelope that preserves typed status details. Memory, IPC, WebSocket, and later QUIC and WebRTC implement the channel. Metadata such as auth, authority epoch, trace context, and protocol version is attached by layers around a channel, never by feature code.

## Considered options

- **gRPC with grpc-web.** Rejected for now: needs a proxy for browsers; revisit if Envoy fronts the backend.
- **Custom POST with a bare body.** Rejected: no streaming, no error structure, no interoperability.

## Consequences

- Rust Connect server libraries were immature when this was written, so the framing is implemented once over axum; browsers use the official Connect-ES client. The `connectrpc` crate (0.9, Apache-2.0, conformance-suite clean as of 2026-09) is a candidate to replace the hand-written framing; see the spike in `docs/ECOSYSTEM.md`.
- On wasm, BoltFFI does not use wasm-bindgen, so a browser channel must route through host callbacks rather than `reqwest`.
