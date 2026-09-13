# Native observation

Rust owns the product projections and their revision streams. `Watch` already coalesces revisions, and the FFI adapter uses a capacity-one BoltFFI `EventSubscription`. Consumers reread the latest snapshot after an invalidation. Transcript readers fetch every message after their last accepted key. These invalidations can be coalesced; message payloads and draft commands must not be dropped.

## BoltFFI delivery modes

The four exports in `bindings/ffi/src/lib.rs` explicitly use callback delivery. BoltFFI 0.30.1 also generates native async APIs: Swift `AsyncStream`, Kotlin `Flow`, and C# `IAsyncEnumerable`. Callback and async modes share BoltFFI's native subscription and batched transport. An async API is not, by itself, evidence of lower cost.

The pinned [Swift generator](https://docs.rs/crate/boltffi_backend/0.30.1/source/templates/target/swift/stream.swift) constructs its async stream with `.unbounded` buffering. Its callback mode lets the Swift adapter bound pending invalidations before entering the main actor. Switching all exports to async mode would remove that guarantee. No generated binding code is patched, no new polling loop is added, and no duplicate async exports are introduced.

The useful optimization is to bound notification work before it reaches the UI scheduler while letting each platform manage its own lifecycle.

| Platform | Delivery to UI | Changes and rationale |
| --- | --- | --- |
| Swift | One consumer task and `.bufferingNewest(1)` | Retains bounded delivery over BoltFFI callbacks. Tests cover cancellation, replacement, initial reads, and bursts. |
| Kotlin / Android | `callbackFlow.conflate()` into `StateFlow` | `awaitClose` owns FFI cleanup when collection is canceled. Replaces manual channel and subscription bookkeeping; conflation was already present. An initial signal reads after subscription setup. |
| TypeScript / React | One pending microtask and `useSyncExternalStore` | Retains the existing scheduling strategy. Reads the initial and replacement snapshots after subscription setup. |
| .NET adapter | Coalesced `SynchronizationContext.Post` | Binds callbacks to their subscription generation so canceled callbacks cannot masquerade as current ones. Reads after subscribing. |
| Windows / WinUI | Coalesced `DispatcherQueue.TryEnqueue` per subscription | Bounds pending dispatcher work and rejects callbacks from previous conversations. Reads composer text once per refresh. |
| Linux / GTK | Rust watch stream consumed on GLib | Already uses native Rust watch coalescing and component-owned tasks. No foreign binding or extra stream adapter is needed. |

Kotlin's [callbackFlow documentation](https://kotlinlang.org/api/kotlinx.coroutines/kotlinx-coroutines-core/kotlinx.coroutines.flow/callback-flow.html) defines cancellation cleanup and channel fusion. Windows uses the platform's [DispatcherQueue](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/migrate-to-windows-app-sdk/guides/threading).

## Verification

```sh
mise run check:apple
mise run check:observers-kotlin
mise run check:observers-dotnet
mise run check:ts
```

The Kotlin test project compiles the actual observer source on the JVM, using the coroutine test dispatcher. The .NET project compiles the actual observer with a queued synchronization context. Neither requires generated native bindings. Their burst, initial-read, replacement, and disposal checks complement the Swift and TypeScript tests. TypeScript tests run in the existing workspace check gate.

These are adapter tests, not complete Android or Windows application tests. This macOS machine lacks the Android SDK/NDK and WinUI runtime. The Windows view changes still need a Windows build and UI check. Burst tests check adapter refresh counts: 10,000 queued invalidations require one refresh in the TypeScript and .NET tests, and at most two in Kotlin because a suspended collector can already own one signal. No end-to-end latency or throughput benchmark has been recorded.
