---
status: accepted
---

# One observable handle per scope, bindings generated over BoltFFI, FFI attributes on projection types

The first slice implemented the chat view-model five times in five languages, each with a different reconciliation rule. We decided that surfaces observe scope handles (conversation list, one conversation, one composer, one operation, availability), each with its own watch, and compose them as the surface needs. Bindings are generated per ecosystem from the handle definitions over BoltFFI plus a thin wrapper of ours; hand-written binding code is a regression. Projection types carry the FFI data attribute in the feature crate and are re-exported, so each type is defined once. Rust-owned surfaces (GTK) read projection types directly.

## Considered options

- **One snapshot struct per screen.** Rejected: it recreates the monolithic state one level down and forces a collapsed sidebar to observe conversations it does not show.
- **UniFFI.** Rejected: no wasm target.
- **Hand-mirrored FFI types.** Rejected: three definitions of every type with `From` impls, verified in the crate sources to be unnecessary.

## Amendment (2026-09-14)

The generated alias facades are deleted, along with the generator that wrote them. Swift re-exports `ArutFfi`, the .NET props file carries `<Using Include="Arut.Ffi" />`, and Kotlin surfaces import `dev.arut.ffi`, so each generated type reaches a surface under its own name with no rename layer to keep in sync. The decision is unchanged: hand-written binding code is still a regression, and `arut-dev check` still refuses a surface that reaches around its binding package.
