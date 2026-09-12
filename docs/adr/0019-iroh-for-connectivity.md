---
status: accepted
amends: 0003, 0009, 0012
---

# Device connectivity, identity, relay, and blob transfer use iroh

Phase 1 planned to hand-roll device keys, LAN discovery, NAT traversal, a relay, route health with failover, and transport encryption. iroh 1.0 provides all of them as one dependency: an endpoint is an Ed25519 key you dial directly, discovery and hole punching are built in, direct-versus-relay switching is automatic, QUIC over raw public keys encrypts the transport, and the relay is a self-hostable binary. We decided to build on iroh, `iroh-blobs` for attachment transfer, and `iroh-gossip` for ephemeral draft and presence replication.

What stays ours: pairing (the QR carries an endpoint id and transfers the root key), sealed-box encryption for envelopes held at rest by carriers and for backups, the authority and fact log, and `RpcChannel` implemented over an iroh bi-directional stream so the generated service layer remains the single RPC surface.

## Considered options

- **Hand-rolled mDNS, WebRTC, relay, and route policy.** Rejected: months of work with worse NAT traversal than a dedicated project.
- **libp2p.** Rejected: larger surface, weaker relay story, no key-as-address model.
- **`irpc` for RPC over iroh.** Rejected: two RPC systems; the generated Connect layer already covers every route.

## Consequences

- ADR 0003 keeps device keys and at-rest sealed boxes; transport encryption is now iroh's QUIC, not a Noise handshake of ours.
- ADR 0009 keeps Connect framing for HTTP-carried routes; an iroh stream is one more `RpcChannel`.
- ADR 0012's route policy becomes iroh's; no routing substrate is built.
- The backend relay is a deployment of `iroh-relay` plus a small pairing service.
- iroh's browser support uses wasm-bindgen, which the BoltFFI wasm core does not; the web surface reaches a node through the relay's WebSocket path and does not embed iroh.
