# Roadmap

Phases are ordered by dependency. Each has an exit criterion that is observable, not a date. Vocabulary follows `CONTEXT.md` and `docs/GLOSSARY.md`.

## Phase 0: Foundation

Replace the substrates the current chat slice sits on so that everything after it deletes code instead of adding it.

1. **Watch substrate.** Newtype over `tokio::sync::watch` (sync feature only). Delete the Condvar queue, the FFI bridge thread, and the Qt observer thread. Verify invalidations arrive on wasm.
2. **Spawner and Host ports.** Executor supplied by the composition root. Remove the global runtime from the HTTP transport. Child-process daemon (`arutd`) with an IPC transport on Linux and macOS.
3. **Typed scopes.** Node, workspace, conversation, operation as structs; capability bundles as supertraits; delete the pending `OnceLock` back-reference pattern.
4. **Scope handles and generated bindings.** One handle per scope with its own watch. FFI data attributes on projection types. Binding templates for Swift, Kotlin, and GTK. Delete the five ChatModels.
5. **Connect framing and streaming.** Memory, IPC, and WebSocket transports behind one framing. `WatchComposer` becomes a server stream. Delete the polling loops.
6. **Fact log and storage ports.** `FactLog`, `BlobStore`, `KeyValue` with a directory-and-JSON implementation. Messages and operation lifecycle as facts; drafts stay ephemeral per ADR 0018 and recover locally through `KeyValue`. `SendMessage` gains a command ID. Delete the checkpoint rewrite.
7. **Generic authority and projections.** `Authority<C: Command>` lifted from the composer; transcript and draft as pure projections.
8. **Conformance suites.** One suite each for `RpcChannel`, `FactLog`, `BlobStore`. CI on Linux and Android per commit, macOS on tag.

Exit: the existing chat behavior runs on Linux, macOS, and Android through the new substrates with no per-language view-model, and the web surface receives updates.

Status on 2026-09-12: all eight steps are implemented and gated. Verified on real runs: Linux GTK against a spawned `arutd` over the Unix socket, the web surface in a browser, wasm invalidation delivery under Node, and every Rust and TypeScript gate. Not yet run: Android and macOS, which need their toolchains; a `BlobStore` conformance suite; sending through the GTK window. IDs come through an `IdSource` port because the wasm core has no clock or entropy; the browser supplies UUIDv7.

## Phase 1: Daily driver (v1)

The scope in `docs/PRD.md`.

1. Device keys, pairing by QR and code, root key transfer, envelope encryption.
2. LAN route with mDNS discovery; relay route through the backend; health-scored failover.
3. Peer-assisted store-and-forward: envelopes carried by any paired device.
4. Streaming model responses through a `ModelProvider` implemented with `rig` against an OpenAI-compatible endpoint; replicated as a resumable stream with cursors.
5. Parallel operations per node with a small scheduler.
6. Attachments as content-addressed blobs with previews generated on the executing node.
7. Node picker on the empty composer.
8. Encrypted backup and restore through the backend.
9. GTK4 Linux surface replacing Qt; SwiftUI macOS surface; Compose Android surface with the core in a foreground service.
10. Capability manifest negotiated at session connect; two-minor-version window enforced.

Exit: every v1 scenario passes on real devices; the author uses it daily.

## Phase 2: Hand-off and harness

1. Checkpoints and snapshots of conversation state; explicit "continue on another node"; authority epoch advances for real.
2. The `Harness` port with a mock implementation; tools and approvals as facts; approval accepted from any surface with typed stale results.
3. First external harness through an open agent protocol; tools through MCP.
4. Automatic failover policy for cloud-coordinated conversations. Local-only failover stays explicit.

Exit: the eleven-step mock-tool slice from the original architecture runs over memory, IPC, and relay routes.

## Phase 3: Backend accounts

1. Sign-in through OAuth; device keys vouched for by the account.
2. Root key escrow under the account for recovery.
3. Push notifications through APNs and FCM.
4. Cloud node: the daemon deployed as a paired device on managed infrastructure; provider key broker so keys never reach a phone.

Exit: a new device recovers everything from an account with no other device present.

## Phase 4: More surfaces

Each is a client of a node and holds no product logic.

1. iOS and iPadOS from the shared Apple code.
2. Windows, WinUI 3.
3. Terminal: inline CLI with optional alternate-screen TUI.
4. VS Code: core in the extension host, webview UI over a transport; or the Chat Participant API.
5. JetBrains: pure JVM client of the local daemon.
6. Browser extensions (Chromium, Firefox, Safari): web-like, connected to nodes over routes.
7. watchOS and Wear OS: pure clients of the phone.
8. Messaging integrations as backend-hosted surfaces: Slack, GitHub, Teams, email.

Exit criterion per surface: passes the v1 scenarios it can express, with lifecycle and theming native to the host.

## Phase 5: Routes

1. WebRTC data channels with signaling through the backend.
2. Bluetooth for nearby devices.
3. Parallel probing; best route active, others warm.
4. Per-feature route selection: drafts over nearby routes, facts over the best available, backups over relay.

Exit: routes change under load with no visible effect on any surface.

## Phase 6: Organizations

Sharing, permissions, tenant policy, audit, on-premises backend, rich text composer as its own protocol version.

## Standing rules

- A phase does not start until the previous phase's exit criterion holds.
- Anything that adds a hand-written binding, a per-language view-model, or a feature-specific transport is a regression, not progress.
- The two-week announcement delay applies to writing about the work, never to the code.
