# Architecture

Arut is one product presented through native surfaces on every device a person owns and executed on whichever of their nodes they choose. Product behavior, protocols, and projections are shared Rust. Presentation, lifecycle, hosting, and packaging are native to each platform.

This document is the target design. Where the repository differs from it today, `docs/ROADMAP.md` says in what order it converges. Product vocabulary is in `CONTEXT.md`; engineering vocabulary is in `docs/GLOSSARY.md`; the reasons behind each boundary are in `docs/adr/`.

## Principles

1. Every surface behaves as if its platform's own developers built it. Nothing is forced across platforms.
2. One product, one truth. Conversations, drafts, operations, and availability are identical on every device.
3. Local first. Everything works with no backend; the backend adds reach and recovery and never owns truth.
4. Execution location is chosen per conversation and changed only deliberately. Routes never move execution.
5. The compiler proves local composition. Manifests describe remote nodes. Nothing is assumed about a node it did not report.
6. Facts have one authority and one order. No CRDTs for authority-owned state.
7. A feature is one Rust crate. Bindings and surfaces gain it without hand-written glue.
8. Prefer a proven open standard or crate over anything written here. Substrates are thin.
9. Add a directory or abstraction only after a concrete implementation proves its boundary.

## System model

```text
person
  devices  ──pairing──  device keys, root key
  nodes    ──host──     laptop daemon, phone service, cloud daemon
  workspaces            resources, connectors, settings, per node
  conversations         one authority each, replicated to every node
  operations            model turns and tool calls, parallel per node
  surfaces  ──session── one live attachment to one node, many conversations shown
  routes                in-process, IPC, LAN, relay, later WebRTC and Bluetooth
  backend               pairing, relay, encrypted backup; optional
```

A conversation is created on a node, which becomes its authority (ADR 0001). Every other node of the person holds a replica and may queue and carry commands for it (ADR 0004). A surface attaches to one node through a session and may show conversations whose authorities are elsewhere; the node it is attached to forwards. Changing which route reaches a node changes nothing about which node executes.

## Ownership tiers

State and services have one of three owners. The tier decides where code lives, what is synchronized, and who may change it.

| Tier | Owns | Synchronized | Lives in |
| --- | --- | --- | --- |
| Product | identity, pairing, workspaces, conversation list, availability, connectivity, settings that follow the person | yes | `product/` |
| Feature | one feature's facts, projections, commands, ports, and rules; for chat: transcript, operations, composer | yes, as facts and ephemeral replication | `features/<name>/` |
| Surface | sidebar collapse, window geometry, focus, hover, open menus, last selected item, platform permissions | never; stored in the platform's own store | each surface |

Surface state may live in the binding wrapper or in the view; it never enters a scope handle. A surface may have features no other surface has (a collapsed rail, a command palette, a menu-bar extra) without any change below it.

## Layers and dependency rules

```text
surfaces/        native views and composition roots
bindings/        generated per-ecosystem observation over scope handles
product/         session, scopes, availability projections
features/        one crate per feature: commands, facts, projections, services, ports
substrates/      watch, authority, machine, log and storage ports, crypto, identity, routing
protocols/       Protobuf packages, generator, RPC types
transports/      RpcChannel implementations
runtimes/        port implementations and drivers per host: local, android, apple, browser, cloud
backend/         pairing, relay, encrypted store-and-forward
```

Allowed directions:

```text
surface -> binding -> product -> feature -> substrate
Rust surface (GTK, terminal) -> product directly
runtime -> feature + product + transport + substrate
transport -> protocols/rpc only
backend -> protocols + substrates + transports
generator -> protocols only
```

Forbidden directions:

```text
feature -> surface, binding, runtime, transport, or native SDK
product -> binding, surface, or generated foreign-language FFI
substrate -> feature or product
transport -> product semantics or capability policy
binding -> runtime selection
anything -> a global service locator
native view -> raw capability identifiers
```

## Scopes

Scopes are typed structs, not a container (ADR 0006).

```rust
pub struct Node<R: NodeRuntime>        { runtime: Arc<R>, identity: DeviceIdentity, workspaces: Watch<WorkspaceList>, .. }
pub struct Workspace<R: NodeRuntime>   { node: Arc<Node<R>>, id: WorkspaceId, config: ConfigChain, .. }
pub struct Conversation<R: ChatRuntime>{ workspace: Arc<Workspace<R>>, id: ConversationId, authority: AuthorityRef, .. }
pub struct Operation                   { conversation_id: ConversationId, id: OperationId, stream: StreamRef, .. }
```

A child holds an `Arc` to its parent and is constructed by the parent. Configuration resolves through the chain global → node → workspace → conversation, which is where skills, connectors, and provider settings attach at the level a person expects.

Feature dependencies are capability bundles:

```rust
pub trait Clock { fn now(&self) -> Timestamp; }
pub trait Persist<F> { type Log: FactLog<F>; fn log(&self) -> &Self::Log; }
pub trait ModelProvider { fn respond(&self, req: ModelRequest) -> BoxStream<'_, Result<Token, ModelError>>; }

pub trait ChatRuntime: Clock + Persist<ChatFact> + ModelProvider + Send + Sync + 'static {}
impl<R: Clock + Persist<ChatFact> + ModelProvider + Send + Sync + 'static> ChatRuntime for R {}
```

A runtime that lacks a port cannot construct that feature. That is the compile-time half of capabilities; the manifest is the runtime half.

## Reactivity and scope handles

Surfaces observe scope handles, one per scope instance, each with its own watch (ADR 0007):

```text
Conversations            list, ordering, unread, per workspace
Conversation(id)         transcript projection, status, typed error
Composer(scope)          draft text, attachments, revision, remote-editing indicator
Operation(id)            progress, stream cursor, outcome
Availability             per-service typed availability with reasons
Connectivity             route, health, paired nodes
```

The watch substrate is a newtype over `tokio::sync::watch` compiled with only the `sync` feature, so it runs on every target including wasm with no threads. Bindings coalesce invalidations and re-read on the native scheduler. Transcripts use a keyed collection with changed-range signals so a token does not re-marshal the whole conversation; Qt-style parallel string lists and joined transcript strings are not permitted.

Optimism is intent-specific and decided in Rust. Draft edits apply locally at once. Sending, approvals, authority changes, and irreversible operations wait for authoritative acceptance. Surfaces never choose consistency behavior.

## Commands, facts, and the log

Every mutation is a `Command` (ADR 0004):

```rust
pub trait Command: Send + 'static {
    type Scope: Hash + Eq + Clone;
    type Fact: Clone + Send;
    type Outcome: From<Applied<Self::Fact>>;
    fn command_id(&self) -> &CommandId;          // UUIDv7, chosen by the surface, kept through retries
    fn scope(&self) -> &Self::Scope;
    fn precondition(&self) -> Precondition;       // Revision(n) | Epoch(e) | OperationOpen(id) | None
    fn apply(self, current: &Projection<Self>) -> Result<Self::Fact, Self::Outcome>;
}
```

One generic `Authority<C, L: FactLog<C::Fact>>` performs dedup by command ID, epoch fencing, precondition checks, apply, and append. Its outcomes are typed: applied, duplicate, revision conflict, authority mismatch, superseded. Queueability, expiry, and optimistic policy are associated items on the command type so shared machinery acts on them without a switch.

Facts go to the `FactLog` with a sequence, an epoch, and the command ID. Projections are pure reducers rebuilt from facts and accelerated by snapshots. Compaction keeps the log bounded. Drafts are the one exception: they replicate as ephemeral state with last-writer-wins and are not facts (ADR 0018).

High-rate data (tokens, progress, later terminal bytes) uses resumable streams with a stable stream ID, monotonic offsets, bounded retention, and an explicit unavailable-range response. A consumer resumes from its cursor or installs the final durable result.

## Sans-I/O machines and drivers

Control flow that must be correct under reconnection and reordering is a machine (ADR 0005):

```rust
pub trait Machine {
    type Input;
    type Effect;
    fn handle(&mut self, input: Self::Input) -> SmallVec<[Self::Effect; 4]>;
    fn next_timeout(&self) -> Option<Instant>;
}
```

Machines: conversation authority, session negotiation (manifest, version window, epoch), stream resume, draft replication, route health. Drivers in `runtimes/` feed inputs and perform effects on the host executor. Tests drive machines with vectors and a fake clock. Effects with results (model call, file read, process spawn) are async ports, not machine effects.

Nothing outside `runtimes/` creates an executor. A `Spawner` port from the composition root runs futures: Tokio on desktops and the backend, a foreground-service thread on Android, the host's async on Apple through BoltFFI, and the page's event loop on wasm through BoltFFI's poll exports.

## Protocols and the service layer

Protobuf defines everything that crosses a boundary (ADR 0015). A project-owned generator emits, per service, an object-safe trait, a client with direct and remote targets, an erased router, canonical procedure names, descriptors, and typed availability (ADR 0008). Direct clients pass generated structs with no encoding. Remote clients encode exactly once at `RpcChannel`.

Evolution is additive within a package version. A session and a node talk when within two minor versions; otherwise negotiation returns a typed unsupported result.

## Transport and routes

`RpcChannel` is the only boundary a remote call crosses. Connect protocol framing sits on top of it for every HTTP-carried route (ADR 0009). Channels: memory (tests, in-process hosting), IPC pipe (child-process daemon), an iroh bi-directional stream between nodes, and the relay's WebSocket path for browsers. Layers around a channel attach and read metadata: device auth, authority epoch, W3C trace context, protocol version. Features never see a channel.

Between devices, connectivity is iroh (ADR 0019): each node is an iroh endpoint whose Ed25519 key is the device key; discovery, hole punching, relay fallback, and transport encryption are iroh's. Route selection is therefore not ours (ADR 0012 as amended). Per-feature preferences sit above it: drafts and presence travel over `iroh-gossip`, facts over streams, attachments over `iroh-blobs`, backups through the relay store.

Envelopes held at rest by a carrier or the relay are sealed under keys derived from the person's root key (ADR 0003). Any paired device or the relay may carry an envelope for an unreachable node and cannot read it (ADR 0002).

## Hosting

Hosting is a port (ADR 0011). The composition root of each surface picks a mode and hands the result to the binding:

| Platform | Mode | Notes |
| --- | --- | --- |
| Linux, Windows | child process (`arutd`) over IPC | system service later without code changes above the port |
| macOS | child process now, login item later | `SMAppService` when a task should outlive the window |
| Android | foreground service, background thread, in-app process | the phone is a full node |
| iOS, iPadOS | in-process | `BGTaskScheduler` for short background work |
| VS Code | core in the extension host, UI in a webview over a transport | no wasm in the extension |
| JetBrains | pure JVM client of the local daemon | no native code in the plugin |
| Web, browser extensions | wasm in the page or a worker | I/O through host callbacks; executes only what needs no OS access |
| Terminal | in-process or client of the daemon | inline CLI, optional alternate-screen TUI |
| Watches, messaging integrations | pure clients of a phone or the backend | no core |

## Storage

Three ports (ADR 0010): `FactLog`, `BlobStore`, `KeyValue`. The first implementation is a directory tree whose layout carries structure, JSON only for documents, raw bytes for blobs, append-and-fsync for the log. SQLite and platform stores plug in later per port. Nothing rewrites whole state on a change.

## Bindings and surfaces

Bindings are generated per ecosystem from scope handle definitions over BoltFFI plus a thin wrapper of ours that bridges watches to BoltFFI event subscriptions and fixes handle conventions. Projection types carry the FFI data attribute in the feature crate and are re-exported by the FFI crate, so each type is defined once. Bindings own coalescing, scheduler hops, and ecosystem cancellation. They own no product transitions. A hand-written binding is a regression.

Rust-owned surfaces (GTK, terminal) read projection types directly with no FFI.

Surfaces own rendering, navigation, disposable state, accessibility, platform permissions, lifecycle observation, and the composition root. They report platform facts (suspend, background expiry, connectivity) to shared policy and never contain authority, routing, or compatibility rules. The core returns typed outcomes and no text (ADR 0016).

Those outcomes become sentences through one source. Every user-facing string lives once in `product/i18n` as Fluent, and a typed variant names its message by convention through `message_key` on the enum itself, with variant fields as Fluent arguments (ADR 0022). Strings never cross the FFI boundary. Rust-owned surfaces and the wasm core read the `.ftl` directly through `arut-i18n`'s `Localizer`; web and editor surfaces read the same files with `@fluent/bundle`; native surfaces read `mise run i18n`'s output — `Localizable.xcstrings`, `values-<lang>/strings.xml`, `Strings/<lang>/Resources.resw` — through their own platform localization API, so each surface keeps its idiom and pays nothing at runtime. Locale selection stays platform-owned: a surface hands over the language list its platform already resolved, and the core never learns the locale. No message id is written by hand anywhere. Beside the resources, `mise run i18n` emits one typed accessor per message for each consumer — a Rust `Message` enum that `Localizer::format` takes, `L10n` in Swift, Kotlin and C#, `t` in TypeScript — with parameter types read off the source, so naming a string that does not exist or passing the wrong argument is a compile error in that language. `#[derive(Localized)]` puts the same convention on the error enums themselves and checks it against the source while the feature crate compiles, so a variant added without a message does not build; it is a proc macro whose expansion names only `&'static str`, which is what lets a `features/` crate use it without depending on anything above it. Every locale must define the same ids, and `mise run check` regenerates everything and fails if any generated file drifted from the source.

## Capabilities and availability

Static capabilities are the port bundles above. Dynamic capabilities are the manifest a node reports at session connect: service versions, methods, permissions, limits, extensions, current availability. The generator turns descriptors into typed per-service availability so product code holds `ChatAvailability` and views read a typed reason, never a string identifier. Availability updates stream during a session.

## Security

Device keys are Ed25519 iroh endpoint keys, generated on first launch, never exported, held in the platform keychain (`keyring` on desktop). Pairing by QR or short code exchanges endpoint ids over an iroh connection and transfers the root key. Transport encryption is iroh's QUIC; stored envelopes and backups use sealed boxes under keys derived from the root key. Provider keys live in the executing node's platform keychain (ADR 0014). Accounts, when they arrive, vouch for device keys and escrow the root key; they do not replace device identity.

Sharing a conversation copies explicitly shareable history and grants nothing else. Continuing from a shared transcript creates a new conversation with a new authority.

## Telemetry and flags

`tracing` instruments the core with OpenTelemetry export off until configured; W3C trace context rides in RPC metadata. Content, prompts, paths, and payloads are never in spans without explicit consent. Feature flags use the OpenFeature SDK with a local file provider first; flags select implementations and rollout and reach views only through availability projections. A flag never changes stored-fact meaning, wire interpretation, authority rules, or cryptography.

## Repository layout

```text
/
|-- CONTEXT.md                      product language
|-- ARCHITECTURE.md                 this document
|-- docs/{PRD,ROADMAP,GLOSSARY}.md
|-- docs/adr/
|-- protocols/
|   |-- proto/arut/<pkg>/v1/*.proto
|   |-- build/                      generator (MIT/Apache-2.0)
|   `-- rpc/                        RpcChannel, Request, Response, Status, descriptors
|-- substrates/
|   |-- watch/                      reactive cell over tokio::sync::watch
|   |-- authority/                  Command, Authority<C>, Projection, Machine
|   |-- storage/                    FactLog, BlobStore, KeyValue ports + directory impl
|   `-- identity/                   pairing, root key, sealed envelopes (keys are iroh endpoint keys)
|-- features/
|   `-- chat/                       transcript, operations, composer; commands, facts, projections, service
|-- product/
|   |-- session/                    scopes, scope handles, availability, connectivity
|   `-- i18n/                       locales/<lang>/*.ftl, the one source of user-facing strings
|       `-- macros/                  #[derive(Localized)]: an error variant's message id, checked at compile time
|-- transports/
|   |-- ipc/  connect-http/  iroh/     (in-process calls use the RpcRegistry directly; no memory transport)
|-- runtimes/
|   |-- local/                      Tokio host, arutd daemon, LAN discovery
|   |-- android/  apple/  browser/   port implementations and drivers
|-- bindings/
|   |-- ffi/                        BoltFFI exports, watch-to-event bridge, re-exported projection types
|   |-- swift/  kotlin/  dotnet/     one observation adapter each, nothing else
|   `-- typescript/                 observation adapter, wasm session bootstrap, ./react hook
|-- surfaces/                       one flat directory per surface, each its own composition root
|   |-- linux-gtk/  apple/  android/  windows/  web/  chromium/  vscode/
|-- backend/
|   `-- relay/                      pairing, relay, encrypted store-and-forward
`-- tools/
    |-- conformance/                suites every channel and storage impl must pass
    `-- i18n/                       .ftl -> xcstrings, strings.xml, .resw, served .ftl, and a typed accessor per language
```

Directories appear when their first concrete implementation exists. No `shared`, `common`, `utils`, or `services` directories. Crates document themselves in `//!` comments at the top of `lib.rs`; there are no per-directory READMEs. The only prose in the repository is this file, `CONTEXT.md`, `docs/`, and the root README.

Surfaces outside the current release stay in the tree and stay compiling where this machine can compile them, but they are not on the release's bar. Nothing in the build assumes every surface is present.

## Ecosystems and tools

Languages in the repository: Rust, Protobuf, Swift, Kotlin, C#, TypeScript. Each exists because a surface needs it; none exists for tooling. Tools: mise (toolchains and tasks), cargo, pnpm, buf (proto lint and breaking checks), BoltFFI (all foreign bindings), `tools/i18n` (Fluent to native string resources), gradle and xcodegen for their platforms. Protobuf compiles through `protox` in the build script, so no `protoc` binary is installed. Anything else is a dependency, not a project.

The dependencies that carry real weight, and what each replaces: iroh, `iroh-blobs`, `iroh-gossip` (identity, discovery, NAT traversal, relay, transport encryption, blob transfer, ephemeral replication); BoltFFI (every foreign binding); `rig` (model providers); `relm4` (the Linux surface); `fluent-bundle` and `fluent-syntax` (one string source for every surface); `keyring` (desktop secrets); `figment` (the config chain); `tracing` with OpenTelemetry and the OpenFeature SDK (telemetry and flags). Rejected with reasons in the ADRs: UniFFI, Diplomat, typeshare, `nami` and the other Rust reactive frameworks, `irpc`, libp2p, CRDT libraries.

## Verification

- Machines are tested with input vectors and a fake clock, no I/O.
- Every `RpcChannel`, `FactLog`, and `BlobStore` implementation passes one shared conformance suite.
- Protocol compatibility is tested against the previous two minor versions before release.
- CI builds and tests Linux and Android on every commit and macOS on tag.
- Surfaces carry smoke tests in v1; platform UI tests grow with each surface.

## Constraints worth knowing

- BoltFFI's wasm target does not use wasm-bindgen; futures and streams are host-polled. Anything in the wasm core needing HTTP must go through a host callback. `rig` and `reqwest` do not run there.
- BoltFFI packs iOS, macOS, and Android natively and has a JNI-based desktop JVM target; watchOS and visionOS need either an upstream contribution or a pure-Swift client.
- libadwaita ignores the system GTK theme, which is why the Linux surface does not use it (ADR 0013).
- Local-only automatic failover needs a coordinator that does not exist yet; hand-off stays explicit until one does.

## Deliberately not done

- No CRDTs. No global application snapshot. No service locator. No hand-written bindings. No English strings from the core. No feature-specific transport code. No cloud authority in the backend. No rich text until it has its own protocol version.
