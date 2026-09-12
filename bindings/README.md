# Bindings

`ffi` re-exports projection types annotated where they are defined. Scope exports
cover conversations, a conversation, its composer, and availability. `observation`
bridges watch wakeups to coalesced BoltFFI events without threads, including wasm.

Run `boltffi generate --deny-skipped` in `ffi` for raw bindings. Run
`python3 bindings/generate-observers.py` from the root for Swift, Kotlin, and
TypeScript scheduler adapters. These adapters contain no chat transitions.
Qt and the per-language chat models have been removed. Rust surfaces read the
product API directly.
