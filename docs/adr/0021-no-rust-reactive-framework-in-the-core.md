---
status: accepted
---

# Observation stays a watch plus a callback stream; no Rust reactive framework in the core

Several Rust reactive systems exist: `nami` (WaterUI's core), Leptos's `reactive_graph`, Dioxus signals, `futures-signals`. Each was evaluated for replacing the watch substrate and the observation adapters. We decided against all of them. Our state lives in a multi-threaded daemon and crosses an FFI boundary into SwiftUI, Compose, WinUI, and React, where the platform's own observation is the idiom. `nami` is thread-confined by design, `reactive_graph` and Dioxus signals assume a Rust-owned view tree, and none of them reaches across FFI. WaterUI's own bridge exposes a binding as a watch callback with a guard handle, which is the same shape BoltFFI's callback streams already give us over `tokio::sync::watch`.

## Consequences

- The reactive substrate stays a small newtype over `tokio::sync::watch`; derived values are computed in projections, not in a signal graph.
- The four observation adapters stay thin and hand-written, one per ecosystem; the remaining boilerplate is the FFI export block per scope handle, which is a candidate for one project-owned macro, not a framework.
- Revisit only if a surface is itself written in Rust with a reactive UI toolkit, which today is only the Linux surface via relm4, and it consumes the watch directly.
