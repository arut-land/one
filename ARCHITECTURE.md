# Architecture

Arut shares Rust product behavior, protocols, and projections across native surfaces. Surfaces own presentation, lifecycle, hosting, and packaging. Product vocabulary is in `CONTEXT.md`; implementation terms are in `docs/GLOSSARY.md`.

This document distinguishes the current chat implementation from the accepted target design. `docs/ROADMAP.md` records the remaining work. ADRs preserve decisions and their amendments; they are not implementation status reports.

## Principles

1. Each surface follows its platform's presentation and lifecycle conventions.
2. Features own commands, facts, projections, and acceptance rules. Surfaces render typed values.
3. A conversation has one authority. Changing its route does not move execution.
4. Local operation does not require a backend. The planned backend carries encrypted data and never owns conversation truth.
5. Concrete port bundles prove local composition; service descriptors describe remote capabilities.
6. Add an abstraction or directory only when an implementation needs it.

## Layers and dependency rules

The current directories and the two Phase 1 additions are:

```text
surfaces/              views and deployment composition roots
bindings/              FFI exports and per-language observation adapters
product/               sessions, node/workspace scopes, localization
features/chat/         chat acceptance, clients, projections, composer, ports
substrates/authority/  generic command acceptance and projection reduction
substrates/storage/    FactLog, BlobStore, KeyValue; memory and native redb
substrates/watch/      revisioned watch values and subscriptions
substrates/identity/   Phase 1: iroh identity and pairing integration
protocols/             Protobuf contracts, Rust generator, RPC vocabulary
transports/            Connect HTTP and Unix IPC channels
transports/iroh/       Phase 1: RpcChannel over iroh streams
runtimes/              local native drivers, host-polled memory ports, browser callbacks
tools/                 development commands and shared conformance suites
```

Features depend on substrates and protocols. Product depends on features, substrates, protocols, and other product crates. Rust surfaces consume product clients directly; foreign surfaces import their binding package. Composition roots select runtimes. Transport implementations depend on RPC contracts, not product behavior. A binding does not select a host mode on behalf of a view.

`arut-dev layers --check` reads Cargo metadata, including optional, target-specific, build, and development edges. It rejects forbidden dependencies, Tokio executor features and wasm-bindgen in isolated core graphs, and weakened `unsafe_code` lints. Parsed Rust source also rejects feature `ServiceImpl` names in product and runtimes and `Authority` names in runtimes, including aliases and macro bodies. Tests are checked too.

Four exceptions are recorded in the tool: the chat feature's i18n derive macro, IPC's reuse of Connect framing, the FFI factory's host-polled runtime dependency, and the FFI crate's test-only `futures-executor`. `arut-dev bindings --check` verifies native export facades, rejects direct generated FFI imports outside bindings, and rejects relative TypeScript source paths that escape a surface or runtime package.

## Scopes

The structs in `product/session/src/scopes.rs` are:

```rust
pub struct Node<R> {
    runtime: Arc<R>,
    id: String,
    cancellation: Cancellation,
}
pub struct Workspace<R> {
    node: Arc<Node<R>>,
    id: String,
    cancellation: Cancellation,
}
```

`Node::workspace` constructs a workspace and gives it child cancellation. Product sessions use `Workspace<ChatClients>` directly. A workspace keeps its node alive and creates cancellation scopes for conversation clients.

Conversation and operation ownership structs are Phase 2 work. Today's `ChatClient` is an observable conversation handle, not a `Conversation` scope struct. The global/node/workspace/conversation configuration chain is also a target; no `ConfigChain` or `NodeRuntime` type exists.

## Ports and composition

The chat feature requires four ports in `features/chat/src/ports.rs`:

- `IdSource::new_id() -> String` supplies identities.
- `Persist<F>::log(namespace)` opens a named fact log.
- `Drafts::drafts()` supplies local key-value recovery storage.
- `Clock::now() -> u64` supplies Unix milliseconds.

`ChatRuntime` combines these ports with `Send + Sync + 'static`. `compose(Arc<R>)` constructs private chat and composer authorities and services. `ChatFeature` exposes generated clients, erased routers, and registrations derived from those routers' descriptors. A runtime without the required ports cannot compose chat.

`LocalRuntime` supplies native UUIDv7 IDs, wall time, redb logs, draft storage, and the node lease. `arutd` composes chat and passes its routers to `Node::serve`. This runtime assembly type is distinct from the product's ownership scope `Node<R>`. `Node::serve` registers routers and the derived capability service. Its small `Blocking` adapter runs dispatch construction and future polling through `spawn_blocking`, retaining the runtime during active calls. Redb's synchronous transactions still need that executor isolation.

`MemoryRuntime` supplies named memory logs, draft storage, injected IDs, and an injected clock. It contains no feature list. Core tests use the feature's `test_support::MemoryPorts` and `TestIds` through the production composition function, without a product-to-runtime development dependency.

`ProductSession::from_chat` consumes a composed local feature and derives its capability client. `ProductSession::remote` binds chat, composer, and capability clients to the root's chosen channel. The session constructs its pending chat through the same helper used for subsequent pending chats. FFI factories compose chat over memory ports and call `from_chat`; the Linux root calls `remote` with its child-process channel.

## Acceptance, facts, and recovery

`Command` declares its scope, fact, projection, rejection, epoch, precondition, optional expiry, and pure `apply(current, now)` function. `Authority<C>` serializes acceptance through `FactLog::commit`. The transaction refreshes the projection, checks the retry outcome, validates the command, and optionally appends one record. Outcomes distinguish applied, duplicate, revision conflict, authority mismatch, superseded, and rejected commands.

Chat invokes `execute_with_clock`. The authority reads the supplied clock inside the transaction after deduplication. The accepted `ChatFact` batch and its transcript messages carry `accepted_at_ms`. `ChatMessage` in the projection and generated FFI bindings preserves that timestamp. The Protobuf additions are field 6 on `ChatFact` and field 4 on `ChatMessage`; older rows decode with zero. A retry returns the original timestamp.

Each accepted mock exchange records a user message, an echo response, and started/completed operation facts. Records also carry sequence, authority epoch, and command ID. Chat currently uses epoch 1. Canonical UUIDv7 command IDs deduplicate starts and sends. Operation execution, model streaming, replication, and authority hand-off are not implemented.

`ChatProjection` is an internal pure reducer. Authority startup replays facts after an optional snapshot. Explicit checkpoint and compaction methods exist; the daemon does not schedule maintenance automatically. Retry outcomes survive compaction and currently have no expiry.

Drafts are ephemeral values with local `KeyValue` recovery, not transcript facts. Composer edits check revision and epoch. A start fact records the pending revision it consumed, so recovery can finish draft cleanup after a failure without erasing later edits. Cross-device last-writer-wins replication and remote-editing indicators remain Phase 1 work under ADR 0018.

## Storage

ADR [0023](docs/adr/0023-redb-default-node-storage.md) amends [0010](docs/adr/0010-storage-as-three-ports.md). The ports remain `FactLog`, `BlobStore`, and `KeyValue`. Native nodes use redb for facts and key-value data. The directory implementation and `NodeStorage` selector are gone; `ARUT_STORAGE` no longer selects a store.

The primary feature namespace uses `node.redb`; other namespaces use `log-<BLAKE3 digest>.redb`. Redb owns each database exclusively. `node.lock` prevents two runtimes from owning the same node directory. Transactions maintain sequence and command indexes, snapshots, retained retry outcomes, and a schema-version table. Existing redb files remain readable. Directory data is not automatically migrated.

Memory implements all three ports for tests and wasm. Redb is an opt-in Cargo feature enabled by the local runtime, not a wasm dependency. No persistent `BlobStore` remains; Phase 1 adds attachment storage and transfer through `iroh-blobs`. Current memory blob addresses use BLAKE3 and verify content on reads.

## Observation and bindings

`Watch<T>` wraps `tokio::sync::watch` with Tokio's `sync` feature only. Updates compare a detached candidate and increment the revision only when the value changes. New subscribers receive an initial invalidation; intervening updates may coalesce. Dropping the writer closes subscriptions.

Product sessions expose conversation summaries and composer availability. Chat and composer handles expose separate watches. Transcript metadata contains `last_message_id`; `messages_after(id)` returns immutable keyed rows strictly after that cursor. Surfaces cache rows and fetch only additions. Connectivity and operation handles are targets, not current exports.

Projection types declare `#[boltffi::data]` in their owning crate and are re-exported by `bindings/ffi`. Explicit `#[export]` blocks expose handles and callback streams. BoltFFI's source scanner does not expand export macros, so these blocks remain explicit. `arut-dev bindings` generates Swift/Kotlin aliases and factory forwarding, plus C# source aliases. Observation adapters remain hand-written per ADR 0021. GTK consumes product types and watches directly.

FFI observation uses a host-polled callback driver. The standalone `HostPolledSpawner` implements `LocalSpawner` through `async_executor::LocalExecutor` and bounded ticks; platform callbacks do not yet drive it. Idle unsubscribed FFI observers may remain retained until the next source change, an open roadmap decision.

## Protocols and transport

`protocols/proto/arut` contains capability, chat, and nested composer packages. `protox` compiles descriptors; `arut-protocol-build` generates Rust service traits, direct and remote clients, routers, procedure names, and descriptors. Direct calls pass typed values. Remote calls encode at `RpcChannel`. The generator supports all four streaming shapes.

The registry supplies an in-process channel. Connect HTTP and Unix IPC support unary calls and server streams; request-streaming calls return `Unimplemented`. Connect framing bounds messages to 8 MiB and preserves typed status codes and details. IPC reuses the HTTP framing with a Unix-socket connector. No WebSocket or iroh channel exists yet.

The capability service derives its manifest from registrations. Product maps composer presence, current availability, and manifest failures into `FeatureAvailability`. Per-service availability types are not generated, and capabilities do not yet stream. ADR 0015's two-minor compatibility window remains Phase 1 work. `buf breaking` checks additive schema evolution against local `master`, not negotiated runtime compatibility.

## Hosting and browser composition

`Host` and `HostMode` describe in-process, child-process, system-service, and remote hosting. `ChildHost` starts `arutd`, waits for `READY`, and retains the child with its channel. `ScheduledChannel` dispatches through a supplied spawner. Executor creation lives in the local runtime and the daemon root; GTK polls UI work on GLib.

| Surface | Current composition | Remaining target |
| --- | --- | --- |
| Linux | relm4/GTK4 root, child `arutd`, Unix IPC, persistent redb | v1 features and device routes |
| Apple | SwiftUI, in-process memory FFI session | macOS child hosting; iOS lifecycle support |
| Android | Compose, ViewModel-owned memory FFI session | foreground service and persistent node |
| Windows | WinUI, in-process memory FFI session | child hosting and persistent node |
| Web, Chromium | wasm memory session in the page | worker/lifecycle integration and remote routes |
| VS Code | wasm session in the extension host, webview messages | native extension-host composition or daemon route |

`runtimes/browser` is the pnpm package `@arut/runtime-browser`. Web and VS Code import its UUIDv7 callback through the package export and supply wall time to the binding. Chromium imports the web composition entry through `@arut/surface-web/main`. This keeps runtime selection in composition roots while making package dependencies explicit. Relative TypeScript paths cannot escape surface or runtime packages.

Terminal, JetBrains, watch, and messaging surfaces have no directories yet.

## Localization

`product/i18n/locales/en` is the current Fluent source. `arut-dev i18n` generates native resources, typed accessors, and Fluent copies for web/editor surfaces. GTK uses Rust `Localizer`; TypeScript uses Fluent bundles; Apple, Android, and Windows use native resources. Surface error mappings select typed accessors; product transitions return typed errors rather than sentences.

`Localized` derives message keys and validates them against the English source at compile time. Locale completeness tests and generator checks detect missing or stale output. Adding a locale requires its files and registration in `product/i18n/src/lib.rs`. Locale choice stays outside the feature core.

## Accepted targets beyond the current slice

Phase 1 adds iroh endpoints, pairing, device keys, sealed envelopes, peer-assisted delivery, gossip drafts, blob transfer, a `ModelProvider` implemented with `rig`, and a `figment` configuration chain. ADR 0019 assigns connection discovery and direct/relay failover to iroh; Arut does not build a routing substrate. Keychain storage, root-key derivation, encryption, and backups remain unimplemented. Current local IPC permissions do not provide the planned device trust protocol.

Phase 2 adds conversation/operation ownership structs, explicit hand-off, harnesses, tools, and approvals. ADR 0005's reusable `Machine`/`Effect` interface remains a target. Today's authority calls synchronous storage ports; there is no generic machine trait or effect driver. Phase 3 adds backend accounts, recovery, push, and hosted nodes. The backend has no directory yet.

`tracing` spans exist for RPC, authority, and composer activity. There is no OpenTelemetry exporter, OpenFeature integration, or feature-flag provider. Those remain target integrations. Content, prompts, and payloads must not enter telemetry without consent.

## Repository layout

The workspace contains 18 Rust crates. The inventory below lists every crate and surface root; implementation files, resources, and generated build output are omitted.

```text
/
|-- .github/workflows/ci.yml
|-- CONTEXT.md, ARCHITECTURE.md
|-- Cargo.toml, Cargo.lock, deny.toml
|-- mise.toml, mise.lock, buf.yaml
|-- package.json, pnpm-workspace.yaml, pnpm-lock.yaml, tsconfig.json
|-- .gitignore, LICENSE-APACHE, LICENSE-MIT, LICENSE-FSL
|-- docs/{PRD,ROADMAP,GLOSSARY,ECOSYSTEM}.md
|   `-- adr/                       numbered decisions and index.md
|-- protocols/                     arut-protocol
|   |-- proto/arut/capability/v1/
|   |-- proto/arut/chat/v1/
|   |-- proto/arut/chat/composer/v1/
|   |-- build/                      arut-protocol-build
|   `-- rpc/                        arut-rpc
|-- substrates/
|   |-- authority/                  arut-authority
|   |-- storage/                    arut-storage
|   `-- watch/                      arut-watch
|-- features/chat/                  arut-feature-chat: commands, projections, drafts
|-- product/
|   |-- session/                    arut-product-session
|   `-- i18n/                       arut-i18n
|       `-- macros/                 arut-i18n-macros
|-- transports/
|   |-- connect-http/               arut-transport-connect-http
|   `-- ipc/                        arut-transport-ipc
|-- runtimes/
|   |-- local/                      arut-runtime-local, arutd
|   |-- host-polled/                arut-runtime-host-polled
|   `-- browser/                    @arut/runtime-browser
|-- bindings/
|   |-- ffi/                        arut_ffi
|   |-- swift/, kotlin/, dotnet/
|   `-- typescript/                 @arut/bindings-typescript
|-- surfaces/
|   |-- linux/                      arut-linux
|   |-- apple/shared/               Swift package and app sources
|   |-- android/, windows/
|   `-- web/, chromium/, vscode/
`-- tools/
    |-- dev/                        arut-dev: i18n, layers, bindings
    `-- conformance/                arut-conformance: shared port tests
```

Foreign generated packages live under ignored `bindings/generated`. No README files are maintained. Rust crates document their contracts with `//!` comments. Do not add generic `shared`, `common`, `utils`, or `services` buckets; `surfaces/apple/shared` is the existing package shared by Apple app targets.

## Tooling and verification

Mise owns the pinned toolchains and task graph. Cargo builds Rust, pnpm builds and checks the TypeScript workspace, buf checks Protobuf, and BoltFFI packages foreign bindings. Gradle and XcodeGen are scoped to platform tasks. Machine-specific tuning belongs in ignored `mise.local.toml`.

`tools/dev` is one binary crate, `arut-dev`, with `i18n`, `layers`, and `bindings` subcommands. Each accepts `--check`; layers always checks without writing. Mise calls these commands, and CI calls the same mise tasks. Platform tasks own Android target installation and Apple binding generation. Localization resource emitters and typed accessors share platform key naming. `tools/conformance` remains a test crate.

`mise run check` runs formatting, Clippy, nextest, doctests, cargo-machete, cargo-deny, isolated wasm builds, buf lint/breaking, localization and binding generation checks, layer checks, and TypeScript checks. `mise run build` builds GTK, `arutd`, and web/Chromium/VS Code bundles. Both gates run before every commit with `CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4`.

CI cancels superseded runs per branch. Linux runs on every push and pull request; Android runs only when `surfaces/android`, `bindings/kotlin`, `bindings/ffi`, `features`, `product`, `substrates`, `protocols`, `runtimes`, or `Cargo.lock` changes. macOS builds and runs Swift binding tests on pull requests and version tags. Native Android and Windows compilation remains unverified on this Linux machine. Apple build and validation details are recorded in `docs/APPLE.md`.

The simplification pass verified 115 nextest tests, all doctests, both gates, and the separately invoked display-backed GTK chat test. Conformance covers registry/HTTP/IPC RPC, memory/redb logs and key-value storage, and memory blobs. The generated wasm package passes a Node smoke test for draft/start/send/list, transcript ranges, and timestamps; TypeScript checks pass. No Swift, Kotlin, or C# source changed in this pass. Native regeneration and compilation must still verify the additive transcript timestamp in those bindings.
