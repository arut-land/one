# 0007: Protobuf contracts and versioning

## Status

Accepted as target architecture.

## Context

Arut components update independently and communicate in-process, across FFI, through local IPC, peer-to-peer, and through cloud infrastructure. Duplicating equivalent schemas adds conversion work, but using generated Protobuf structs for every internal invariant also weakens the Rust model.

## Decision

Protobuf defines stable values that cross an isolation, persistence, replication, or language boundary. Generated Rust types may be reused directly inside a process when they are already the correct representation. Internal domain types remain handwritten when they encode stronger invariants or never cross a boundary.

Generated direct clients pass protocol values without serialization. Process and network channels encode the same contracts. Transport framing and product message meaning remain separate.

Schema evolution is additive within a version. Removed field numbers and names are reserved. Conceptual breaks introduce versioned packages such as `conversation.v2` with explicit adapters. Components negotiate supported service versions through the generated manifest and enforce a documented rolling compatibility window.

## Consequences

- One wire definition serves local and remote connections.
- The code avoids mirror DTOs that add no invariant or boundary value.
- Rust remains free to use richer internal representations where justified.
- Compatibility tests and generated manifest tests are required before external release.
- Supporting a new major package requires explicit translation or a clear update error.
