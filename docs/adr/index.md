# Architecture Decision Records

ADRs record decisions that are hard to reverse, surprising without context, and the result of a real trade-off. `ARCHITECTURE.md` is the consolidated design; these records preserve why its boundaries are where they are. Product vocabulary is in `CONTEXT.md`, engineering vocabulary in `docs/GLOSSARY.md`.

| ADR | Decision | Status |
| --- | --- | --- |
| [0001](0001-conversation-authority-on-creating-node.md) | A conversation is authoritative on the node that created it | Accepted |
| [0002](0002-backend-is-relay-not-truth.md) | The backend pairs, relays, and stores encrypted backups; never the source of truth | Accepted |
| [0003](0003-device-keys-and-end-to-end-encryption.md) | Device keys, linked-device root key, end-to-end encryption from v1 | Accepted, amended by 0019 |
| [0004](0004-commands-are-carried-envelopes.md) | Commands are signed envelopes any paired device may carry | Accepted |
| [0005](0005-sans-io-machines-and-ports.md) | Sans-I/O machines, async ports, executor from the composition root | Accepted |
| [0006](0006-typed-scopes-not-a-service-container.md) | Typed scopes and capability bundles, no service container | Accepted, amended 2026-09-14 |
| [0007](0007-scope-handles-and-generated-bindings.md) | One observable handle per scope, generated bindings over BoltFFI | Accepted, amended 2026-09-14 |
| [0008](0008-generated-service-layer-from-protobuf.md) | Service layer generated from Protobuf descriptors | Accepted, amended by 0025 |
| [0009](0009-connect-framing-over-rpc-channel.md) | Connect framing over `RpcChannel`; transports are adapters | Accepted, amended by 0019 |
| [0010](0010-storage-as-three-ports.md) | Storage is three narrow ports; directory tree first | Accepted, amended by 0023 |
| [0011](0011-hosting-mode-is-a-port.md) | Hosting mode is a port chosen per platform | Accepted |
| [0012](0012-single-route-with-health-failover.md) | One active route with health failover in v1 | Accepted, amended by 0019 |
| [0013](0013-gtk4-without-libadwaita-on-linux.md) | GTK4 without libadwaita on Linux | Accepted, amended by 0020 |
| [0014](0014-model-access-through-rig.md) | Model access through `rig` behind a port | Accepted |
| [0015](0015-protobuf-contracts-and-compatibility-window.md) | Protobuf contracts, additive evolution, two-minor window | Accepted |
| [0016](0016-typed-errors-no-strings-from-core.md) | Typed errors; surfaces own user-facing strings | Accepted, amended by 0022 |
| [0017](0017-license-split.md) | FSL for product crates, MIT/Apache-2.0 for substrates | Accepted |
| [0018](0018-drafts-are-ephemeral-last-writer-wins.md) | Drafts are ephemeral, last-writer-wins, not facts | Accepted, amended by 0024 |
| [0019](0019-iroh-for-connectivity.md) | Connectivity, identity, relay, and blob transfer use iroh | Accepted, amends 0003, 0009, 0012 |
| [0020](0020-relm4-for-the-linux-surface.md) | The Linux surface is written with relm4 | Accepted, amends 0013 |
| [0021](0021-no-rust-reactive-framework-in-the-core.md) | No Rust reactive framework in the core | Accepted, amended 2026-09-14 |
| [0022](0022-fluent-as-the-single-string-source.md) | One Fluent string source generated into native resources | Accepted, amends 0016, amended 2026-09-14 |
| [0023](0023-redb-default-node-storage.md) | Native node persistence uses redb; memory remains for tests and wasm | Accepted, amends 0010 |
| [0024](0024-composer-drafts-echo-locally-and-flush-last-writer-wins.md) | Composer drafts echo locally and flush last-writer-wins | Accepted, amends 0018 |
| [0025](0025-capability-descriptors-are-typed-service-markers.md) | Capability descriptors are typed service markers | Accepted, amends 0008, amended 2026-09-14 |
| [0026](0026-mise-orchestrates-with-native-build-systems.md) | mise orchestrates the task graph; ecosystems keep their build systems | Accepted |

The records that shaped the first vertical slice live in git history before the reset commit and are not authoritative.

## Lifecycle

Statuses are `proposed`, `accepted`, `deprecated`, or `superseded by ADR-NNNN`. A decision that reverses an earlier one adds a new record and links back rather than editing history. A record whose mechanism changed without its decision changing keeps its status and gains a dated amendment section instead, which is what "amended 2026-09-14" means in the table above.
