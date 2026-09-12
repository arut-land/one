# Phase 0 progress

Status as of 2026-09-12. Phase 0 of `docs/ROADMAP.md` is implemented on the Rust side and verified by the four gates. Items listed under "Not yet verified" are the honest remainder before the phase's exit criterion is fully met.

## Gates on the current tree

| Gate | Result |
| --- | --- |
| `cargo build --workspace` | pass |
| `cargo test --workspace` | pass, 30 tests |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo check -p arut_ffi --target wasm32-unknown-unknown` | pass |

A Node smoke run of the packed wasm module succeeded: it imports only `__boltffi_wake`, `__boltffi_stream_wake`, and the host ID callback, and `createBrowserSession(...).chat().state()` returns a valid empty conversation. No thread imports remain.

## Steps

1. **Watch substrate.** Done. `substrates/watch` is a newtype over `tokio::sync::watch` with only the `sync` feature. `substrates/state`, the FFI bridge thread, and the Qt observer thread are gone.
2. **Spawner and Host ports.** Done. `runtimes/local` owns the Tokio executor and the `arutd` daemon; `ChildHost` spawns and supervises it over a Unix socket. The global runtime inside the HTTP transport is gone.
3. **Typed scopes.** Done. Node, workspace, conversation scopes in `product/session/src/scopes.rs`; the `OnceLock` self-reference is gone.
4. **Scope handles and bindings.** Done. Projection types carry `#[boltffi::data]` in `features/chat` and are re-exported by `bindings/ffi`. The five per-language ChatModels are deleted. Swift, Kotlin, .NET, and TypeScript bindings are observation adapters only, rendered from `bindings/templates` by `bindings/generate-observers.py`. `bindings/qt` and the Qt Linux surface are removed; `surfaces/linux-gtk` is a Rust-owned GTK4 surface with no FFI and no libadwaita.
5. **Connect framing and streaming.** Done. `transports/connect-http` (framing, server), `transports/ipc` (Unix socket), `transports/memory`. `WatchComposer` is a server stream; the 500 ms polling loops are gone.
6. **Storage ports.** Done. `substrates/storage` defines `FactLog`, `BlobStore`, `KeyValue` with memory and directory implementations. Chat facts persist through the log; drafts stay ephemeral per ADR 0018. `SendMessage` carries a UUIDv7 `command_id`. The whole-file checkpoint and its proto messages are gone.
7. **Generic authority.** Done. `substrates/authority` provides `Authority<C: Command>` and pure reducers; the chat service uses it for start and send.
8. **Conformance suites.** Done. `tools/conformance` runs shared suites against every `RpcChannel` and `FactLog` implementation.

## Decisions made during implementation

- **`IdSource` port.** UUIDv7 needs a clock and entropy, which the BoltFFI wasm core cannot reach. IDs come from a port the composition root supplies: native runtimes use `uuid`, the browser runtime supplies them from JavaScript. The `NativeIds` fallback panics on wasm by design.
- **Executor ownership.** The GTK composition root builds the Tokio runtime and passes a `TokioSpawner`; nothing below it creates one.
- **Daemon placement.** `ChildHost` looks for `arutd` next to the current executable, puts the socket in the temp directory, and stores data under `$XDG_DATA_HOME/arut`.
- **VS Code.** Moved to `surfaces/vscode` as a prototype over the TypeScript observation adapter; extension-host hosting is deferred to its roadmap phase.
- **Browser runtime.** `runtimes/browser` is a small TypeScript package that initializes the wasm core and supplies `IdSource`; the web surface composes through it.

## Not yet verified

- The TypeScript workspace (`pnpm check`, `pnpm build`) has not been run; `node_modules` and `bindings/generated` are absent in this checkout.
- The GTK surface compiles but has not been launched against a running `arutd`.
- Android and Apple builds have not been run; their adapters are template output and their views were reduced to bind to the new handles.
- Browser invalidation delivery has been exercised only by a Node smoke run, not in a page with a UI.
- No conformance suite exists for `BlobStore` yet; only `RpcChannel` and `FactLog`.

## Leftovers to clean

- `docs/adr/superseded/` holds the pre-reset ADRs for history; delete when no longer wanted.
- The old `backend/` directory is empty; the relay from roadmap Phase 1 will recreate it as `backend/relay`.
