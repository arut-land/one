# Native observation

Rust owns the product projections and their revision streams. A surface subscribes to a stream of revisions and rereads the latest snapshot after each one; nothing but the revision number crosses the boundary. Transcript readers fetch every message after their last accepted key, so an invalidation may be dropped or coalesced but a message never is. Draft writes are acknowledged separately and coalesce by their own rule (ADR 0024).

## What Rust bounds

`Watch::update` compares a detached candidate and advances the revision only when the value changes, so an unchanged write notifies nobody. `EventSubscription::new(1)` gives each FFI stream a ring buffer of one: a burst of revisions arriving while a consumer is busy collapses to the latest, and the consumer reads current state on its next turn. A new subscriber is marked changed at setup, so the first read cannot fall into the gap between subscribing and reading.

## Delivery to each platform

The four exports in `bindings/ffi/src/lib.rs` declare `#[ffi_stream(item = u64, mode = "async")]`. BoltFFI 0.30.1 generates the ecosystem's own asynchronous sequence from that, so each platform cancels, buffers, and schedules with the mechanism its own developers already know.

| Platform | Generated type | How a surface consumes it |
| --- | --- | --- |
| Swift | `AsyncStream` | `for await _ in handle.chatChanges()` inside a task the view owns; cancelling the task unsubscribes |
| Kotlin / Android | `Flow` | `collect` in a `viewModelScope`, `conflate()` if the collector is slower than the source |
| C# / WinUI | `IAsyncEnumerable` | `await foreach` on the UI dispatcher, with a `CancellationToken` from the page's lifetime |
| TypeScript / React | async iterable | one `useSyncExternalStore` store of about 50 lines, which breaks out of the loop on unsubscribe |
| Linux / GTK | none; Rust `Subscription<u64>` | a GLib task per watch, refreshing GObject properties and the keyed `gio::ListModel` directly |

Breaking out of the loop, cancelling the task, or dropping the collector ends the underlying `StreamSession`, which unsubscribes and frees the handle. That is the whole lifecycle contract: there is no adapter class, no generation counter, and no hand-written coalescing, because the capacity-one subscription already bounds what a slow consumer sees.

## GTK bridge

The Linux crate consumes the watch directly rather than through FFI (ADR 0013, ADR 0021). `Tasks` owns one `glib::spawn_future_local` per subscription and aborts it on drop, so a dropped component cannot refresh obsolete widgets:

```rust
pub fn observe(&mut self, changes: Arc<Subscription<u64>>, mut refresh: impl FnMut() + 'static) {
    refresh();
    self.spawn(async move { while changes.changed().await.is_some() { refresh(); } });
}
```

The leading `refresh()` closes the subscribe-then-read gap. `Subscription::changed()` is a plain future, so no stream adapter sits between the watch and the loop. `Messages` implements `gio::ListModel` over immutable rows fetched with `messages_after(last_id)`; each accepted batch emits one `items_changed(position, 0, count)` and existing row objects keep their identity, so `GtkListView` recycles widgets and property expressions follow the bound conversation. The composer binds its text to `ComposerState.text`, which echoes locally and flushes last-writer-wins, so coalescing revisions cannot lose an edit.

The scheduler is gtk-rs's own. In glib 0.22.9 one task is a task GSource plus a child waker GSource, both allocated at setup; a wake marks the existing child ready. The bridge adds no source, thread, timer, or allocation per invalidation. No latency or allocation benchmark is claimed.

## Verification

```sh
cargo test -p arut-linux                                  # GTK bridge, no display needed
cargo test -p arut-linux -- --ignored --test-threads=1     # display-backed, needs Wayland
mise run check:ts                                          # TypeScript store and workspace types
mise run check:apple                                       # macOS build and Swift binding tests
```

`burst_refreshes_once_and_cancellation_rejects_queued_updates` in `surfaces/linux` is the test that states the coalescing and abort claims above. Smoke launches can set `ARUT_LINUX_APP_ID=dev.arut.SmokeTest` and temporary XDG data and state paths so they do not activate an existing personal instance.

These are bridge tests, not complete application tests. The Windows build and UI validation are documented in [WINDOWS.md](WINDOWS.md), the Apple ones in [APPLE.md](APPLE.md). No end-to-end latency or throughput benchmark has been recorded.
