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

Status on 2026-09-13: the chat foundation passes the Rust, wasm, TypeScript, protocol, localization, and production-build gates. The cleanup added keyed transcript ranges across every surface, one fact-log transaction per authority decision, one candidate clone per watch update, and cancellation of composer followers without retry polling. Verified locally: GTK keyboard sending, independent drafts, and message-widget preservation; generated wasm start/send/list and transcript ranges under Node; all transport and storage conformance suites. Earlier runs exercised GTK against `arutd` and the web surface in a browser. Swift, Kotlin, and C# edits still require native compilation. Phase 0's platform exit is not yet verified: Android and macOS have not run here. The host-polled executor is implemented and tested as a port but is not wired into platform callbacks; native hosting and the remaining conversation/operation scope expansion still need completion.

## Open decisions from the 2026-09-13 review

Recorded here so the code they touch is not changed speculatively. Each names the ADR or phase that settles it.

- **Draft write acknowledgement.** Replace futures each promise their own result, so per-keystroke writes queue behind one mutex and cannot be coalesced. Decide whether the composer offers a latest-draft setter plus a flush boundary before send, then coalesce. Settle with Phase 1 item 4 (drafts over gossip).
- **Retry outcome retention.** ADR 0004 promises retry deduplication with no expiry window, so command outcomes are never pruned and the directory log scans the whole live log per commit. Decide the retention window before pruning; use the redb store for sustained workloads meanwhile.
- **Idle FFI observer release.** BoltFFI's event subscriptions expose no producer-side cancellation hook, so an unsubscribed observer is retained until its source changes. Contribute the hook upstream or adopt a generated subscription-owner API; do not add polling timers.
- **Session factory ABI.** The native and browser session constructors look interchangeable but embed hosting behavior. Settle with the native-hosting completion in Phase 0's remaining work (ADR 0011) before changing the exported ABI.
- **BoltFFI npm packaging defect.** The generated package file list omits the Node loader; harmless while the package is unpublished. Fix upstream before publishing.

## Phase 1: Daily driver (v1)

The scope in `docs/PRD.md`.

1. iroh endpoint per node (ADR 0019): the endpoint key is the device key; pairing by QR needs no PAKE, pairing by short code runs SPAKE2 before revealing identity; the root key transfers inside that channel; `keyring` with per-platform store crates holds desktop secrets, mobile secrets stay native behind FFI.
2. `RpcChannel` over an iroh bi-directional stream; the relay is a deployed `iroh-relay` plus a small pairing service. Direct, LAN, and relayed routes come from iroh.
3. Peer-assisted store-and-forward: envelopes sealed for the target node, carried by any paired device, delivered over iroh when the target is reachable.
4. Drafts and presence over `iroh-gossip`, last-writer-wins on revision; the composer service becomes a local projection fed by gossip.
5. Streaming model responses through a `ModelProvider` implemented with `rig` against an OpenAI-compatible endpoint; replicated as a resumable stream with cursors.
6. Parallel operations per node with a small scheduler.
7. Attachments as content-addressed blobs with previews generated on the executing node; transfer through `iroh-blobs` behind the `BlobStore` port.
8. Node picker on the empty composer.
9. Encrypted backup and restore through the relay's store, sealed under the root key.
10. Linux surface rewritten with relm4 (ADR 0020); SwiftUI macOS surface; Compose Android surface with the core in a foreground service.
11. Capability manifest negotiated at session connect; two-minor-version window enforced. Config chain through `figment`.
12. Strings from one Fluent source generated into native resources (ADR 0022); English only, with the key-completeness test and the stale-resource check in `mise run check`.

Exit: every v1 scenario passes on real devices; the author uses it daily.

## Phase 2: Hand-off and harness

1. Checkpoints and snapshots of conversation state; explicit "continue on another node"; authority epoch advances for real.
2. The `Harness` port with a mock implementation; tools and approvals as facts; approval accepted from any surface with typed stale results.
3. First external harness through the Agent Client Protocol (ACP), whose session updates and permission requests map onto streamed facts and any-surface approvals; tools through MCP via `rmcp` with its transport riding an existing channel. The harness port mirrors the model port: one event stream plus out-of-band resolve and cancel; approvals are in-band events so no surface owns the answer. Pin `schemars` to one major first.
4. Automatic failover policy for cloud-coordinated conversations. Local-only failover stays explicit.

Candidate for the tool execution plane: iii (iii-hq/iii), a Rust orchestration engine with workers in Node, Python, Rust, and Go, triggers, queues, an exec worker, and built-in tracing. It does not belong in the node core (server daemon, JSON over WebSocket, no mobile or wasm), but it may host tools behind the harness port with MCP as the open protocol. Spike criteria before adopting: tools callable from the harness port through a typed adapter with streaming output; the engine runs beside the cloud node and optionally beside a desktop daemon with no change to surfaces; traces flow into our OpenTelemetry pipeline; the Elastic License 2.0 engine is compatible with on-premises packaging.

Exit: the eleven-step mock-tool slice from the original architecture runs over memory, IPC, and relay routes.

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
