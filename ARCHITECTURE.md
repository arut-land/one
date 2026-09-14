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
bindings/              FFI exports and the per-ecosystem package roots
product/               sessions, node/workspace scopes, localization
features/chat/         chat acceptance, clients, projections, composer, ports
substrates/authority/  generic command acceptance and projection reduction
substrates/storage/    FactLog, BlobStore, KeyValue; memory and native redb
substrates/watch/      revisioned watch values and subscriptions
substrates/identity/   Phase 1: iroh identity and pairing integration
protocols/             Protobuf contracts, Rust generator, RPC vocabulary
transports/            arut-transport: Connect HTTP framing, Unix IPC behind cfg(unix)
transports/iroh/       Phase 1: RpcChannel over iroh streams
runtimes/              local native drivers, host-polled memory ports, browser callbacks
tools/                 development commands and shared conformance suites
```

Substrates depend only on substrates and protocols. Runtimes compose features, product, substrates, protocols, transports, and other runtimes. Features depend on substrates and protocols. Product depends on features, substrates, protocols, and other product crates. Rust surfaces consume product clients directly; foreign surfaces import their binding package. Composition roots select runtimes. Transport implementations depend on RPC contracts, not product behavior. A binding does not select a host mode on behalf of a view.

`arut-dev check` reads Cargo metadata, including optional, target-specific, build, and development edges, and rejects a dependency the table above forbids. It also rejects a `bindings/ffi` dependency other than BoltFFI and `async-task`, a relative TypeScript import that escapes its surface or runtime package, generated localization resources that differ from `product/i18n/locales`, generated FFI handles that differ from `bindings/ffi/handles.toml`, and an `x:Uid` in the WinUI XAML that the generated `.resw` does not define. It also rejects a subscribe-and-read loop over a `*Changes()` stream in a surface file and names the binding helper that owns it; the Linux surface is exempt, because it observes through GTK's own main loop rather than a binding package. The per-crate `unsafe_code` declaration rule is gone: the workspace lint table states it once and `check:core-graph` greps for the escape. The source scans for feature `ServiceImpl` and `Authority` names are gone: a feature's service implementations are `pub(crate)`, so Rust privacy already stops a crate above them from naming one.

Three exceptions are recorded in the tool: the chat feature's i18n derive macro, which is a build-time proc-macro edge; the FFI factory's host-polled runtime dependency; and the FFI crate's test-only `futures-executor`. The remaining core-graph rules are the `check:core-graph` mise task, five lines of shell: `cargo tree` proves that `arut_ffi` built for wasm reaches neither wasm-bindgen nor a Tokio executor, and a grep proves that no crate under `features/`, `product/`, `substrates/` or `bindings/` opts out of the unsafe-code deny.

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

`Node::workspace` constructs a workspace and gives it child cancellation. Neither struct names a feature: `Workspace::clients` hands back whatever client set the node was built over, and `ProductSession<F>` holds `Workspace<F::Clients>`. A workspace keeps its node alive and creates cancellation scopes for conversation clients.

Conversation and operation ownership structs are Phase 2 work. Today's `ChatClient` is an observable conversation handle, not a `Conversation` scope struct. The global/node/workspace/conversation configuration chain is also a target; no `ConfigChain` or `NodeRuntime` type exists.

## Ports and composition

The chat feature requires four ports in `features/chat/src/ports.rs`:

- `IdSource::new_id() -> String` supplies identities.
- `Persist<F>::log(namespace)` opens a named fact log.
- `Drafts::drafts()` supplies local key-value recovery storage.
- `Clock::now() -> u64` supplies Unix milliseconds.

`ChatRuntime` combines these ports with `Send + Sync + 'static`. `compose(Arc<R>)` constructs private chat and composer authorities and services. `ChatFeature` exposes generated clients and erased routers; its services are named by the type `ChatServices`, a tuple of the two generated service markers, rather than by a registration list. A runtime without the required ports cannot compose chat.

`product/session/src/feature.rs` lifts that to a set. `Feature` names a member's services and clients, `Compose<R>` is implemented for a feature only where `R` supplies its ports, and tuples of members implement `FeatureSet` and `ComposeSet<R>` the way tuples of service markers implement `ServiceSet`. `FeatureSet::DESCRIPTORS` concatenates its members' service sets at compile time, and `Services<F>` presents that as an ordinary `ServiceSet`, so the manifest still comes from types. A root writes `type Features = (Chat,)` and gets the routers it serves, the clients its session holds, and what it advertises; adding a feature is one tuple member. The feature's adapter lives in `product/session`, because `features/` may not depend on `product/`.

`LocalRuntime` supplies native UUIDv7 IDs, wall time, redb logs, draft storage, and the node lease. `arutd` composes chat and passes its routers to `Node::serve`. This runtime assembly type is distinct from the product's ownership scope `Node<R>`. `Node::serve` registers routers and the derived capability service. Its small `Blocking` adapter runs dispatch construction and future polling through `spawn_blocking`, retaining the runtime during active calls. Redb's synchronous transactions still need that executor isolation.

`MemoryRuntime` supplies named memory logs, draft storage, injected IDs, and an injected clock. It contains no feature list. Core tests use the feature's `test_support::MemoryPorts` and `TestIds` through the production composition function, without a product-to-runtime development dependency.

`ProductSession::local` takes the clients a composed set produced and derives its capability client from `Services<F>`. `ProductSession::remote` binds the set's clients and the capability client to the root's chosen channel. The session constructs its pending chat through the same helper used for subsequent pending chats. FFI factories compose `Features` over memory ports and call `local`; the Linux root calls `remote` with its child-process channel.

## Acceptance, facts, and recovery

`Command` declares its fact, projection, rejection, epoch, optional expiry, a pure `apply(current, now)` function, and `precondition(&self, &Self::Projection) -> Result<(), Conflict>`. The precondition is a method rather than a declared enum, so each command answers its own staleness question against the projection it reduces; `Conflict` names the two answers every command can give, a revision that moved and a superseded pending value.

`Authority<C>` serializes acceptance through `FactLog::commit`, which is now the log's only mutating entry point: `FactLog::append` is deleted, and the conformance suite carries its own compare-and-append. Inside `arut-storage`, one `commit_policy` function over an internal `LogStore` trait holds fencing, deduplication, cursor validation, and append ordering. Memory and redb each implement `LogStore` and inherit that policy instead of restating it. The transaction refreshes the projection, checks the retry outcome, validates the command, and optionally appends one record. Outcomes distinguish applied, duplicate, revision conflict, authority mismatch, superseded, and rejected commands.

Chat invokes `execute_with_clock`. The authority reads the supplied clock inside the transaction after deduplication. The accepted `ChatFact` batch and its transcript messages carry `accepted_at_ms`. `ChatMessage` in the projection and generated FFI bindings preserves that timestamp. The Protobuf additions are field 6 on `ChatFact` and field 4 on `ChatMessage`; older rows decode with zero. A retry returns the original timestamp.

Each accepted mock exchange records a user message, an echo response, and started/completed operation facts. Records also carry sequence, authority epoch, and command ID. Chat currently uses epoch 1. Canonical UUIDv7 command IDs deduplicate starts and sends. Operation execution, model streaming, replication, and authority hand-off are not implemented.

`ChatProjection` is an internal pure reducer. Authority startup replays facts after an optional snapshot. Explicit checkpoint and compaction methods exist; the daemon does not schedule maintenance automatically. Retry outcomes survive compaction and currently have no expiry.

Drafts are ephemeral values with local `KeyValue` recovery, not transcript facts. Composer edits check revision and epoch. A start fact records the pending revision it consumed, so recovery can finish draft cleanup after a failure without erasing later edits. Cross-device last-writer-wins replication and remote-editing indicators remain Phase 1 work under ADR 0018.

## Storage

ADR [0023](docs/adr/0023-redb-default-node-storage.md) amends [0010](docs/adr/0010-storage-as-three-ports.md). The ports remain `FactLog`, `BlobStore`, and `KeyValue`. Native nodes use redb for facts and key-value data. The directory implementation and `NodeStorage` selector are gone; `ARUT_STORAGE` no longer selects a store.

The primary feature namespace uses `node.redb`; other namespaces use `log-<BLAKE3 digest>.redb`. Redb owns each database exclusively. `node.lock` prevents two runtimes from owning the same node directory. Transactions maintain sequence and command indexes, snapshots, retained retry outcomes, and a schema-version table. Existing redb files remain readable. Directory data is not automatically migrated.

Memory implements all three ports for tests and wasm. Redb is an opt-in Cargo feature enabled by the local runtime, not a wasm dependency. No persistent `BlobStore` remains; Phase 1 adds attachment storage and transfer through `iroh-blobs`. Current memory blob addresses use BLAKE3 and verify content on reads.

## Bindings and surfaces

`Watch<T>` wraps `tokio::sync::watch` with Tokio's `sync` feature only. Updates compare a detached candidate and increment the revision only when the value changes. New subscribers receive an initial invalidation; intervening updates may coalesce. Dropping the writer closes subscriptions.

Product sessions expose conversation summaries and composer availability. The conversations watch also holds the selection and the search: `select(id)`, `selected_id()`, `set_query(text)` and `title()` move through the invalidation every surface already observes, so no surface keeps its own copy, writes its own casing rule, or decides what the window is called. `state()` returns the summaries the query matches, each carrying `match_ranges` in characters and `unread`, which a conversation gains when its transcript moves while another one is selected and loses when it is selected. Chat and composer handles expose separate watches. Transcript metadata contains `last_message_id`; `messages_after(id)` returns immutable keyed rows strictly after that cursor, each with `starts_time_group`, `starts_speaker_group` and `ends_speaker_group`. Surfaces cache rows and fetch only additions. Connectivity and operation handles are targets, not current exports.

`ChatState` carries `can_send`, `is_sending`, and `is_empty` as fields, recomputed by `ChatState::derive` on every write to the projection. They are pure functions of the other fields, so five surfaces do not restate the same rule. `can_send` deliberately excludes "the draft is non-empty": the draft is `ComposerState.text`, which a surface observes separately, and merging the two would need the single merged view ADR 0007 rejects. Chat and composer handles expose `error_key()` and `error_args()`, the Fluent id of the current typed error and its arguments as `ErrorArg { name, value }` pairs in catalog order, so a surface selects a sentence without one crossing FFI and without re-pairing names to positions (ADR 0016, ADR 0022). Both come from the `Localized` derive's `message_args`, so the argument order is stated once, in the message.

`ComposerClient::replace(text)` writes `text` into `ComposerState.text` at call time, before its future is polled, and resolves when a flush carrying that text or a later one is acknowledged. Edits coalesce behind one in-flight request: 100 rapid calls issue two `replace_composer` requests and the last text wins. `ChatClient::send` flushes the composer before it takes the operation lock, so a send after unflushed keystrokes commits what the person typed. A surface binds a text field straight to `ComposerState.text` and keeps no queue of its own (ADR 0024).

Projection types declare `#[boltffi::data]` in their owning crate and are re-exported by `bindings/ffi`. The `#[export]` blocks that expose handles and revision streams are generated: `bindings/ffi/handles.toml` declares each scope and `arut-dev generate` writes `bindings/ffi/src/generated/handles.rs`, which is checked in because BoltFFI's source scanner reads source and expands no macros. Intents keep a second `#[export] impl` block per handle in `bindings/ffi/src/intents.rs`, which is where the factories live too. The four streams declare `mode = "async"`, so each ecosystem receives its own idiom: a Swift `AsyncStream`, a Kotlin `Flow`, a C# `IAsyncEnumerable`, and a TypeScript async iterable. Nothing renames the generated types: Swift re-exports `ArutFfi`, the .NET props file carries `<Using Include="Arut.Ffi" />`, and Kotlin surfaces import `dev.arut.ffi` directly. What each platform keeps is one generic subscribe-and-read helper over those streams, not an observation class per handle (ADR 0021). GTK consumes product types directly through its local observation module. A keyed `gio::ListModel` appends transcript rows with one `items_changed(position, 0, count)` per batch, and `GtkListView` recycles their widgets. Composer and status GObject properties bind to widgets; one waker-driven GLib task per watch refreshes them without an intermediate component message. The native gtk-rs scheduler internally pairs its task source with a child waker source. No timer, extra thread, or per-invalidation task is added.

The Linux shell uses a searchable list model, desktop-configured decorations and toolbar, and a `GtkPaned` sidebar that collapses below 720 logical pixels. Toggling the sidebar eases the pane position on the frame clock over 250 ms while the revealer crossfades over the same duration, so the list slides and clips rather than cutting; `motion.rs` owns that curve, the same eased jump-to-latest scroll, and the reduced-motion answer, which folds `gtk-enable-animations` into GTK 4.22's own setting. The toggle button reports its state through `toggled`, and the shell acts only when that state disagrees with the model, because writing the model back into the button also emits the signal. Transcript and composer share an 880-pixel reading column. Both sides of the transcript are borderless bubbles, the person's tinted with the accent and the assistant's with the foreground; bubbles inside one speaker group flatten the corners between them, the assistant's name appears once per group, and a row appended while the transcript is on screen fades in through a CSS keyframe on opacity alone, because a keyframe that changes a row's height fights `GtkListView`'s scroll anchoring. Rows show the locale's short clock without seconds and the date header reads Today, Yesterday, the weekday within a week, then the month and day; the full stamp stays in the tooltip. A failed node connection is a banner overlaid at the top of whichever page is showing, with a Retry action that runs initialization again; the availability pill in the toolbar appears only for states that need explaining. The surface's own icons are Lucide SVGs under `surfaces/linux/data/icons/lucide` (Linux only; the other surfaces keep their native symbols), which `build.rs` converts to GTK's stroke-symbolic form and compiles with the hicolor app icon into one GResource that `icons.rs` registers with the icon theme; the sidebar's row states are the theme's `navigation-sidebar` style, not the surface's. The header carries a primary menu (About, Keyboard Shortcuts, Quit); one shortcut table drives the accelerators, the tooltips rendered from `ShortcutTrigger::to_label`, and the `GtkShortcutsWindow` behind `Ctrl+?`. Window size, maximized state, and the sidebar width persist beside the navigation state. Visited composer controllers retain their buffers; the draft itself echoes locally and flushes last-writer-wins, so the surface holds no edit queue. Transcript reading positions are retained per conversation. Actions use `gio::SimpleAction` and `GtkShortcutController`. One `style.css` provider at application priority refers to GTK's own named theme colors, so the running theme resolves them; GTK 4.22 reports color scheme, contrast and reduced motion, which the provider passes through so `style.css` answers them with its own `@media` queries; the appearance portal supplies only the accent, which GTK does not expose before 4.24, and `gtk-enable-animations` still folds into reduced motion. `decorations.rs` probes an unmapped window for GTK's server-decoration preference and reads `gtk-decoration-layout`, into which GTK already folds the portal's button layout. Server frames win; Hyprland, Sway, river, niri, and empty layouts get a real toolbar with no window controls; KDE uses compositor frames when offered and never an invented GNOME or Breeze layout; only GNOME falls back to GNOME buttons. Only navigation is persisted, under `XDG_STATE_HOME/arut/linux-ui`; the node owns conversations and drafts. `mise run review:linux` runs `tools/review-linux.sh`, which builds with `--features review`, exercises real IPC fixtures on an isolated Hyprland output, captures three sizes plus the narrow breakpoint, and removes its window and output.

FFI observation lives in `bindings/ffi/src/observation.rs`, which bridges a `Subscription<u64>` onto a capacity-one BoltFFI `EventSubscription` through an `async_task` runnable whose schedule function runs it, so a wake polls the stream on the thread that woke it and no executor is involved. `async-task` is `no_std` and is the only crate besides BoltFFI that `bindings/ffi` names from outside the workspace. The standalone `HostPolledSpawner` in `runtimes/host-polled` implements `LocalSpawner` through `async_executor::LocalExecutor` and bounded ticks; platform callbacks do not yet drive it. Idle unsubscribed FFI observers may remain retained until the next source change, an open roadmap decision.

## Protocols and transport

`protocols/proto/arut` contains capability, chat, and nested composer packages. `protox` compiles descriptors; `arut-protocol-build` generates Rust service traits, direct and remote clients, routers, procedure names, descriptors, and one `Service` marker type per service. Direct calls pass typed values. Remote calls encode at `RpcChannel`. All four streaming shapes stay declared on `RpcChannel`, but `client_stream` and `bidirectional` carry default bodies that return `Unimplemented`, so a transport implements only the shapes it can carry and the generator emits a router arm for a shape only when a service has a method of it.

The registry supplies an in-process channel. One crate, `arut-transport`, holds Connect HTTP and, behind `#[cfg(unix)] pub mod ipc`, the Unix-socket connector over the same framing; both support unary calls and server streams. Connect framing bounds messages to 8 MiB through `http_body_util::Limited` and preserves typed status codes and details, and a `tokio_util::codec::Decoder` reads envelopes for the server and the streaming client alike. `StatusDetail` maps a typed enum onto one status code and a tag byte in the status detail, which is how a daemon's startup result reaches its parent as a value rather than a parsed line. No WebSocket or iroh channel exists yet.

The capability service derives its manifest from typed service markers (ADR 0025). Each generated service has a marker carrying its `ServiceDescriptor` as an associated constant, tuples of markers implement `ServiceSet`, and `manifest::<S>()` builds the manifest from the set. `Node::serve` takes that set as a type parameter, so no package or service name is written by hand in the product. `CapabilityManifest::availability::<S>()` answers `Available`, `ReportedUnavailable`, or `NotAdvertised`, and product maps those plus manifest failures into `FeatureAvailability`. Per-service availability types are still not generated, and capabilities do not yet stream. `ServiceDescriptor::version` is a `Version { major, minor }` and `compatible()` states ADR 0015's two-minor window, but nothing produces a minor other than zero yet, so the comparison is inert until a second minor exists. `buf breaking` checks additive schema evolution against local `main`, not negotiated runtime compatibility.

## Hosting and browser composition

`Host` and `HostMode` describe in-process, child-process, system-service, and remote hosting. `ChildHost` starts `arutd` and waits up to 10 seconds for its handshake line, a bound `ARUT_READY_TIMEOUT_MS` overrides. `process-wrap` owns the kill: the daemon runs in its own process group on Unix and its own job object on Windows, so dropping the channel takes whatever it spawned with it. `arutd` asks Linux for `PR_SET_PDEATHSIG` on its main thread before it builds the Tokio runtime, so no worker thread starts without it. One `Readiness` enum owns both sides of that handshake: the daemon prints `Readiness::line()`, the parent parses it back, and `LeaseHeld`, `SocketUnreachable`, `SpawnFailed`, or `TimedOut` reaches the caller as a typed `Unavailable` status through `StatusDetail`. Channel layers are one type as well: `Wrapped<W>` forwards unary and server-stream calls through a `Wrap`, which is how spawner scheduling, redb's `spawn_blocking` isolation, and the child process's lifetime are each expressed. Executor creation lives in the local runtime and the daemon root; GTK dispatches UI work on GLib when its futures are woken.

| Surface | Current composition | Remaining target |
| --- | --- | --- |
| Linux | relm4/GTK4 root, child `arutd`, Unix IPC, persistent redb | v1 features and device routes |
| Apple | SwiftUI, in-process memory FFI session | macOS child hosting; iOS lifecycle support |
| Android | Compose, ViewModel-owned memory FFI session | foreground service and persistent node |
| Windows | WinUI, in-process memory FFI session | child hosting and persistent node |
| Web, Chromium | wasm memory session in the page, rendering `@arut/chat-ui` | worker/lifecycle integration and remote routes |
| VS Code | wasm session in the extension host; the webview renders `@arut/chat-ui` over a `postMessage` port and receives whole projections | native extension-host composition or daemon route |

`runtimes/browser` is the pnpm package `@arut/runtime-browser`. Web, Chromium, and VS Code import its UUIDv7 callback through the package export and supply wall time to the binding. `surfaces/chat-ui` is `@arut/chat-ui`: the React chat view, the `ChatPort` interface shaped like the generated session handle, `wasmPort` (the session itself) and `bridgePort` (a webview over posted projections), and `mountChat`. Each browser root is a few lines that pick a port and mount the view. Relative TypeScript paths cannot escape surface or runtime packages; `pnpm check` runs one browser `tsc` project, one Node project for the extension host, and the store tests.

Terminal, JetBrains, watch, and messaging surfaces have no directories yet.

## Localization

`product/i18n/locales/en` is the current Fluent source, and one crate parses it: `arut-i18n-catalog` is shared by `product/i18n`'s build script, the `Localized` derive, and `arut-dev`, so a message the generator would refuse can no longer compile through the derive. The build script writes the `Message` enum and `Message::KEYS` into `OUT_DIR`; no generated Rust is checked in.

`mise run generate` writes the native resources: `Localizable.xcstrings` for Apple, `values/strings.xml` for Android in the binding library so its own `R` names them, `Resources.resw` for Windows, and one `bindings/typescript/src/generated/catalog.ts` carrying the Fluent text the browser and editor surfaces used to fetch as copied files. Typed accessors are generated only where a platform has no checked accessor of its own. Android uses `R.string` and `R.plurals` for what a layout names, plus a generated `dev.arut.bindings.generated.Messages` map for an id the core supplies at runtime. Swift and C# get key constants over `String(localized:)` and `ResourceLoader`, with a generated `Quantity()` on the C# side because `.resw` has no plural form. TypeScript gets a `MessageKey` union, an `Args` map, the `placeables` each message interpolates, and one `t()`. GTK uses the Rust `Localizer`; surface error mappings select keys from typed values, and product transitions return typed errors rather than sentences.

`Localized` derives message keys, validates them against the English source at compile time, and emits `MESSAGE_KEYS`, so the completeness test reads the required key list from the types instead of parsing the source itself. It also derives `message_args`, each variant's fields beside the Fluent names that select them, and refuses a variant whose fields and message placeables disagree by name or count. `arut-dev check` regenerates every resource and fails when the checked-in output differs, and fails on an `x:Uid` in the WinUI XAML that the generated `.resw` does not define. Adding a locale requires its files and registration in `product/i18n/src/lib.rs`. Locale choice stays outside the feature core.

## Accepted targets beyond the current slice

Phase 1 adds iroh endpoints, pairing, device keys, sealed envelopes, peer-assisted delivery, gossip drafts, blob transfer, a `ModelProvider` implemented with `rig`, and a `figment` configuration chain. ADR 0019 assigns connection discovery and direct/relay failover to iroh; Arut does not build a routing substrate. Keychain storage, root-key derivation, encryption, and backups remain unimplemented. Current local IPC permissions do not provide the planned device trust protocol.

Phase 2 adds conversation/operation ownership structs, explicit hand-off, harnesses, tools, and approvals. ADR 0005's reusable `Machine`/`Effect` interface remains a target. Today's authority calls synchronous storage ports; there is no generic machine trait or effect driver. Phase 3 adds backend accounts, recovery, push, and hosted nodes. The backend has no directory yet.

`tracing` spans exist for RPC, authority, and composer activity. There is no OpenTelemetry exporter, OpenFeature integration, or feature-flag provider. Those remain target integrations. Content, prompts, and payloads must not enter telemetry without consent.

## Repository layout

The workspace contains 18 Rust crates. The inventory below lists every crate and surface root; implementation files, resources, and generated build output are omitted.

```text
/
|-- .github/workflows/{ci,android}.yml
|-- CONTEXT.md, ARCHITECTURE.md
|-- Cargo.toml, Cargo.lock, deny.toml
|-- mise.toml, mise.lock, buf.yaml
|-- package.json, pnpm-workspace.yaml, pnpm-lock.yaml, tsconfig.json
|-- .gitignore, LICENSE-APACHE, LICENSE-MIT, LICENSE-FSL
|-- docs/                          PRD, ROADMAP, GLOSSARY, ECOSYSTEM, OBSERVATION, BUILDING, platform notes
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
|       |-- catalog/                arut-i18n-catalog: the one Fluent parser
|       `-- macros/                 arut-i18n-macros
|-- transports/                     arut-transport: Connect HTTP, Unix IPC
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
|   |-- chat-ui/                    @arut/chat-ui, the shared React chat view and ChatPort
|   `-- web/, chromium/, vscode/
`-- tools/
    |-- dev/                        arut-dev: generate, check
    `-- conformance/                arut-conformance: shared port tests
```

Foreign generated packages live under ignored `bindings/generated`. No README files are maintained. Rust crates document their contracts with `//!` comments. Do not add generic `shared`, `common`, `utils`, or `services` buckets; `surfaces/apple/shared` is the existing package shared by Apple app targets.

## Tooling and verification

Mise pins the toolchains and owns the task graph; every ecosystem keeps its own build system (ADR 0026). Cargo builds Rust, pnpm builds and checks the TypeScript workspace, buf checks Protobuf, and BoltFFI packages foreign bindings. The JVM tasks share one JDK 25 and one Gradle 9.7.1; Gradle, XcodeGen, and the .NET SDK are scoped to the platform tasks that need them, so a Windows build never installs Node. Tasks that belong to one surface live in that surface's own `mise.toml` and are addressed as `//surfaces/linux:run`, with an `alias` for the repository-wide name; `mise-tasks/` holds the two check scripts too long to read inside a TOML string. Machine-specific tuning belongs in ignored `mise.local.toml`.

`tools/dev` is one binary crate, `arut-dev`, with two subcommands: `generate` writes the localization resources and the FFI handle blocks, and `check` runs the repository rules listed above. Both generators share one output module, so each writes only when the bytes differ and each is diffed the same way. Mise calls both, and CI calls the same mise tasks. Platform tasks own Android target installation and Apple binding generation. Localization resource emitters and typed accessors share platform key naming. `tools/conformance` remains a test crate.

`mise run check` depends on five tasks. `check:rust` carries every cargo step in one task so they do not fight the build lock: formatting, Clippy, nextest, doctests, cargo-machete, the wasm build of the seven isolated core crates, and `arut-dev check`. `check:core-graph` is the `cargo tree` assertion and the unsafe-code grep. `check:proto` runs buf lint and buf breaking, `check:deps` runs cargo-deny, and `check:ts` type-checks the pnpm workspace. Every task declares the files it reads, and mise skips it when a blake3 hash of them is unchanged, so a second `check` costs seconds; `check:proto` and `check:deps` declare nothing and always run, because the main branch and the advisory database move on their own. Cargo tasks declare a state marker rather than `target/`, and mr-boxington caches rustc invocations in a store shared across checkouts. `mise run fmt:dotnet` formats C# with `dotnet format`, which the .NET SDK already carries, and `--verify-no-changes` checks instead of writing. `mise run build` builds GTK, `arutd`, and web/Chromium/VS Code bundles. Both gates run before every commit with `CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4`.

CI runs once per commit: pull requests are checked by the pull-request event alone, and the push event covers only `main` and version tags, so a branch with a pull request is not built twice. Superseded runs on the same ref are cancelled; a manual dispatch keeps its own concurrency group. The Linux job runs `mise run check` and then `mise run build` inside a Fedora container, because the GitHub Ubuntu image ships GTK 4.14 and the surface needs 4.22 (`fedora:latest` is Fedora 44, gtk4 4.22.5). Dependency policy is a separate job, so an advisory-database fetch failure neither masks a compile failure nor blocks the build. Windows runs `mise run check:windows`. macOS builds and runs Swift binding tests on pull requests and version tags. Android has its own workflow, filtered by GitHub's `paths:` rather than by a job that computes changed files; `mise run --affected` computes the same set from the project graph but only after a runner has started. The Linux job restores the mise task state and the mr-boxington store with `actions/cache`, so a commit that touches no Rust file skips the Rust gate. Native Android and Windows compilation remains unverified on this Linux machine. Apple build and validation details are recorded in `docs/APPLE.md`.

`mise run check` and `mise run build` pass on this machine: 142 nextest tests, all doctests, `arut-dev check`, the core-graph assertions, buf lint and breaking, cargo-deny, the TypeScript projects and store tests, and the web, Chromium, and VS Code bundles. The 12 display-backed GTK tests are ignored by default and pass under `tools/review-linux.sh`, which also produced the review captures. Conformance covers registry/HTTP/IPC RPC, memory/redb logs and key-value storage, and memory blobs. Swift, Kotlin, and C# were rewritten without a compiler on this machine and must be built on their own platforms before release.
