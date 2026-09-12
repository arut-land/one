# 0005: Sans-I/O core and runtime ports

## Status

Accepted as target architecture.

## Context

Core behavior should run with local desktop, cloud, sandboxed, test, and selected embedded runtimes without depending directly on Tokio or a platform SDK. Sans-I/O separates state machines from I/O, but it does not provide standard filesystem, process, clock, model, or storage traits.

## Decision

Core owns portable algorithms, reducers, protocol machines, and orchestration. Sans-I/O components consume typed inputs and emit effects where that model fits.

Project-owned feature ports describe external effects such as files, processes, clocks, models, persistence, and routes. Features depend on narrow capability bundles rather than one universal runtime interface. Runtimes implement and compose these ports using std, Tokio, WASI, Embassy, browser APIs, provider SDKs, or test doubles as appropriate.

Portability is declared per crate. Pure crates may support `no_std + alloc`; orchestration crates may require allocation, concurrency, or async execution. The architecture does not force every feature onto the smallest possible runtime.

## Consequences

- Core logic remains testable without real I/O.
- Tokio and platform SDKs stay in adapters that need them.
- Embedded and WASI targets can support honest subsets rather than fake every service.
- Feature-specific bundles keep generic bounds readable and prevent a service locator.
- Effect and cancellation semantics must be designed as part of each port.
