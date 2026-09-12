# Engineering Glossary

Implementation vocabulary used in code, ADRs, and `ARCHITECTURE.md`. Product language lives in `CONTEXT.md`; when a word appears in both, `CONTEXT.md` wins for user-facing text.

## Scopes and ownership

**Scope**: A typed ownership boundary in the core: node, workspace, conversation, operation. Each scope is a Rust struct that holds `Arc`s to its parent and constructs its children. There is no runtime service container.

**Scope handle**: The unit a surface observes. One handle per scope instance (`Conversations`, `Conversation(id)`, `Composer(scope)`, `Operation(id)`, `Availability`), each with its own watch. Surfaces observe only the handles they render.

**Ownership tier**: Who owns a piece of state: product (shared across every surface), feature (one feature's projections and rules), or surface (disposable presentation such as sidebar collapse, stored in the platform's own store and never synced).

## Reactivity

**Watch**: The reactive cell substrate: a value, a revision, coalesced notification, no threads. A newtype over `tokio::sync::watch` compiled with only the `sync` feature so it runs on every target including wasm.

**Revision** (substrate sense): The monotonically increasing counter a watch publishes. Distinct from the product-level revision of a composer scope.

**Invalidation**: The signal that a watched value changed. Bindings coalesce invalidations and re-read the latest value on the native scheduler.

## Commands, facts, and the log

**`Command` trait**: The typed contract a mutating command implements: scope, fact type, outcome type, idempotency, precondition, and `apply`. Generic machinery acts on it without a per-command switch.

**`Authority<C>`**: The generic authority over one command type: dedup by command ID, epoch fencing, precondition check, apply, append. The composer authority was its first instance.

**`Projection` trait**: A pure reducer from facts to a bounded state. Testable with no I/O.

**`FactLog`**: The storage port for ordered facts: append, outcome-of, read-from-cursor, snapshot, compact. Not a key-value store.

**`BlobStore`**: The content-addressed storage port for attachments and previews.

**`KeyValue`**: The storage port for small unordered state: settings, pairing records, cursors.

**Sequence** (`Seq`): The position of a fact in a conversation's log, assigned by the authority.

**Cursor**: A position in a log or stream that a consumer acknowledges and resumes from.

**Snapshot** (substrate sense): A materialized projection at a sequence, used to accelerate startup and permit compaction.

## Sans-I/O

**Machine**: A state machine that takes typed inputs and returns typed effects and never performs I/O. Authority, session negotiation, stream resume, and draft replication are machines.

**Effect**: A typed instruction a machine returns for its driver to perform: append, send, persist, set timer, emit state.

**Driver**: The runtime code that feeds a machine inputs and performs its effects on a real executor.

**Port**: A narrow trait a feature depends on for an external effect: clock, model provider, file store, process host, spawner. Runtimes implement ports; features never name a runtime.

**Capability bundle**: A supertrait combining the ports one feature needs, with a blanket impl, so a runtime that lacks a port cannot construct that feature.

**`Spawner`**: The port that runs a future to completion on the host's executor. Supplied by the composition root; never created by a library crate.

## RPC and transport

**`RpcChannel`**: The object-safe boundary every remote call crosses: unary, server-stream, client-stream, bidirectional, over bytes. Generated clients target it; transports implement it.

**Direct client**: A generated client that calls a service trait in-process with no encoding.

**Remote client**: A generated client that encodes at the channel boundary.

**Router**: A generated erased service that decodes bytes, calls the trait, and encodes the result.

**Connect framing**: The wire format on top of `RpcChannel` for HTTP-carried routes: Connect protocol envelopes, content types, and error JSON. Chosen for browser reach and interoperability.

**Transport**: An implementation of `RpcChannel` over some I/O: memory, IPC pipe, WebSocket, QUIC, WebRTC data channel.

**Layer**: A wrapper around a channel that attaches or reads metadata: auth, epoch, trace context, protocol version.

**Metadata**: The typed key-value side channel on requests and responses. Never carries product meaning.

**Endpoint**: A node's iroh identity and connection point. Its Ed25519 public key is the product-level device key; other nodes dial it by key.

**Gossip topic**: An `iroh-gossip` pub/sub topic among a person's paired nodes, used for ephemeral state such as drafts and presence. Best effort, unordered, never a carrier of facts.

**Route policy**: What is ours above iroh's connection choice: which channel a feature prefers (gossip, stream, relay store). Direct-versus-relay selection and failover belong to iroh.

## Hosting

**`Host`**: The port through which a surface's composition root obtains a running core: in-process thread, child process, system service, or remote node.

**Composition root**: The one place per deployable artifact that constructs the session, chooses the host mode, supplies the spawner, and hands the result to the binding. Nothing below it knows what it chose.

**Daemon** (`arutd`): The core packaged as a standalone process for child-process and system-service hosting.

## Bindings and surfaces

**Binding**: The per-ecosystem adapter from scope handles to native observation: `@Observable`, `StateFlow`, `INotifyPropertyChanged`, `useSyncExternalStore`, GTK properties. Generated where possible; owns no product transitions.

**FFI substrate**: Arut's thin wrapper over BoltFFI that adds the watch-to-event bridge and the handle conventions. The only crate that depends on `boltffi` directly besides features that annotate data types.

**Projection type**: A Rust struct a surface reads. Annotated for FFI in place; never mirrored.

## Protocol and compatibility

**Descriptor**: Generated static metadata for a service and its methods, the input to the manifest.

**Compatibility window**: The range of minor versions within which a session and a node agree to talk: two minors. Outside it, a typed unsupported result.

**Additive evolution**: Within a protocol version, fields are only added; removed numbers stay reserved. Conceptual breaks start a new package version with explicit translation.
