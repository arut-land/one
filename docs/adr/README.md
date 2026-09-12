# Architecture Decision Records

ADRs record decisions that are hard to reverse, surprising without context, and the result of a real trade-off. `ARCHITECTURE.md` is the consolidated design; these records preserve why its boundaries are where they are. Product vocabulary is in `CONTEXT.md`, engineering vocabulary in `docs/GLOSSARY.md`.

| ADR | Decision | Status |
| --- | --- | --- |
| [0001](0001-conversation-authority-on-creating-node.md) | A conversation is authoritative on the node that created it | Accepted |
| [0002](0002-backend-is-relay-not-truth.md) | The backend pairs, relays, and stores encrypted backups; never the source of truth | Accepted |
| [0003](0003-device-keys-and-end-to-end-encryption.md) | Device keys, linked-device root key, end-to-end encryption from v1 | Accepted |
| [0004](0004-commands-are-carried-envelopes.md) | Commands are signed envelopes any paired device may carry | Accepted |
| [0005](0005-sans-io-machines-and-ports.md) | Sans-I/O machines, async ports, executor from the composition root | Accepted |
| [0006](0006-typed-scopes-not-a-service-container.md) | Typed scopes and capability bundles, no service container | Accepted |
| [0007](0007-scope-handles-and-generated-bindings.md) | One observable handle per scope, generated bindings over BoltFFI | Accepted |
| [0008](0008-generated-service-layer-from-protobuf.md) | Service layer generated from Protobuf descriptors | Accepted |
| [0009](0009-connect-framing-over-rpc-channel.md) | Connect framing over `RpcChannel`; transports are adapters | Accepted |
| [0010](0010-storage-as-three-ports.md) | Storage is three narrow ports; directory tree first | Accepted |
| [0011](0011-hosting-mode-is-a-port.md) | Hosting mode is a port chosen per platform | Accepted |
| [0012](0012-single-route-with-health-failover.md) | One active route with health failover in v1 | Accepted |
| [0013](0013-gtk4-without-libadwaita-on-linux.md) | GTK4 without libadwaita on Linux | Accepted |
| [0014](0014-model-access-through-rig.md) | Model access through `rig` behind a port | Accepted |
| [0015](0015-protobuf-contracts-and-compatibility-window.md) | Protobuf contracts, additive evolution, two-minor window | Accepted |
| [0016](0016-typed-errors-no-strings-from-core.md) | Typed errors; surfaces own user-facing strings | Accepted |
| [0017](0017-license-split.md) | FSL for product crates, MIT/Apache-2.0 for substrates | Accepted |
| [0018](0018-drafts-are-ephemeral-last-writer-wins.md) | Drafts are ephemeral, last-writer-wins, not facts | Accepted |

The records that shaped the first vertical slice are kept under `superseded/` for history. Their numbering overlaps this set and they are not authoritative.

## Lifecycle

Statuses are `proposed`, `accepted`, `deprecated`, or `superseded by ADR-NNNN`. A decision that reverses an earlier one adds a new record and links back rather than editing history.
