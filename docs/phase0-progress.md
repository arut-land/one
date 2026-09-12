# Phase 0 progress

This is an incomplete working checkpoint, not a completed Phase 0.

The project was not a Git repository when work started. No Git repository or
commits were created. The requested design documents were read before edits.

## Status

1. Watch: partial. `substrates/state` moved to `substrates/watch`, using Tokio
   watch with only `sync` enabled. Updates coalesce by revision. The Condvar
   queue, FFI bridge thread, and Qt observer thread were removed. The wrapper
   currently also contains scheduler callback registration; it is not yet the
   target's minimal newtype. Browser runtime delivery has not been exercised.
2. Spawner and Host: not started.
3. Typed scopes: not started.
4. Scope handles and generated bindings: not started. ChatModels remain.
5. Connect framing and streaming: not started.
6. Storage ports: not started.
7. Generic authority: not started.
8. Conformance suites: not started.

Steps 2 through 8 remain unimplemented because the required workspace test gate
cannot pass in this sandbox. The existing HTTP integration test binds a loopback
TCP listener and receives `PermissionDenied: Operation not permitted` at
`backend/composer/src/lib.rs`. This also failed before the watch changes.
Tests have not been disabled or changed to hide this failure. This session's
permission policy does not allow requesting elevation.

## Decisions

ADR 0018 and ARCHITECTURE.md explicitly make drafts ephemeral, with local recovery
through KeyValue. They take precedence over the contradictory Phase 0 roadmap
sentence saying drafts are facts. Storage work has not begun and the roadmap was
not edited.

The old Qt surface remains wired into the workspace. Only its observation adapter
was changed. Its command worker and polling remain for the later ordered steps.
No GTK surface was added.

## Verification environment

The configured sccache cannot execute in this sandbox. Commands use the system
C/C++ compilers and disable the Rust wrapper for the individual process. The
explicit protoc path is needed because the shortened PATH excludes mise tools.

```sh
export PATH=/usr/bin:/home/raj/.cargo/bin
export RUSTC_WRAPPER=
export PROTOC=/home/raj/.local/share/mise/installs/protoc/36.1/bin/protoc
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p arut-watch -p arut-feature-chat -p arut-product-session -p arut_ffi -p arut-bindings-qt
cargo check -p arut-watch --target wasm32-unknown-unknown
```

Workspace test: FAIL, denied loopback socket bind. Watch wasm compilation: PASS.
A wasm compilation check is not evidence of browser invalidation delivery.

## Layout audit

Only the watch directory has reached its target location. `transports/http`,
`backend/composer`, `bindings/qt`, and `surfaces/linux` remain. `runtimes/` and
`tools/conformance/` do not yet exist. The target layout is not achieved.

The five per-language ChatModels remain in Qt, Swift, Kotlin, TypeScript, and
.NET. Their deletion and replacement remain required.

## Final results for this checkpoint

- `cargo build --workspace`: PASS.
- `cargo test --workspace`: FAIL at the existing HTTP listener bind.
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS, exit 0.
- Focused watch, feature, session, FFI, and Qt tests: PASS, 16 tests.
- Watch `wasm32-unknown-unknown` check: PASS.

The build still emits upstream Qt C++ warnings and a gold linker warning. A
successful clippy exit does not mean the complete build output has zero warnings.
The focused tests exercise conversation creation/listing, independent composer
drafts, established sends, FFI access, and Qt updates.

Full logs for this session are in `/tmp/arut-phase0-build.log`,
`/tmp/arut-phase0-test.log`, `/tmp/arut-phase0-clippy.log`,
`/tmp/arut-phase0-core-test.log`, and `/tmp/arut-phase0-wasm.log`.
