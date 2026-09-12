---
status: accepted
---

# Sans-I/O machines for control flow, async ports for effects, executor supplied by the composition root

The core must run on a desktop daemon, inside an Android service, in a browser, and on a cloud worker with the same tests. We decided that authority, session negotiation, stream resume, and draft replication are state machines that take typed inputs and return typed effects and never perform I/O. Model calls, files, processes, and clocks are async ports. A `Spawner` port provided by the composition root runs futures; no library crate creates a runtime. Tokio, wasm-bindgen-futures, and host executors live in runtime crates only.

## Considered options

- **Async traits everywhere.** Rejected for control flow: it ties tests to an executor and hides the ordering that authority and resume depend on.
- **Sans-I/O everywhere.** Rejected: a model call is an effect with a result, and forcing it through a machine adds ceremony with no testing gain.
