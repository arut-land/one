# Engineering Glossary

Implementation vocabulary used in code, ADRs, and `ARCHITECTURE.md`. Product language lives in `CONTEXT.md`; when a word appears in both, `CONTEXT.md` wins for user-facing text.

## Scopes and ownership

**Scope**: A typed ownership boundary in the core: node, workspace, conversation, operation. The current structs are `Node<R>` and `Workspace<R>`; a workspace holds an `Arc` to its node. Conversation and operation ownership structs are Phase 2 targets. There is no runtime service container.

**Scope handle**: The unit a surface observes. Current handles expose conversation summaries, chat metadata and transcript ranges, composer state, and availability, with separate watches. Operation and connectivity handles are targets.

**Ownership tier**: Who owns a piece of state: product (shared across every surface), feature (one feature's projections and rules), or surface (disposable presentation such as sidebar collapse, stored in the platform's own store and never synced).

## Reactivity

**Watch**: The reactive cell substrate: a value, a revision, coalesced notification, no threads. A newtype over `tokio::sync::watch` compiled with only the `sync` feature so it runs on every target including wasm.

**Revision** (substrate sense): The monotonically increasing counter a watch publishes. Distinct from the product-level revision of a composer scope.

**Invalidation**: The signal that a watched value changed. Bindings coalesce invalidations and re-read the latest value on the native scheduler.

## Commands, facts, and the log

**`Command` trait**: The typed contract a mutating command implements: fact and projection types, rejection type, epoch, expiry, `apply(current, now)`, and `precondition(&self, &Projection) -> Result<(), Conflict>`. Generic machinery acts on it without a per-command switch.

**`Conflict`**: The closed enum a precondition returns when the command is stale: `Revision { current }` when the projection moved past what the command read, `Superseded` when a later pending value replaced it. Phase 2's approvals add a variant rather than a parallel type.

**`Authority<C>`**: The generic authority over one command type: dedup by command ID, epoch fencing, precondition check, apply, append. Chat uses it for durable exchanges; the composer has a separate ephemeral authority.

**Precondition**: The `Command` method that decides whether a command is still current, given the projection as the transaction refreshed it. It is a method rather than a declared enum so each command asks its own question and returns its own `Conflict`.

**`Projection` trait**: A pure reducer from facts to state; it also reports scope revisions and whether an operation is open. Testable with no I/O.

**`FactLog`**: Synchronous ordered storage with atomic `commit`, retry lookup, cursor reads, snapshots, and compaction. `commit` is the only mutating entry point; there is no separate `append`. Implemented by memory and redb, both over the shared `commit_policy` that holds fencing, deduplication, and append ordering once.

**`BlobStore`**: The BLAKE3-addressed byte-storage port. Memory implements it; persistent attachments and previews remain Phase 1 work.

**`KeyValue`**: The storage port for small unordered state, currently used for local draft recovery. Settings, pairing records, and cursors are future consumers.

**Sequence**: The ordered position in a feature's fact log, assigned atomically by storage during authority acceptance.

**Acceptance timestamp**: `accepted_at_ms`, Unix milliseconds read from `Clock` inside authority acceptance and retained in facts, transcript messages, and FFI values. Zero means an older fact did not record it.

**Cursor**: A position in a log or stream that a consumer acknowledges and resumes from.

**Snapshot** (substrate sense): A materialized projection at a sequence, used to accelerate startup and permit compaction.

## Sans-I/O

**Machine**: A state machine that takes typed inputs and returns typed effects and never performs I/O. ADR 0005 proposes it for reconnection and ordering rules; the current authority calls storage directly.
(target; not yet in code)

**Effect**: A typed instruction a machine returns for its driver to perform: append, send, persist, set timer, emit state.
(target; not yet in code)

**Driver**: Runtime code that schedules RPC or observation on a host executor. A driver for the target machine interface does not exist yet.

**Port**: A narrow trait a feature depends on for an external effect: clock, identity, persistence, drafts, process host, or spawner. Model-provider ports remain targets. Runtimes implement ports; features never name a runtime.

**Capability bundle**: A supertrait combining the ports one feature needs, with a blanket impl, so a runtime that lacks a port cannot construct that feature.

**`Spawner`**: The port that runs a future to completion on the host's executor. Supplied by the composition root; never created by a library crate.

## RPC and transport

**`RpcChannel`**: The object-safe boundary every remote call crosses: unary, server-stream, client-stream, bidirectional, over bytes. Generated clients target it; transports implement it.

**Direct client**: A generated client that calls a service trait in-process with no encoding.

**Remote client**: A generated client that encodes at the channel boundary.

**Router**: A generated erased service that decodes bytes, calls the trait, and encodes the result.

**Connect framing**: The wire format on top of `RpcChannel` for HTTP-carried routes: Connect protocol envelopes, content types, and error JSON. Chosen for browser reach and interoperability.

**Transport**: An implementation of `RpcChannel`. Current channels are the in-process registry, Connect HTTP, and Unix IPC. Iroh and browser relay routes remain targets.

**Layer**: A target wrapper around a channel for auth, epoch, trace context, or protocol-version metadata. Current channels carry metadata without those policies.

**Metadata**: The typed key-value side channel on requests and responses. Never carries product meaning.

**Endpoint**: A Phase 1 target for a node's iroh identity and connection point. Its Ed25519 public key is the product-level device key; other nodes dial it by key.

**Gossip topic**: A Phase 1 target using an `iroh-gossip` pub/sub topic among a person's paired nodes, used for ephemeral state such as drafts and presence. Best effort, unordered, never a carrier of facts.

**Route policy**: What is ours above iroh's connection choice: which channel a feature prefers (gossip, stream, relay store). Direct-versus-relay selection and failover belong to iroh.
(target; not yet in code)

## Hosting

**`Host`**: The port through which a surface's composition root obtains a running core: in-process thread, child process, system service, or remote node.

**Composition root**: The one place per deployable artifact that constructs the session, chooses the host mode, supplies the spawner, and hands the result to the binding. Nothing below it knows what it chose.

**Daemon** (`arutd`): The core packaged as a standalone process for child-process and system-service hosting.

**`Readiness`**: The daemon's startup result, printed by the child as one handshake line and parsed back by the parent: `Ready`, `LeaseHeld`, `SocketUnreachable`, `SpawnFailed`, `TimedOut`. The parent waits 10 seconds for the line, a bound `ARUT_READY_TIMEOUT_MS` overrides, and the failure reaches the caller as a typed `Unavailable` status through `StatusDetail`.

## Bindings and surfaces

**Binding**: The per-ecosystem path from scope handles to native observation, over the async stream BoltFFI generates: a Swift `AsyncStream`, a Kotlin `Flow`, a C# `IAsyncEnumerable`, a TypeScript async iterable feeding `useSyncExternalStore`, and GTK/GLib observation. There is no alias facade and no adapter class, only one generic subscribe-and-read helper per platform. Owns no product transitions.

**FFI binding**: `bindings/ffi` exports handles and bridges watches to BoltFFI callback streams. Features and product/session also depend on BoltFFI to annotate their projection types.

**Projection type**: A Rust struct a surface reads. Annotated for FFI in place; never mirrored.

## Protocol and compatibility

**Descriptor**: Generated static metadata for a service and its methods, the input to the manifest. It reaches the manifest through a generated marker type whose `Service::DESCRIPTOR` constant carries it, so a service is named by a type rather than by a string.

**Compatibility window**: The range of minor versions within which a session and a node agree to talk: two minors. Outside it, a typed unsupported result.
(target; not yet in code)

**Additive evolution**: Within a protocol version, fields are only added; removed numbers stay reserved. Conceptual breaks start a new package version with explicit translation.
