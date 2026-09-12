# Protocols

Versioned Protobuf packages define values and services that cross isolation, persistence, replication, or language boundaries.

`arut-protocol-build` recursively discovers `.proto` files, generates the Rust package tree, and generates typed service traits, direct and remote clients, erased routers, canonical procedure names, and service descriptors. Adding a protocol file requires no Rust module or `build.rs` registration.

`arut-rpc` contains the transport-neutral request, response, status, stream, channel, registry, and descriptor types used by generated code. Network protocols and raw I/O remain adapters below `RpcChannel`.

The capability service derives its manifest from generated service descriptors and runtime registration metadata. Service names, versions, methods, routes, and streaming shapes are never repeated in a handwritten capability registry. Product sessions translate the wire manifest into feature-specific availability.

Feature crates implement generated service traits. Product code uses generated clients. Neither side writes procedure paths, Protobuf framing, or transport-specific dispatch.
