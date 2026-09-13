# Roadmap

Phases are ordered by dependency. Each has an exit criterion that is observable, not a date. Vocabulary follows `CONTEXT.md` and `docs/GLOSSARY.md`.

## Phase 0: Foundation

The implemented foundation and its remaining platform work:

1. **Watch substrate.** Revisioned `tokio::sync::watch`, compiled with `sync` only. Invalidations coalesce and unchanged writes do not advance the revision.
2. **Spawner and Host ports.** Runtime-owned executors, a host-polled executor, and a child-process daemon over IPC. Linux uses the child; native Apple/Android hosting and platform callback integration remain open.
3. **Typed scopes.** `Node<R>` and `Workspace<R>` own cancellation over supplied `ChatClients`. Conversation and operation ownership structs belong to Phase 2.
4. **Scope handles and bindings.** Projection types declare their FFI data once. BoltFFI exports explicit handles; generated Swift, Kotlin, and C# facades sit above per-language observation adapters. GTK consumes Rust handles directly.
5. **Connect framing and streaming.** Registry, HTTP, and IPC channels share conformance. `WatchComposer` is a server stream; request streaming is unsupported by HTTP/IPC. The WebSocket route remains Phase 5 work.
6. **Fact log and storage ports.** `FactLog`, `BlobStore`, and `KeyValue`; redb is the native node default under ADR 0023, with memory for tests and wasm. Directory storage and its selector are removed. Persistent blobs remain Phase 1 work.
7. **Generic authority and projections.** One transaction per decision, canonical command IDs, pure transcript reduction, and authority-clock timestamps preserved through Protobuf and FFI. Drafts recover through `KeyValue`, not transcript facts.
8. **Tooling and conformance.** One `arut-dev` binary owns `i18n`, `layers`, and `bindings` checks; `tools/conformance` remains a test crate. Browser callbacks are a workspace package. CI cancels superseded runs and filters Android builds by their inputs.

Exit: the existing chat behavior runs on Linux, macOS, and Android through these ports, and the web surface receives updates. Android and macOS still need on-device verification.

- [x] `mise run check`: 116 nextest tests, doctests, Rust/wasm/TypeScript, protocol compatibility, localization, binding, dependency, and layer checks pass.
- [x] `mise run build`: GTK, `arutd`, and web/Chromium/VS Code bundles build.
- [x] GTK over child IPC verifies sending, independent drafts, and preserved widgets; generated wasm verifies draft/start/send/list, ranges, and timestamps; conformance passes.
- [ ] Regenerate and compile Swift, Kotlin, and C# bindings with `accepted_at_ms`; verify native apps on their platforms.
- [ ] Complete persistent native hosting and host-polled platform callbacks; conversation/operation ownership structs remain Phase 2 work.

## Open decisions from the 2026-09-13 review

Recorded here so the code they touch is not changed speculatively. Each names the ADR or phase that settles it.

- **Draft write acknowledgement.** Replace futures each promise their own result, so per-keystroke writes queue behind one mutex and cannot be coalesced. Decide whether the composer offers a latest-draft setter plus a flush boundary before send, then coalesce. Settle with Phase 1 item 4 (drafts over gossip).
- **Retry outcome retention.** ADR 0004 promises retry deduplication with no expiry window, so command outcomes are never pruned. Redb is now the default and indexes command IDs, but retained outcomes still grow without bound. Decide the retention window before pruning.
- **Idle FFI observer release.** BoltFFI's event subscriptions expose no producer-side cancellation hook, so an unsubscribed observer is retained until its source changes. Contribute the hook upstream or adopt a generated subscription-owner API; do not add polling timers.
- **Session factory ABI.** The native and browser session constructors look interchangeable but embed hosting behavior. Settle with the native-hosting completion in Phase 0's remaining work (ADR 0011) before changing the exported ABI.
- **BoltFFI npm packaging defect.** The generated package file list omits the Node loader; harmless while the package is unpublished. Fix upstream before publishing.

## Phase 1: Daily driver (v1)

The scope in `docs/PRD.md`.

1. Add `substrates/identity` for an iroh endpoint per node (ADR 0019): the endpoint key is the device key; pairing by QR needs no PAKE, pairing by short code runs SPAKE2 before revealing identity; the root key transfers inside that channel; `keyring` with per-platform store crates holds desktop secrets, mobile secrets stay native behind FFI.
2. Add `transports/iroh` for `RpcChannel` over an iroh bi-directional stream; the relay is a deployed `iroh-relay` plus a small pairing service. Direct, LAN, and relayed routes come from iroh.
3. Peer-assisted store-and-forward: envelopes sealed for the target node, carried by any paired device, delivered over iroh when the target is reachable.
4. Drafts and presence over `iroh-gossip`, last-writer-wins on revision; the composer service becomes a local projection fed by gossip.
5. Streaming model responses through a `ModelProvider` implemented with `rig` against an OpenAI-compatible endpoint; replicated as a resumable stream with cursors.
6. Parallel operations per node with a small scheduler.
7. Persistent attachment storage and transfer through `iroh-blobs` behind `BlobStore`, with previews generated on the executing node. BLAKE3 addressing is already implemented; only the memory blob store exists. Redb fact-log and draft storage are complete.
8. Node picker on the empty composer.
9. Encrypted backup and restore through the relay's store, sealed under the root key.
10. Linux relm4 composition is complete (ADR 0020). Complete native hosting for the existing SwiftUI macOS and Compose Android surfaces, including the Android foreground service.
11. Capability manifest negotiated at session connect; two-minor-version window enforced. Config chain through `figment`.
12. Complete: English Fluent resources and typed accessors for every surface (ADR 0022), generated by `arut-dev i18n` and verified in `mise run check`. Native platform compilation remains part of the release exit.

Exit: every v1 scenario passes on real devices; the author uses it daily.

## Phase 2: Hand-off and harness

1. Add typed conversation and operation ownership structs. Build hand-off on the existing fact-log snapshots and checkpoints; explicit "continue on another node"; authority epoch advances for real.
2. The `Harness` port with a mock implementation; tools and approvals as facts; approval accepted from any surface with typed stale results.
3. First external harness through the Agent Client Protocol (ACP), whose session updates and permission requests map onto streamed facts and any-surface approvals; tools through MCP via `rmcp` with its transport riding an existing channel. The harness port mirrors the model port: one event stream plus out-of-band resolve and cancel; approvals are in-band events so no surface owns the answer. Pin `schemars` to one major first.
4. Automatic failover policy for cloud-coordinated conversations. Local-only failover stays explicit.

Candidate for the tool execution plane: iii (iii-hq/iii), a Rust orchestration engine with workers in Node, Python, Rust, and Go, triggers, queues, an exec worker, and built-in tracing. It does not belong in the node core (server daemon, JSON over WebSocket, no mobile or wasm), but it may host tools behind the harness port with MCP as the open protocol. Spike criteria before adopting: tools callable from the harness port through a typed adapter with streaming output; the engine runs beside the cloud node and optionally beside a desktop daemon with no change to surfaces; traces flow into our OpenTelemetry pipeline; the Elastic License 2.0 engine is compatible with on-premises packaging.

Exit: a mock tool can start, stream progress, request approval, complete, and resume after explicit hand-off over memory, IPC, and relay routes.

## Phase 3: Backend accounts

1. Sign-in through OAuth; device keys vouched for by the account.
2. Root key escrow under the account for recovery.
3. Push notifications through APNs and FCM.
4. Cloud node: the daemon deployed as a paired device on managed infrastructure; provider key broker so keys never reach a phone.
5. Evaluate iii for the pairing service, cron, queues, and the backend-hosted surfaces of Phase 4 (Slack, GitHub, email), where its HTTP triggers make each surface a small worker.

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

Most of what this phase once held is iroh's job (ADR 0019). What remains ours:

1. Bluetooth for nearby devices without a network, if a watch or offline scenario demands it.
2. Per-feature route preferences on top of iroh's connection choice: gossip for drafts, streams for facts, relay store for backups.
3. Browser reach: the web surface talks to a node through the relay's WebSocket path.

Exit: routes change under load with no visible effect on any surface.

## Phase 6: Organizations

Sharing, permissions, tenant policy, audit, on-premises backend, rich text composer as its own protocol version.

## Standing rules

- A phase does not start until the previous phase's exit criterion holds.
- Anything that adds a hand-written binding, a per-language view-model, or a feature-specific transport is a regression, not progress.
- The two-week announcement delay applies to writing about the work, never to the code.
