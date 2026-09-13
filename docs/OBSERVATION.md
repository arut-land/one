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
| TypeScript / React | One pending microtask and `useSyncExternalStore` | Subscription generations reject late callbacks. Replacement clears obsolete pending invalidations while retaining one scheduled microtask. Reads after subscription setup. |
| .NET adapter | Coalesced `SynchronizationContext.Post` | Checks the generation at callback entry and publication, including reads completed after replacement or disposal. Counts invalidations locally because source revisions can restart. Reads after subscribing. |
| Windows / WinUI | Shared .NET observer with injected `DispatcherQueue.TryEnqueue` | Removes the duplicate view adapter. Native presentation properties use `INotifyPropertyChanged`; ordered draft commands are separate from coalesced invalidations. |
| Linux / GTK | Direct watch refresh into GObject properties and a keyed gio::ListModel | One native GLib task per subscription, canceled with its component. No relm message queue between watch and refresh. |

Kotlin's [callbackFlow documentation](https://kotlinlang.org/api/kotlinx.coroutines/kotlinx-coroutines-core/kotlinx.coroutines.flow/callback-flow.html) defines cancellation cleanup and channel fusion. Windows uses the platform's [DispatcherQueue](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/migrate-to-windows-app-sdk/guides/threading).

## Verification

```sh
mise run check:apple
mise run check:observers-kotlin
mise run check:observers-dotnet
mise run check:ts
```

The Kotlin test project compiles the actual observer source on the JVM, using the coroutine test dispatcher. The .NET project compiles the actual observer with a queued synchronization context. Neither requires generated native bindings. Their burst, initial-read, replacement, and disposal checks complement the Swift and TypeScript tests. TypeScript tests run in the existing workspace check gate.

These are adapter tests, not complete application tests. The Windows build and UI validation are documented in [WINDOWS.md](WINDOWS.md). Burst tests check adapter refresh counts: 10,000 queued invalidations require one refresh in the TypeScript and .NET tests, and at most two in Kotlin because a suspended collector can already own one signal. No end-to-end latency or throughput benchmark has been recorded.

## Adapter contract evidence

| Adapter | Mechanism | Bounded coalescing test | Stale rejection test | No initial gap test |
| --- | --- | --- | --- | --- |
| Swift | One consumer, capacity-one AsyncStream, task cancellation, subscribe then read | `burstReadsLatestSnapshotOnce` | `replacingAndStoppingDiscardQueuedNotifications` | `subscriptionClosesInitialReadGap` |
| Kotlin / Android | Conflated callbackFlow, canceled collector, initial signal after subscribe | `burstAndLifecycle` | `burstAndLifecycle` | `burstAndLifecycle` |
| .NET | One pending context post, generation-checked publication, read after subscribe | `BurstReplacementAndDisposal` | `ReplacementOrDisposalDuringReadRejectsStaleSnapshot` | `BurstReplacementAndDisposal`, `SetupReadCannotOverwriteNewerRefresh` |
| TypeScript | One microtask, generation-checked callback, read after subscribe | `a burst refreshes once and dispose suppresses queued work` | `replacement rejects stale callbacks and queued invalidations` | `subscribe precedes the initial and replacement snapshots` |

Kotlin can consume one signal already held by the collector plus one conflated signal. Its work stays bounded at two reads per queued burst. Canceled collectors cannot publish into the replacement observation. TypeScript previously reused one callback across subscriptions, allowing late callbacks to schedule unnecessary replacement reads; the generation check now rejects them.

## GTK bridge

The Linux crate owns this adapter because it has one consumer. `Tasks::observe` reads after subscription setup and refreshes directly on the GLib main context. The Rust watch coalesces queued invalidations. Dropping the task owner aborts its sources before queued changes can refresh obsolete widgets. Composer commands use a separate ordered consumer, so coalescing notifications never discards edits. Visible text changes synchronously and ignores intermediate acknowledgements while commands are pending.

`Messages` implements `gio::ListModel` over immutable rows fetched with `messages_after(last_id)`. Each accepted row emits one `items_changed(position, 0, 1)`; existing row objects retain identity. GTK factories recycle widgets, and property expressions follow the currently bound conversation item. Status and draft properties use `bind_property`.

| Adapter | Mechanism | Test |
| --- | --- | --- |
| GTK coalescing and stale rejection | Watch plus component-owned GLib task, abort on drop | `burst_refreshes_once_and_cancellation_rejects_queued_updates` |
| GTK initial gap and property binding | Subscribe before first read; GObject binding | `setup_reads_after_subscription_and_properties_bind_without_display` |
| GTK transcript ranges | Keyed reads and single-row items_changed | `keyed_ranges_append_one_notification_and_preserve_objects` |
| GTK integration | Spawned arutd, recycled models, search, seven-line cap, ordered edits then send, unmount cleanup | `recycled_models_search_and_ordered_drafts_work_over_ipc`, ignored by default |

Run `cargo test -p arut-linux` without a display. With Wayland available, run `cargo test -p arut-linux -- --ignored --test-threads=1`. Smoke launches can set `ARUT_LINUX_APP_ID=dev.arut.SmokeTest` and temporary XDG data/state paths to avoid activating an existing personal instance.

The scheduler uses one native gtk-rs task per subscription. In glib 0.22.9 that is a task GSource plus a child waker GSource, both allocated at setup. Wakes mark the existing child ready; the bridge adds no source, task, thread, or allocation per invalidation. This is not literally a single GSource. Keeping GTK's scheduler avoids custom unsafe source dispatch and finalization code. No allocation or latency benchmark is claimed.

The .NET adapter checks the generation again when publishing a completed read. This matters when no SynchronizationContext exists and refresh uses the thread pool. `ReplacementOrDisposalDuringReadRejectsStaleSnapshot` fails on the previous adapter for both replacement and disposal. `ReplacementRevisionRestartDoesNotLosePendingChange` proves a new source's revision can restart without losing an invalidation queued during the old read. A local invalidation counter drives rescheduling; source revision values do not cross subscription lifetimes.

`SetupReadCannotOverwriteNewerRefresh` also covers a refresh completing while setup is still reading. Publication checks the local invalidation counter as well as the subscription generation, so the older setup snapshot cannot overwrite the newer value.

`StoppingObservationDiscardsQueuedReadsAndCanResume` covers callbacks queued before `StopObserving()`. Those callbacks must not read or publish the inactive source. Explicit `RefreshNow()` remains available for command acknowledgements, and a later `Observe()` resumes subscriptions normally.
