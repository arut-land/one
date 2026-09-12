---
status: accepted
---

# Protobuf defines every boundary contract; evolution is additive within a two-minor-version window

Components update independently across process, device, and language boundaries. We decided that Protobuf defines every value that crosses a boundary (commands, facts, snapshots, stream cursors, service contracts, manifests), that generated Rust types are reused in-process where they fit and handwritten types are kept where they enforce invariants, and that evolution within a package version is additive with removed numbers reserved. A conceptual break starts a new package version with explicit translation. A session and a node agree to talk when they are within two minor versions; outside that, the manifest negotiation returns a typed unsupported result.

## Consequences

- One wire definition serves in-process, IPC, LAN, relay, and backup.
- Compatibility tests against the previous two minors are required before a release.
- Unbounded compatibility is not promised because it cannot be tested.
