# Ecosystem Map

What exists in the Rust ecosystem for each part of arut, with a verdict per crate. Surveyed 2026-09-13 against live crates.io and docs.rs data; wasm claims were verified by building for `wasm32-unknown-unknown` and inspecting `cargo tree` where the section says so. Re-run a section's survey before acting on it if more than a few months have passed.

## How to use this file

- **Verdicts.** ADOPT NOW: use it in the next change that touches the area. ADOPT AT PHASE n: use it when `docs/ROADMAP.md` reaches that phase. BORROW PATTERN: do not depend on it; study the named file or idea and write our own small version. REJECT: the reason is recorded so it is not proposed again.
- **License rule (ADR 0017).** MIT, Apache-2.0, BSD, MPL-2.0, Zlib, ISC are fine. LGPL only for dynamically linked system libraries such as GTK. GPL and AGPL are rejected. Elastic, BUSL, and FSL are flagged and need a deliberate decision.
- **wasm rule.** The BoltFFI wasm core does not use wasm-bindgen. Any crate whose graph contains `wasm-bindgen` cannot enter `features/`, `product/`, `substrates/`, or `bindings/ffi`. It may still be used in `runtimes/local` and the surfaces. CI should assert this with `cargo tree -i wasm-bindgen --target wasm32-unknown-unknown -p arut_ffi` returning nothing.
- **Standing sources.** When a need appears that this file does not cover, look in this order: [rust-unofficial/awesome-rust](https://github.com/rust-unofficial/awesome-rust), [uhub/awesome-rust](https://github.com/uhub/awesome-rust), [tecras/awesome-rust on Codeberg](https://codeberg.org/tecras/awesome-rust), [Rust LibHunt](https://rust.libhunt.com/), then [lib.rs](https://lib.rs), [docs.rs](https://docs.rs), and [crates.io](https://crates.io). Verify version, date, license, and wasm status before adopting; the lists are discovery, not endorsement.
- **Borrowing internals.** When a BORROW PATTERN entry names a file in another project, copying a small amount of MIT or Apache-2.0 code with attribution is fine; keep it under our own module with a comment naming the source.

## Actions this survey implies

Ordered by when they pay off. Each is small enough to be one commit.

1. **CI assertion** that no `wasm-bindgen` appears in the `arut_ffi` wasm dependency graph (the async section verified `n0-future`, and therefore iroh, would fail it).
2. **Watch substrate traps** found by inspection: bump the revision only when the value changed (`send_if_modified`), never re-enter `get()` inside `send_modify`, and document that the `mark_changed()` in `subscribe` is what delivers the first invalidation.
3. **Pin `schemars` 1.x** in `[workspace.dependencies]` before Phase 2; `rig-core`, `rmcp`, and `agent-client-protocol` all require it and two majors means two incompatible traits.
4. **`async-executor` as the host-polled `Spawner`** for wasm and the Android host thread; `tokio-util` cancellation tokens with `child_token()` for the scope tree.
5. **Spike the `connectrpc` crate** (0.9, Apache-2.0, passes the Connect conformance suite) against `transports/connect-http`; if it holds, ADR 0009's "immature" note is retired and the hand-written framing goes.
6. **`thiserror` for ADR 0016** typed errors; `snafu` rejected because its context idiom pushes strings back into the core.
7. **BLAKE3 digests** for `BlobStore` when `iroh-blobs` lands, replacing SHA-256.
   **Fact log store: `redb` typed tables, not SQL.** The log is six statements over opaque protobuf rows with no relational schema, so Diesel or SQLx would type-check nothing; `redb`'s `TableDefinition<K, V>` types keys and values at compile time, adds no C build, and needs no migration tooling beyond a `meta` schema-version table with Rust migration functions at open. SQLite returns later for a full-text-search read model (FTS5) behind a separate port, with `rusqlite_migration` for automatic versioning; never for the log.
8. **Pairing:** QR needs no PAKE; the short-code path runs SPAKE2 before any identity is revealed. Envelopes: sign, then `crypto_box::seal`; backup keys via HKDF from the root key.
9. **`keyring` 4 needs explicit store crates** per platform; mobile secrets go through the native side over FFI, not through the young mobile store crates.
10. **OpenTelemetry crates pull js-sys on wasm**; gate export behind a feature that is off in the wasm core.
11. **The first external harness adapter speaks ACP** (Agent Client Protocol); its schema crate is serde-only and wasm-clean, its runtime crate stays in `runtimes/local`.
12. **`mise run check` gains `cargo nextest` and `cargo machete`**; `cargo deny`, `semver-checks`, `hack`, and `mutants` are CI-only.

## Async, Sans-I/O, reactivity

wasm column verified by building each crate for `wasm32-unknown-unknown` on rust 1.98 and running
`cargo tree -i wasm-bindgen`; "no (bindgen)" means wasm-bindgen actually appears in the graph.

| Crate | Version | License | wasm-no-bindgen | Verdict | Why |
| --- | --- | --- | --- | --- | --- |
| tokio (`sync` only) | 1.53.1 | MIT | yes (built) | ADOPT NOW (already) | `watch`, `Notify`, `Semaphore`, `mpsc`, `oneshot`, `broadcast` all build wasm-clean; it already covers most primitives below. |
| tokio-util | 0.7.19 | MIT | yes (built) | ADOPT NOW | `sync::CancellationToken` + `child_token()` + `DropGuard` mirror the Node→Workspace→Conversation→Operation tree exactly; no feature flags needed, so no `rt` on wasm. |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | yes (built) | ADOPT NOW | The workspace sets `unsafe_code = "deny"`; this is the only way to hand-write a `Future`/`Stream` in a substrate without it. no_std, no proc-macro. |
| async-executor | 1.14.0 | Apache-2.0 OR MIT | yes (built) | ADOPT NOW (Phase 0 gap) | `LocalExecutor::try_tick()` verified on wasm — it *is* the BoltFFI host-polled `Spawner`. No threads, no I/O, no bindgen. Also the Android host-thread `Spawner`. |
| futures-lite | 2.6.1 | Apache-2.0 OR MIT | yes (built) | ADOPT NOW | Lighter combinator/`block_on` set than the `futures-util` currently in `substrates/watch`; same wasm story, smaller graph, no_std-capable. |
| futures-concurrency | 7.7.1 | MIT OR Apache-2.0 | yes (built) | ADOPT AT PHASE 1 | `Join`/`Race` over tuples+arrays and `ConcurrentStream::limit(n)` give parallel operations per node with no executor and no tokio `rt`. |
| futures-buffered | 0.2.13 | MIT | yes (built) | ADOPT AT PHASE 1 | `FuturesUnorderedBounded` — one allocation for N in-flight operations; the dynamic half of the per-node scheduler. |
| slotmap | 1.1.1 | Zlib | yes (built) | ADOPT AT PHASE 1 | Generational keys for the operation scheduler and scope-handle registry; an `OperationId` that cannot dangle after removal. |
| trait-variant | 0.1.3 | MIT OR Apache-2.0 | yes | ADOPT AT PHASE 1 | `#[trait_variant::make(Send)]` lets one `ModelProvider`/`Persist` definition serve the `!Send` wasm core and the `Send` daemon. rust-lang owned. |
| diatomic-waker | 0.2.3 | MIT OR Apache-2.0 | yes (built) | ADOPT AT PHASE 1 | Lock-free waker registration, no spin on the notify side; better than `atomic-waker` if the wasm poll export is hot. |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT | yes (built) | ADOPT AT PHASE 1 (fallback) | Stable and ubiquitous but last released 2023; use only if `diatomic-waker` proves awkward. |
| loom | 0.7.2 | MIT | n/a (dev-dep) | ADOPT AT PHASE 1 (dev) | Model-checks the watch substrate and any hand-written waker under the C11 model. Last release 2024 but still the standard. |
| embassy-sync | 0.8.0 | MIT OR Apache-2.0 | yes (built) | BORROW PATTERN | Copy `watch.rs`: `Watch<M,T,N>` with N *independently-acked* receivers (`changed()`/`try_changed()`) and the `RawMutex` generic. Don't take the dep — std exists on every target. |
| str0m | 0.23.1 | MIT OR Apache-2.0 | no (ring/getrandom) | BORROW PATTERN | Study `Rtc::poll_output() -> Output::{Timeout(Instant),Transmit,Event}` and `handle_input(Input::{Timeout,Receive})`. One three-way output enum a driver cannot forget to drain. |
| quinn-proto | 0.11.17 | MIT OR Apache-2.0 | no (ring/getrandom) | BORROW PATTERN | Study `src/connection/timer.rs` `TimerTable` — named timers in a fixed array merged into one `poll_timeout`; and the `Endpoint`/`Connection` split for session vs conversation machines. |
| futures-signals | 0.3.34 | MIT | yes (built) | BORROW PATTERN | ADR 0021 rejects the framework; steal `signal_vec::VecDiff` (`Replace/InsertAt/UpdateAt/RemoveAt/Move/Push/Pop/Clear`) as the transcript changed-range vocabulary. |
| async-broadcast | 0.7.2 | MIT OR Apache-2.0 | yes (built) | BORROW PATTERN | Steal `InactiveReceiver`: a scope handle with zero live observers must keep its channel open rather than close. `tokio::sync::broadcast` otherwise covers it. |
| statig | 0.4.1 | MIT | yes (built) | BORROW PATTERN | Only the `#[state]`/`#[superstate]` hierarchy idea, if session negotiation grows nested states. Its macro DSL hides the ordering ADR 0005 wants visible. |
| n0-future | 0.3.2 | MIT OR Apache-2.0 | **no (bindgen)** | REJECT | Verified: pulls `wasm-bindgen`, `js-sys`, `web-time`, `send_wrapper` on wasm32-unknown-unknown. Same reason iroh cannot enter the wasm core. |
| smol / async-io | 2.0.2 / 2.6.0 | Apache-2.0 OR MIT | no (needs polling/OS) | REJECT | OS reactor; no wasm, and Tokio already owns desktop and backend. |
| async-task | 4.7.1 | Apache-2.0 OR MIT | yes | REJECT (direct) | `async-executor` already wraps its `Runnable`/`Task` split; taking it directly means writing the queue yourself. |
| event-listener | 5.4.2 | Apache-2.0 OR MIT | yes (built) | REJECT (redundant) | `tokio::sync::Notify` under the `sync` feature you already compile everywhere covers the same need. |
| async-lock | 3.4.2 | Apache-2.0 OR MIT | yes (built) | REJECT (redundant) | `tokio::sync::{Mutex,RwLock,Semaphore}` already ship in the `sync` feature. |
| arc-swap | 1.9.2 | MIT OR Apache-2.0 | yes | REJECT (redundant) | `watch::Receiver::borrow()` is already the cheap read; a second cell duplicates the substrate. |
| crossbeam-channel / flume / postage / tachyonix | — | MIT/Apache-2.0 | mixed | REJECT | Blocking (wrong on the wasm main thread) or redundant with `tokio::sync::mpsc`. |
| pollster | 1.0.1 | Apache-2.0/MIT | yes | REJECT | `block_on` traps the wasm main thread; `futures-lite::future::block_on` covers the daemon's sync entry. |
| sansio | 1.0.1 | MIT/Apache-2.0 | yes | REJECT | A thin trait for the four methods `Machine` already declares; no ordering or timer help. |
| sans-io-runtime | 0.3.0 | MIT | yes | REJECT | ~1.3k recent downloads, last release 2024, opinionated worker/bus model that fights the `Spawner` port. |
| rust-fsm | 0.8.0 | MIT | yes | REJECT | Transition-table macro with no effect vocabulary and no timers. |
| typestate | 0.9.0-rc2 | MIT OR Apache-2.0 | yes | REJECT | Pre-release since 2021, abandoned. |
| state-shift | 2.1.1 | MIT | yes | REJECT | ~2.3k downloads; typestate via `PhantomData` + generics is twenty lines you should own. |
| sealed | 0.7.0 | MIT OR Apache-2.0 | yes (built) | REJECT | Sealing a capability bundle would break the blanket impl that ADR 0006 relies on as the extension point. |
| frunk / impl-trait-for-tuples | 0.5.0 / 0.2.3 | MIT, Apache-2.0/MIT | yes | REJECT | HLists for capability bundles cost the error messages supertraits give for free. |
| tokio_with_wasm / wasm_thread / send_wrapper | — | MIT / Apache-2.0 OR MIT | **no (bindgen)** | REJECT | All three are wasm-bindgen shims. |
| reactive_graph / nami / dioxus-signals | — | MIT / MIT / MIT-Apache | mixed | REJECT | Already rejected by ADR 0021; each assumes a Rust-owned view tree or thread confinement. |

### Patterns to copy, and traps

1. **str0m's one-enum output.** `poll_output() -> Timeout(Instant) | Transmit | Event` beats separate `poll_transmit`/`poll_event`: a driver physically cannot forget a variant. It maps onto `Machine::handle -> SmallVec<[Effect;4]>` if `Effect` carries the timeout arm instead of `next_timeout()`.
2. **quinn-proto `connection/timer.rs`.** A fixed array of *named* timers (`Timer::Close`, `Timer::Idle`, …) reduced to one `poll_timeout()`. Every machine you listed needs at least two timers; do not grow two `Option<Instant>` fields.
3. **Never call `Instant::now()` inside a machine** (Firezone's rule): `now` is a parameter on every input. That is what makes vector tests exact rather than approximate, and it is what ADR 0005's fake clock assumes.
4. **Trap: a `poll_timeout` that does not advance busy-loops the driver.** Put "timeout is strictly monotonic across a `handle_timeout`" in the machine conformance suite, not just in review.
5. **Trap: `tokio::sync::watch::borrow()` holds a read lock.** Never `send`/`send_modify` while a borrow is alive; the `Watch::update` closure in `substrates/watch/src/lib.rs` is already inside `send_modify`, so keep callers from re-entering `get()`.
6. **Use `send_if_modified` for coalescing at the source.** Today every `update` bumps the revision even when the projection is unchanged, so bindings hop the native scheduler for a no-op re-read.
7. **Trap: `Sender::subscribe()` starts "seen", `watch::channel()`'s receiver starts "unseen".** The `mark_changed()` in `Watch::subscribe` papers over that difference deliberately — it deserves a comment, because removing it silently drops the first invalidation.
8. **`InactiveReceiver` (async-broadcast).** A scope handle whose surface has navigated away has zero observers; the stream must stay open, not close and force a re-subscribe with a lost revision.
9. **embassy-sync's `Watch` receivers.** N receivers each with their own "seen" marker means coalescing happens *per binding*, not per cell — the right shape when SwiftUI and a GTK window observe the same conversation at different frame rates.
10. **`LocalExecutor::try_tick()` is the wasm `Spawner`.** Return whether it made progress so the host knows to reschedule, and bound the tick loop so one task cannot starve the frame.
11. **`CancellationToken::child_token()` is the scope tree.** Hang one token per scope struct and a `DropGuard` on each; cancelling a workspace then cancels every conversation and operation under it with no bookkeeping.
12. **`futures-concurrency`/`futures-buffered` keep the operation scheduler off tokio `rt`,** which does not exist on wasm. The scheduler stays a `Stream` the `Spawner` drives, not a runtime.
13. **`trait-variant` solves the `Send`/`!Send` split** you will hit the first time a wasm port holds a JS-side handle while the daemon needs `Send + Sync` bundles.
14. **Trap: iroh's dependency on `n0-future` means iroh transitively wants wasm-bindgen.** Keep it out of `bindings/ffi`'s wasm graph entirely; a `cargo tree -i wasm-bindgen --target wasm32-unknown-unknown` assertion in CI is cheap insurance.
15. **`loom` on the watch substrate before Phase 1,** while it is still ~120 lines — it is the one piece where a memory-ordering bug shows up as a dropped invalidation on one platform only.

## Connectivity, identity, encryption

| Crate | Version | License | wasm-no-bindgen | Verdict | Why |
| --- | --- | --- | --- | --- | --- |
| iroh | 1.2.0 (2026-09-09) | MIT/Apache-2.0 | no (browser build uses wasm-bindgen) | ADOPT NOW (ADR 0019) | Endpoint = device key, discovery, hole-punch, relay fallback, QUIC transport encryption. |
| iroh-relay | 1.2.0 (2026-09-09) | MIT/Apache-2.0 | n/a (server binary) | ADOPT NOW | The backend relay deployment; pairing service sits beside it. |
| iroh-net-report | 0.34.1 (2025-04-07, stale) | MIT/Apache-2.0 | n/a | REJECT (direct dep) | Reachability/NAT reporting now folded into `iroh` core; no reason to depend on it separately. |
| iroh-docs | 0.101.0 (2026-06-15) | MIT/Apache-2.0 | no | REJECT | Multi-writer CRDT sync; ARCHITECTURE.md bans CRDTs for authority-owned state. Drafts already use gossip + LWW, not this. |
| crypto_box | 0.9.1 (2025-10, 0.10 pre exists) | Apache-2.0/MIT | yes | ADOPT NOW | X25519+XSalsa20-Poly1305 `seal`/`SealedBox`, libsodium-compatible anonymous sealed box — matches ADR 0003/0019 wording directly. |
| chacha20poly1305 | 0.11.0 (2026-06-28) | Apache-2.0/MIT | yes | ADOPT NOW | AEAD for content keys derived from the root key (backups, at-rest envelopes). |
| ed25519-dalek | 3.0.0 (2026-07-06) | BSD-3-Clause | yes | ADOPT NOW | Application-level command/envelope signing; same primitive family iroh uses for endpoint keys. |
| x25519-dalek | 3.0.0 (2026-07-06) | BSD-3-Clause | yes | ADOPT NOW | Key agreement under crypto_box; also for deriving per-recipient content keys from the root key. |
| hpke (RustCrypto) | 0.14.1 (2026-09-06) | Apache-2.0/MIT | yes (no_std) | ADOPT AT PHASE 3 | RFC 9180 multi-recipient sealing; keep for account root-key escrow when >1 recipient must open a backup. crypto_box covers single-recipient now. |
| age | 0.12.1 (2026-07-14) | Apache-2.0/MIT | partial (wasm perf-timer feature needs web-sys/bindgen; core doesn't) | REJECT | File-format+recipient-list tool, heavier than one fixed sealed envelope; crypto_box already fits. |
| snow | 0.10.0 (2025-07-19) | Apache-2.0/MIT | yes | REJECT | Noise handshake no longer needed: ADR 0003 amendment moved transport encryption to iroh's QUIC. |
| spake2 | 0.4.0 stable / 0.5.0-pre (2026-01-25) | MIT/Apache-2.0 | yes (curve25519-dalek based) | ADOPT AT PHASE 1 | Symmetric PAKE for short-code pairing; RustCrypto/PAKEs, lineage from Magic Wormhole, actively updated. |
| pake-cpace / cpace family | 0.1.7 (2023-12-15, stale) | ISC | unclear (fragmented impls) | REJECT | Ecosystem split across 4 tiny unmaintained crates; no clear pure-Rust wasm-friendly winner. spake2 does the same job and is maintained. |
| opaque-ke | 4.0.1 stable / 4.1.0-pre (2026-03-27) | Apache-2.0/MIT | yes | REJECT | Asymmetric client-server aPAKE (registration + server envelope); wrong shape for two-device symmetric pairing. Roadmap uses OAuth for accounts, not passwords. |
| qrcode | 0.14.1 (2024-07-05) | MIT/Apache-2.0 | yes | ADOPT NOW | Pure-Rust QR matrix generation for the pairing QR; surface renders the matrix itself. |
| rqrr | 0.11.0 (2026-09-02) | (MIT/Apache-2.0) AND ISC | yes | ADOPT NOW | Pure-Rust QR decode from a camera frame buffer for the scanning side of pairing. |
| btleplug | 0.13.0 (2026-08-31) | MIT/Apache-2.0/BSD-3-Clause | n/a (native) | ADOPT AT PHASE 5 | Cross-platform BLE central (Win/macOS/Linux/Android via JNI) for nearby pairing without a network. |
| bluer | 0.17.4 (2025-06-06) | BSD-2-Clause | n/a (native, Linux-only) | BORROW PATTERN | BlueZ D-Bus API; use only if btleplug's Linux GATT-server support proves insufficient — copy its peripheral-mode approach then, don't add as a second BLE dep. |
| mdns-sd | 0.21.3 (2026-09-08) | Apache-2.0/MIT | n/a | REJECT | LAN discovery is iroh's job now (ADR 0012 amended). |
| libp2p | 0.57.0 (2026-09-11) | MIT | no (browser transports need bindgen) | REJECT (confirmed) | Larger surface, weaker relay story, no key-as-address model (ADR 0019). |
| vodozemac | 0.11.0 (2026-09-11) | Apache-2.0 | yes | BORROW PATTERN | Matrix's Olm/Megolm device-trust model: copy the per-device signing key that vouches for a shared secret, plus device-verification/revocation list, once >2 paired devices need transitive trust. |
| matrix-sdk-crypto | 0.18.0 (2026-06-02) | Apache-2.0 | no (sqlite/store-heavy) | BORROW PATTERN | Same cross-signing idea as vodozemac, plus its backup-key escrow shape for Phase 3 account recovery. Too heavy to depend on directly. |
| libsignal (signalapp) / libsignal-protocol (crates.io) | crates.io crate abandoned at 0.1.0 (2019) | AGPL-3.0 (official repo) | — | REJECT | AGPL fails the license rule outright; also no maintained crates.io wrapper. Double Ratchet isn't needed anyway — single-authority fact log plus sealed envelopes covers the threat model. |
| tokio-tungstenite | 0.30.0 (2026-07-11) | MIT | n/a (backend/server only) | ADOPT NOW | Relay-side WebSocket path (axum) for browser reach, per ADR 0009/0019; browser side uses the native WebSocket API via host callback, not this crate. |
| rustls | 0.23.44 (2026-09-07) | Apache-2.0/ISC/MIT | n/a (native/server) | ADOPT NOW | TLS termination for the relay's HTTPS/WSS endpoint under axum. |

Notes:
- Pairing protocol: two paths, both inside an iroh connection. **QR path**: the QR already carries the target endpoint id (high-entropy, authenticated dial target), so no PAKE is needed — open the iroh stream, seal the root key with `crypto_box::seal` to the scanned key, done. **Short-code path** (no camera): run **SPAKE2** over the fresh stream keyed by the human-typed code before either side reveals its endpoint id or root key; this defeats offline brute-force of a short numeric code and blocks an active MITM carrier. CPace crates are rejected only for staleness/fragmentation, not the protocol idea — revisit if a maintained pure-Rust CPace appears.
- Sealed-envelope construction: `crypto_box::seal` (X25519 + XSalsa20-Poly1305, libsodium sealed-box compatible) per envelope and per backup blob, addressed to a recipient key derived from the root key; the plaintext command is signed with `ed25519-dalek` before sealing so the recipient (never the carrier) can verify sender authenticity after opening. Backup blobs use the same seal call with a key derived via HKDF-SHA256 from the root key, one per checkpoint, so the relay literally cannot read them (ADR 0002/0003).
- `hpke` is the RFC 9180 fallback if Phase 3 escrow ever needs one ciphertext openable by several recipients (device + account); not needed for the current single-recipient case.
- Everything marked wasm-no-bindgen "yes" above is pure-Rust/RustCrypto-family and fine inside the BoltFFI wasm core; iroh itself is excluded from the web surface for exactly that reason (ADR 0019), which is why the relay's WebSocket path (tokio-tungstenite + rustls, server-side) is the browser's only route in.

## Storage, logs, serialization

| Crate | Version | License | Mobile | wasm | Verdict | Why |
| --- | --- | --- | --- | --- | --- | --- |
| redb | 4.2.0 (Aug 2026) | MIT/Apache-2.0 | yes (pure Rust, mmap) | no | ADOPT AT PHASE n | Pure-Rust single-file MVCC B-tree, no C toolchain; strong FactLog/KeyValue candidate, active |
| fjall | 3.1.10 (Aug 2026) | MIT/Apache-2.0 | yes (pure Rust) | no | ADOPT AT PHASE n | LSM-tree = literally append+compact; use if write throughput ever dominates over redb's txn model |
| rusqlite (bundled) | 0.40.2 (Aug 2026) | MIT | yes (cc/NDK proven) | no | ADOPT AT PHASE n (recommended) | Matches ADR 0010's stated "SQLite first, deferred"; mature mobile story, WAL + real transactions |
| sqlx | 0.9.0 (May 2026) | MIT/Apache-2.0 | yes | no | REJECT | Async-only API fights the sync FactLog/KeyValue port trait; duplicates rusqlite's bundled sqlite path |
| sled | 0.34.7 (Oct 2024) | MIT/Apache-2.0 | yes | no | REJECT | Unmaintained (>1yr), historical space-amp/correctness issues, author paused it for a rewrite |
| rocksdb | 0.25.0 (Aug 2026) | Apache-2.0 | heavy (C++) | no | REJECT | C++ dep makes 5-target cross-compile (esp. iOS static link) heavy; fjall/redb cover the same ground |
| heed (LMDB) | 0.22.1 (Apr 2026) | MIT | yes (C lib) | no | REJECT (fallback only) | Wraps liblmdb (C); redb gives similar mmap-B-tree semantics in pure Rust |
| lmdb-rkv | 0.14.0 (2020) | Apache-2.0 | yes | no | REJECT | Abandoned since 2020, superseded by heed |
| okaywal | 0.3.1 (2023) | MIT/Apache-2.0 | n/a | n/a | BORROW PATTERN | Copy: rotated fixed-size WAL segments + CRC framing + a checkpoint marking segments fully applied |
| marble | 16.0.2 (Feb 2025) | MIT/Apache-2.0 | n/a | n/a | BORROW PATTERN | Copy: generational GC of on-disk objects, for in-place BlobStore compaction of orphaned blobs |
| iroh-blobs | 0.103.0 (Jun 2026) | MIT/Apache-2.0 | yes (project dep) | no | ADOPT AT PHASE 1 | Already required by ROADMAP step 7; its own store can implement BlobStore directly, verified partial DL |
| bao-tree | 0.16.1 (Aug 2026) | MIT/Apache-2.0 | transitive | n/a | ADOPT AT PHASE 1 (transitive) | Comes in via iroh-blobs; pattern source for chunk-group verified range reads if a custom store is ever built |
| blake3 | 1.8.7 (Aug 2026) | CC0/Apache-2.0 | yes | yes | ADOPT AT PHASE 1 | Needed to align blob addresses with iroh-blobs, which hashes BLAKE3 not SHA-256 |
| postcard | 1.1.3 (Jul 2025) | MIT/Apache-2.0 | yes | yes | REJECT | ADR 0015 mandates protobuf for every boundary value; second wire format buys nothing |
| rkyv | 0.8.18 (Aug 2026) | MIT | yes | yes | REJECT | Zero-copy is nice but conflicts with protobuf-everywhere; revisit only if snapshot decode is a measured bottleneck |
| prost | 0.14.4 (Jun 2026) | Apache-2.0 | yes | yes | ADOPT NOW | Already adopted; `Fact` trait bound is `prost::Message`, matches ADR 0015 |
| serde | 1.0.229 (Jul 2026) | MIT/Apache-2.0 | yes | yes | ADOPT NOW | Fine for non-boundary Rust-only types (config chain); not used for fact rows |
| cbor4ii | 1.2.3 (Sep 2026) | MIT | yes | yes | REJECT | Redundant with prost under the single-wire-format rule |
| fs4 | 1.1.0 (Apr 2026) | MIT/Apache-2.0 | yes | n/a | ADOPT AT PHASE n | Only if shared/read locks or async-lock-across-await are needed; std already covers exclusive locks |
| fd-lock | 4.0.4 (2025) | MIT/Apache-2.0 | yes | n/a | REJECT | Superseded by stabilized `std::fs::File::lock()`/`try_lock()` (Rust 1.89), already in use in `directory.rs` |
| atomicwrites | 0.4.4 (2024) | MIT | yes | n/a | BORROW PATTERN | `directory.rs::atomic()` already implements its temp+fsync+rename+fsync-dir sequence; no dependency needed |
| tempfile | 3.27.0 (Mar 2026) | MIT/Apache-2.0 | yes | n/a | ADOPT NOW | Swap hand-rolled temp/rename for `NamedTempFile::persist()`: more robust Windows replace-on-open semantics |
| directories | 6.0.0 (Jan 2025) | MIT/Apache-2.0 | n/a (desktop) | n/a | ADOPT AT PHASE n | Resolve desktop data dir (XDG/AppData/Library) at daemon startup; mobile/wasm get paths from the host |
| etcetera | 0.11.0 (Oct 2025) | MIT/Apache-2.0 | n/a (desktop) | n/a | REJECT | Same job as `directories`, smaller user base; pick one |
| refinery | 0.9.2 (Jun 2026) | MIT | yes | n/a | ADOPT AT PHASE n | Pairs with rusqlite (has a rusqlite feature) for embedded migrations once SQLite lands |
| sqlx-cli / sqlx migrate | 0.9.0 (May 2026) | MIT/Apache-2.0 | n/a | n/a | REJECT | Tied to sqlx's async engine, which is rejected above |
| cqrs-es | 0.5.0 (Dec 2025) | Apache-2.0 | n/a | n/a | BORROW PATTERN | `Authority<C>` already exceeds it (epoch fencing, idempotency); skim only its `GenericQuery` read-model replay |
| esrs | 0.18.0 (Nov 2024) | MIT/Apache-2.0 | n/a | n/a | BORROW PATTERN | Copy: "policy" concept — side effects reacting to persisted events, run outside the write path |
| disintegrate | 4.0.0 (Feb 2026) | MIT | n/a | n/a | BORROW PATTERN | Copy: deriving a command's precondition from a query across multiple fact types, not one scope's revision |
| thalo | 0.8.0 (2023) | Apache-2.0/MIT | n/a | n/a | REJECT | Stale since 2023; wasm-actor runtime shape, not a storage library |
| eventually | 0.4.0 (2020) | MIT | n/a | n/a | REJECT | Abandoned since 2020 |

Notes:
- Recommended first non-directory `FactLog`/`KeyValue` backend: **rusqlite with bundled SQLite** — matches ADR 0010's own "SQLite first, deferred not rejected"; mature Android NDK / iOS cc builds; one file gives WAL durability plus real cross-table transactions (facts table keyed by sequence, unique index on command_id for `outcome_of`, kv table, snapshot row) while still storing `Fact` as protobuf-encoded blobs, preserving ADR 0015.
- `redb` is the pure-Rust fallback if a C toolchain across Android/iOS/Windows becomes the constraint; `fjall` (LSM) if append-throughput ever outweighs the need for multi-table transactions.
- Cross-process locking for the desktop daemon: keep `std::fs::File::lock()`/`try_lock()` (stabilized 1.89, already used per-log in `directory.rs`) — add one daemon-wide `arutd.lock` file taken with `try_lock()` in the composition root at startup so a second `arutd` instance fails fast instead of corrupting state; reach for `fs4` only if shared/read locks or locking across an await point are later needed.
- `iroh-blobs` is already the Phase 1 `BlobStore` per ROADMAP step 7; when it lands, migrate blob digests from SHA-256 (current `digest()` in `storage/lib.rs`) to BLAKE3 so addresses match iroh-blobs' own hashing.
- wasm: nothing here targets wasm directly; keep the existing `MemoryLog`/`MemoryStore` until an OPFS-backed impl is justified — that would be hand-written against `web-sys`/host callbacks, not a crate.
- Rows stay protobuf-only per ADR 0015; postcard/cbor4ii/rkyv are rejected as second wire formats, not for quality.
- Event-sourcing crates are pattern sources only, per the task's own framing — none is a dependency candidate.

## FFI, codegen, RPC

| Crate | Version | License | wasm-no-bindgen | Verdict | Why |
|---|---|---|---|---|---|
| BoltFFI | 0.30.1 (2026-08-17) | MIT | yes (host-polled) | ADOPT NOW | Already the pin in `bindings/ffi/Cargo.toml`; it *is* current latest. Since 0.30.0: binding-IR migration for Kotlin/Dart, TS abort-signal cancellation, wasm perf/size work — no breaking change to the `#[export]` shape we depend on. |
| UniFFI | 0.32.1 (2026-09-09) | MPL-2.0 | no | REJECT | Confirmed per ADR 0007: no wasm target. |
| Diplomat | 0.16.1 (2026-08-20) | MIT/Apache-2.0 | partial (own JS-wasm wrapper, no wasm-bindgen dep) | REJECT | No unified Swift+Kotlin+C#+Dart pass and no callback/stream primitive matching our `EventSubscription`; would still need our watch-bridge layer plus a second binding generator to maintain. |
| typeshare | 1.0.5 (2026-01-02) | MIT/Apache-2.0 | n/a | REJECT | Mirrors serde type shapes only; no calls, no streams, no handles — exactly the "hand-mirrored types" ADR 0007 already rejected. |
| swift-bridge | 0.1.59 (2026-01-06) | MIT/Apache-2.0 | no (Swift-only) | REJECT | Single-ecosystem, low cadence (last release Jan 2026); duplicates BoltFFI's Swift target for no gain. |
| jni | 0.22.4 (2026-03-16) | MIT/Apache-2.0 | n/a (JVM only) | BORROW PATTERN | Not a boundary crate — BoltFFI already emits the JNI target. Reach for raw `jni` only underneath BoltFFI for an Android platform shim BoltFFI's IR can't express. |
| ndk | 0.9.0 (2024-04-26) | MIT/Apache-2.0 | n/a | REJECT (stale) | Over two years since last release; same "shim only" caveat as `jni` if ever needed. |
| windows (windows-rs) | 0.62.2 (2025-10-06) | MIT/Apache-2.0 | n/a (Windows only) | ADOPT AT PHASE n | For WinUI-surface platform shims (keychain-equivalent, native dialogs) once that surface starts, per ARCHITECTURE's platform table — not an FFI-boundary crate. |
| cxx | 1.0.202 (2026-09-12) | MIT/Apache-2.0 | n/a | REJECT | No C++ dependency exists to bridge; BoltFFI already owns the exported boundary. |
| wasm-bindgen | 0.2.128 (2026-09-04) | MIT/Apache-2.0 | is wasm-bindgen | REJECT | Conflicts directly: BoltFFI's wasm target is host-polled with no JS glue layer; adding wasm-bindgen means two competing wasm ABIs and drags `reqwest`-style JS shims into the wasm core, which ADR 0009/ARCHITECTURE explicitly keep out. |
| prost-reflect | 0.16.5 (2026-07-09) | MIT/Apache-2.0 | yes (pure Rust) | ADOPT AT PHASE n | Walk the `FileDescriptorSet` already produced by `protox::compile` inside `ArutServiceGenerator` to emit per-service typed availability structs — this is the ROADMAP/ADR-0008 "generator will also emit typed availability" item. |
| protox | 0.9.1 | MIT/Apache-2.0 | yes | ADOPT NOW | Already adopted in `protocols/build`; current version. |
| pbjson | 0.9.0 (2025-12-09) | MIT | yes | ADOPT AT PHASE n | Only needed if Connect framing grows a canonical proto-JSON content type for browsers beyond today's binary + JSON-error-envelope; not needed yet. |
| tonic | 0.14.6 (2026-05-07) | MIT | n/a (server-only) | REJECT | gRPC/HTTP2, not Connect; ADR 0009 already rejected gRPC+grpc-web for browser reach. Worth reading its interceptor/TLS layering as a pattern, nothing more. |
| tonic-web | 0.14.6 | MIT | n/a | REJECT | Solves grpc-web, not the Connect protocol we standardized on. |
| connectrpc (connectrpc/connect-rust) | 0.9.0 (2026-08-25) | Apache-2.0 | n/a (server-only) | ADOPT AT PHASE n | **Best current match** for replacing the hand-written framing: tower-based, mounts as an axum fallback service, passes the full Connect conformance suite (3,600 server / 6,872 client tests) per its README. Still pre-1.0. Worth a spike behind the existing `RpcChannel`/`arut-transport-connect-http` seam since framing is already isolated from product code — low blast radius if it doesn't pan out. |
| connectrpc-axum | 0.2.3 (2026-08-05) | MIT | n/a | REJECT (for now) | Thinner axum-native wrapper over the same protocol, explicitly flagged "not recommended for production yet" upstream; re-evaluate only if the base `connectrpc` crate stalls. |
| axum-connect | 0.5.3 (2025-06-13) | MIT/Apache-2.0 | n/a | REJECT | Over a year stale, superseded by `connectrpc`/`connectrpc-axum`. |
| prost-protovalidate | 0.6.0 (2026-07-10) | MIT/Apache-2.0 | yes (uses prost-reflect) | ADOPT AT PHASE n | Evaluates `buf.validate` CEL constraints against descriptors at runtime; natural pairing once protos start declaring validation rules and once `prost-reflect` is in the tree. |
| darling | 0.24.1 (2026-08-20) | MIT | n/a (build-time) | ADOPT AT PHASE n | Attribute parsing for the one project-owned FFI export macro (see sketch below). |
| syn | 3.0.5 (2026-09-04) | MIT/Apache-2.0 | n/a | ADOPT AT PHASE n | Foundation for the macro; current major (3.x). |
| quote | 1.0.47 (2026-07-19) | MIT/Apache-2.0 | n/a | ADOPT AT PHASE n | Foundation for the macro. |
| proc-macro-error2 | 2.0.1 (2024-09-06) | MIT/Apache-2.0 | n/a | REJECT (stale) | Two years without a release; borrow its span-pointing diagnostic *pattern* but emit errors via plain `syn::Error`/`compile_error!` instead of taking the dependency. |
| macrotest | 1.2.1 (2026-02-15) | MIT/Apache-2.0 | n/a | ADOPT AT PHASE n | Golden-file expansion tests for the new macro's generated code. |
| trybuild | 1.0.121 (2026-09-08) | MIT/Apache-2.0 | n/a | ADOPT AT PHASE n | UI/compile-fail tests for macro misuse (wrong handle shape, missing `Send + Sync`, etc). |
| cargo-expand | 1.0.126 (2026-08-19) | MIT/Apache-2.0 | n/a | ADOPT NOW | Dev-only tool, not a dependency; use immediately while designing the macro. |

## Notes

- **Connect framing verdict**: no Rust Connect implementation is mature enough to *replace* the hand-written `transports/connect-http` framing today, but `connectrpc` (the `connectrpc/connect-rust` project) is close — it is conformance-tested and already axum-shaped. ADR 0009's "Rust Connect server libraries are immature" should be revisited at the next transport-layer milestone, not accepted as permanent; a spike swapping only `transports/connect-http`'s server half (client stays hand-rolled `reqwest`, since it's simple and works) is the lowest-risk trial, since `RpcChannel` already isolates the framing from every feature.
- `protovalidate` on crates.io is a placeholder (version `0.0.0`, name reserved) — do not depend on it; `prost-protovalidate` is the real, maintained implementation.
- No candidate beats BoltFFI for the actual export boundary; all UniFFI/Diplomat/typeshare rejections in ADR 0007/ARCHITECTURE are reconfirmed on current versions.

### Sketch: project macro for scope-handle FFI export blocks

Goal: replace the repeated `pub struct XHandle { .. } #[export] impl XHandle { .. }` blocks in `bindings/ffi/src/lib.rs` with one declarative macro, built later with `darling` + `syn` + `quote`:

```rust
arut_scope_handle! {
    struct ChatHandle { client: ChatClient }
    impl {
        fn state(&self) -> ChatState => client.state();
        async fn send(&self, text: String) -> ChatState => client.send(text);
        stream fn chat_changes(&self) -> u64 => client.changes(); // expands to ffi_subscription + #[ffi_stream]
    }
}
```
The macro expands to the handle struct, the `#[export] impl`, and the `ffi_subscription(...)` wiring already hand-written per scope — `stream fn` is sugar specifically for the `Subscription<u64>` → `EventSubscription<u64>` bridge in `observation.rs`, since every scope repeats that pattern verbatim. Validate the macro with `trybuild` (reject a `stream fn` on a non-`u64` item, reject a handle missing `Send + Sync`) and `macrotest` (freeze the expansion of `ChatHandle` itself as a golden file) before cutting the four hand-written handles over.

## Native UI and platform integration from Rust

Verified on crates.io/docs.rs 2026-09-13. Build host has GTK 4.22.5, so the `v4_20` feature gate is safe today.

| Crate | Version | License | Platform | Verdict | Why |
| --- | --- | --- | --- | --- | --- |
| relm4 | 0.11.0 (2026-04-08) | MIT OR Apache-2.0 | Linux | ADOPT NOW | ADR 0020 confirmed: `AsyncComponent`, `FactoryVecDeque`/`AsyncFactoryComponent`/`FactoryHashMap`, `Worker`, `typed_view`. Deps `gtk4 ^0.11.2`, MSRV 1.93, `libadwaita` is an opt-in feature we leave off. 0.11.0 currently fails the docs.rs build; read 0.10.0 docs. |
| relm4-components | 0.11.0 | MIT OR Apache-2.0 | Linux | ADOPT AT PHASE 1 | `OpenDialog`, `SaveDialog`, `alert`, `simple_combo_box` — removes hand-wired dialog plumbing from the surface. |
| relm4-icons | 0.11.0 | (MIT OR Apache-2.0) AND CC0-1.0 | Linux | REJECT | Bundles a GNOME icon set; overriding the running desktop's icon theme is exactly what ADR 0013 forbids. |
| gtk4 / gdk4 | 0.11.4 (2026-06-29) | MIT | Linux | ADOPT NOW | Already in `surfaces/linux-gtk`. Bump the dep to enable `v4_20` for `gtk_interface_color_scheme()` and `gtk_interface_contrast()`. |
| glib / gio | 0.22.9 | MIT | Linux | ADOPT NOW | Transitive. `gio::Notification` + `ApplicationExt::send_notification` is the notification path; it routes through the portal under Flatpak with no extra crate. |
| ashpd | 0.13.13 (2026-07-17) | MIT | Linux | ADOPT NOW (`settings`) / PHASE 4 (rest) | Portals as typed zbus proxies: `settings` (color scheme, accent, contrast, reduced motion + change streams), `background` (autostart), `notification`, `file_chooser`, `secret`, `global_shortcuts`, `inhibit`, `open_uri`. Features `gtk4` (gives `From<Color> for gdk4::RGBA`), `glib`, `tokio`. |
| libadwaita | 0.9.2 | MIT | Linux | REJECT | Confirmed: it hard-codes the Adwaita stylesheet and ignores `gtk-theme-name`, so a KDE/XFCE/COSMIC desktop gets a GNOME-looking app. ADR 0013 stands. |
| zbus | 5.19.0 | MIT | Linux | ADOPT NOW (transitive) | Arrives under ashpd/keyring. Do not hand-roll portal D-Bus calls on top of it. |
| notify-rust | 4.18.0 | MIT OR Apache-2.0 | desktop | REJECT for GTK; ADOPT AT PHASE 4 for terminal | `gio::Notification` already covers the GTK surface; a headless CLI/TUI has no GApplication and needs this. |
| tray-icon | 0.25.0 (2026-09-11) | MIT OR Apache-2.0 | desktop | ADOPT AT PHASE 4 | Tauri's tray. On Linux pick the `ksni` feature, not `libappindicator` (LGPL-3.0 C lib, an extra runtime dep). Pulls `muda`, `objc2-app-kit`, `windows-sys`. |
| ksni | 0.3.6 | Unlicense (public domain) | Linux | ADOPT AT PHASE 4 | Pure-Rust StatusNotifierItem; the only tray path that works on KDE/sway without libappindicator. Use directly if only Linux needs a tray. |
| rfd | 0.17.2 | MIT | desktop | ADOPT AT PHASE 1 | Attachment picker. Feature `xdg-portal` (+`wayland`), never `gtk3`, so it uses the FileChooser portal. Alternative: `gtk::FileDialog` needs no crate at all — prefer it in the GTK surface and keep rfd for the terminal surface. |
| keyring | 4.2.0 (2026-08-29) | MIT OR Apache-2.0 | desktop | ADOPT NOW | Already named in ARCHITECTURE. v4 is a facade: pick a store crate explicitly, or set feature `v1` for the old bundled behavior. |
| zbus-secret-service-keyring-store | 1.0.1 | MIT OR Apache-2.0 | Linux | ADOPT NOW | The v4 Linux store; keeps everything on the zbus already linked. `linux-keyutils-keyring-store` 1.0.0 as the headless/session fallback. |
| apple-native-keyring-store | 1.0.2 | MIT OR Apache-2.0 | macOS, iOS | ADOPT AT PHASE 1 | Keychain store for the v4 facade; no objc2 code of ours. |
| windows-native-keyring-store | 1.1.0 | MIT OR Apache-2.0 | Windows | ADOPT AT PHASE 4 | Credential Manager store. |
| android-native-keyring-store | 1.0.0 | MIT OR Apache-2.0 | Android | ADOPT AT PHASE 1 | Android Keystore behind the same `keyring` port — cheaper than writing the JNI keystore calls by hand. |
| oo7 | 0.6.0 | MIT | Linux | REJECT | Good pure-Rust Secret Service + secret-portal client, but it is a second secrets API beside `keyring`; revisit only if the keyring store proves unreliable. |
| secret-service | 5.2.0 | MIT OR Apache-2.0 | Linux | REJECT | One layer below the keyring store crate we already take. |
| objc2 | 0.6.4 | MIT | Apple | ADOPT AT PHASE 1 | Only inside `runtimes/apple`, which needs `#![allow(unsafe_code)]` against the workspace `unsafe_code = "deny"`. |
| block2 | 0.6.2 | MIT | Apple | ADOPT AT PHASE 1 | Completion handlers for every async AppKit/UIKit call reached from Rust. |
| objc2-service-management | 0.3.2 | Zlib OR Apache-2.0 OR MIT | macOS | ADOPT AT PHASE 2 | `SMAppService` + `SMAppServiceStatus` are bound; this is the ADR 0011 "login item later" path with no Swift shim. |
| objc2-background-tasks | 0.3.2 | Zlib OR Apache-2.0 OR MIT | iOS | ADOPT AT PHASE 4 | `BGTaskScheduler`, `BGAppRefreshTaskRequest`, `BGProcessingTaskRequest` all bound. Caveat: registration must run before `didFinishLaunching` returns, which is SwiftUI's `@main`; calling it from Swift and handing Rust the callback is simpler than driving the whole lifecycle from Rust. |
| objc2-user-notifications | 0.3.2 | Zlib OR Apache-2.0 OR MIT | Apple | REJECT | Notifications belong to the SwiftUI surface, not the core. |
| swift-bridge | 0.1.59 | MIT OR Apache-2.0 | Apple | REJECT | BoltFFI owns every foreign binding (ADR 0007); a second generator is the regression the roadmap names. |
| jni | 0.22.4 | MIT OR Apache-2.0 | Android | ADOPT AT PHASE 1 | Foreground-service start/stop and any Java callback from `runtimes/android`; same local `unsafe_code` allow. |
| ndk-context | 0.1.1 (2022) | MIT OR Apache-2.0 | Android | ADOPT AT PHASE 1 | Tiny, stable: hands the loaded `.so` the `JavaVM` and `Context` the Kotlin side registered. Unmaintained-looking because it is finished. |
| ndk | 0.9.0 (2024) | MIT OR Apache-2.0 | Android | REJECT | Asset manager, native window, input — all owned by Compose here. |
| android-activity | 0.6.1 | MIT OR Apache-2.0 | Android | REJECT | Assumes Rust owns the Activity and the event loop; Compose owns both. |
| android_logger | 0.15.1 | MIT OR Apache-2.0 | Android | ADOPT AT PHASE 1 | `tracing` → logcat so the foreground service is debuggable. |
| windows | 0.62.2 | MIT OR Apache-2.0 | Windows | ADOPT AT PHASE 4 | Microsoft-maintained; toast, jump list, shell integration for the WinUI 3 surface's Rust side. |
| windows-service | 0.8.1 | MIT OR Apache-2.0 | Windows | ADOPT AT PHASE 4 | SCM dispatcher + control handler; turns `arutd` into a real service without hand-written `winapi`. |
| ratatui | 0.30.2 | MIT | terminal | ADOPT AT PHASE 4 | The terminal surface's alternate-screen mode. Rust-owned, reads projections directly like the GTK surface. |
| crossterm | 0.29.0 | MIT | terminal | ADOPT AT PHASE 4 | ratatui's default backend; also gives the inline CLI raw-mode and key events. |
| iced | 0.14.0 | MIT | any | REJECT | A Rust-owned look on every platform is the opposite of PRD principle 1; it draws its own widgets rather than the host's. |
| slint | 1.17.1 | GPL-3.0-only OR royalty-free OR commercial | any | REJECT | License: GPL is rejected outright and the royalty-free terms are a per-product grant, not an open license. |
| dioxus / dioxus-native | 0.7.10 | MIT OR Apache-2.0 | any | REJECT | Blitz is an alpha HTML/CSS+wgpu renderer; the web surface is already React, so this buys nothing native. |
| libcosmic | not on crates.io | MPL-2.0 | Linux | REJECT | git-only, iced-based, and COSMIC-styled — one desktop's look, the same failure mode as libadwaita. |
| egui | 0.36.2 | MIT OR Apache-2.0 | any | REJECT | Immediate mode, no platform accessibility or IME parity; fine for a debug inspector, not a surface. |
| accesskit | 0.25.0 | MIT OR Apache-2.0 | any | REJECT | Only needed by toolkits that draw their own widgets; GTK4 already exposes AT-SPI. |
| opener 0.8.5 / arboard 3.6.1 | — | MIT OR Apache-2.0 | desktop | REJECT for GTK | `gtk::UriLauncher`, `gtk::FileLauncher` and `gdk::Clipboard` cover both; reconsider only for the terminal surface. |
| WaterUI `backends/gtk` | git, water-rs/waterui | MIT | Linux | BORROW PATTERN | See notes 4-6. |

Notes

1. Theme: do nothing. Plain GTK4 widgets already follow the running desktop's `gtk-theme-name`. Never set `GTK_THEME`, never ship a full stylesheet; add one small `GtkCssProvider` at `STYLE_PROVIDER_PRIORITY_APPLICATION` for arut-specific classes only, and express every color in it as a named theme color, not a literal.
2. Color scheme: build gtk4 with `v4_20` and read `Settings::default().gtk_interface_color_scheme()` (`InterfaceColorScheme::{Unsupported,Default,Dark,Light}`, non-exhaustive), watching `notify::gtk-interface-color-scheme`. GTK 4.20 (2025-08-29) queries the portal itself, so no ashpd is needed for dark mode on a current GTK; keep `ashpd::desktop::settings::Settings::color_scheme()` + `receive_color_scheme_changed()` as the pre-4.20 fallback behind a cfg.
3. Accent: `gtk-accent-color` only appears, unstable, in GTK 4.24 — so read it from the portal now: `ashpd` `settings::Settings::read(APPEARANCE_NAMESPACE, ACCENT_COLOR_SCHEME_KEY)` → `ashpd::desktop::Color` → `gdk4::RGBA` via ashpd's `gtk4` feature, subscribed through `receive_setting_changed()`. Out-of-range RGB means "unset": fall back to the theme's own `theme_selected_bg_color`.
4. BORROW from WaterUI `backends/gtk/src/theme.rs`: resolve the palette from the theme's named colors — `theme_bg_color`, `theme_base_color`, `theme_unfocused_bg_color`, `theme_fg_color`, `theme_unfocused_fg_color`, `theme_selected_bg_color`, `theme_selected_fg_color`, `borders` — through `StyleContext::lookup_color`, and re-read the whole palette on `notify::gtk-theme-name` and on the color-scheme notify. GTK 4.10 deprecated `GtkStyleContext` with no replacement for named-color lookup, so scope one `#[expect(deprecated, reason = ...)]` to that single `lookup` function and nothing else.
5. BORROW from WaterUI `backends/gtk/src/app.rs::install_app_icon`: `gtk::Window::set_default_icon_name(app_id)` plus `IconTheme::for_display(&display).add_search_path(<packaged hicolor tree>)`, and use symbolic icon names (`gtk::Image::from_icon_name`) everywhere so the desktop's icon theme wins. Missing tree simply falls back to the generic window icon.
6. BORROW from WaterUI `backends/gtk/src/lib.rs`: `#[cfg(target_os = "linux")]` over the whole module tree so the surface crate still compiles to an empty crate on macOS/Windows CI, which is what ARCHITECTURE's "stay compiling where this machine can compile them" needs.
7. Also read `gtk_interface_contrast()` (v4_20) for high-contrast, and ashpd's `ReducedMotion` before animating anything.
8. Autostart is the `background` portal (`ashpd::desktop::background`, `autostart(true)` + command), not a hand-written `.desktop` file — the portal is the only path that works both packaged and unpackaged.
9. No Rust-owned Windows or macOS surface. Alternatives recorded above (iced, slint, dioxus-native, libcosmic, egui) all draw their own widgets, which loses the platform's accessibility, IME, text selection, menu and lifecycle behavior — the exact bar PRD principle 1 sets. The terminal is the one extra Rust-owned surface because a terminal has no native toolkit to lose.
10. ADR 0021 holds: relm4 consumes `Watch` directly on GLib's main context; nothing here adds a reactive framework.
11. `unsafe_code = "deny"` is a workspace lint; `objc2`, `block2`, `jni`, `ndk-context` and `windows` all need a crate-local `#![allow(unsafe_code)]` in `runtimes/{apple,android,windows}` — worth an explicit line in each crate's `//!` docs.
12. Watch items: `ndk-context` (last release 2022) and `single-instance` (2021) are stale; `single-instance` is unnecessary anyway since `gtk::Application` gives single-instance for free through GApplication.

## Models, harness, tools, agent protocols

Verified 2026-09-13 against the crates.io API (version, license, last publish), docs.rs feature
pages, and repo activity. `wasm-no-bindgen` = compiles for `wasm32-unknown-unknown` without
wasm-bindgen, i.e. usable inside the BoltFFI core (ARCHITECTURE.md "Constraints worth knowing").

| Crate | Version | License | wasm-no-bindgen | Verdict | Why |
|---|---|---|---|---|---|
| rig-core | 0.42.0 (2026-08-17) | MIT | no (reqwest, tokio) | ADOPT NOW | ADR 0014 holds. 23 in-tree providers incl. OpenAI-compatible base-URL override, `streaming` module, `tool::PortableTool` + `#[rig_tool]`, already depends on `schemars`. 8.6k stars, pushed daily. Its `wasm_compat`/`if_wasm` path is browser wasm-bindgen, so still node-side only. |
| rig-agent | 0.42.0 | MIT | no | ADOPT AT PHASE 2 | Split-out run loop; has an **optional `rmcp` dependency**, so the MCP-tools-into-agent-loop bridge is upstream, not ours to write. |
| rig-derive | 0.42.0 | MIT | no | ADOPT NOW | Pulled by rig-core's `derive` feature; no separate decision. |
| genai | 0.6.5 (0.7 beta) | MIT OR Apache-2.0 | no (reqwest) | REJECT | Cleaner multi-provider API than rig but no tool/agent layer and 8x fewer users; nothing rig lacks. |
| async-openai | 0.42.0 (2026-09-09) | MIT | no (reqwest) | BORROW PATTERN | Copy its typed OpenAI request/response structs + `eventsource-stream` SSE decode loop if rig's `CompletionModel` abstraction fights `ModelProvider::respond`. Keep as the drop-in fallback provider crate. |
| async-openai-wasm | 0.31.2 (2025-12) | MIT | no (wasm-bindgen fetch) | REJECT | Its whole point is browser fetch via wasm-bindgen — the one thing our wasm core cannot use. |
| llm | 1.3.8 (2026-04) | MIT | no | REJECT | Single-maintainer rig overlap, 119k downloads, no release in 5 months. |
| langchain-rust / llm-chain / swiftide / kalosm / autoagents / anda | — | MIT / Apache | no | REJECT | Chain-and-pipeline frameworks that own the control flow. Arut's harness owns it. |
| **agent-client-protocol** | 2.1.0 (2026-09-04) | Apache-2.0 | no (async-io, async-process, rustix) | ADOPT AT PHASE 2 | The protocol for Phase 2 item 3. JSON-RPC 2.0 over stdio; sessions, streamed updates, `session/request_permission`, client-side file access. Adopted by Zed 1.0, JetBrains, Google, GitHub; public agent registry since 2026-01. 4.3M downloads. Runtime lives in a native runtime crate only. |
| agent-client-protocol-schema | 1.7.0 | Apache-2.0 | **yes** (serde only) | ADOPT AT PHASE 2 | The wire types with no I/O: `anyhow, derive_more, serde, serde_json, serde_with, strum`. This is what the core and the protobuf mapping depend on. 4.6M downloads. |
| a2a-rs | 0.9.2 | MIT | no | REJECT | A2A is agent-to-agent; our axis is client-to-agent. 11k downloads. |
| a2a-sdk | 0.7.0 (2025-08) | MIT | no | REJECT | Stale, internal to agentgateway. |
| **rmcp** (official MCP SDK) | 3.3.0 (2026-09-10) | Apache-2.0 | core yes, transports no | ADOPT AT PHASE 2 | Required deps are serde/tokio/tracing only; every transport is opt-in: `transport-async-rw` (default, generic AsyncRead/AsyncWrite — this is the one that rides an iroh stream or `RpcChannel`), `transport-io` (stdio), `transport-child-process`, `transport-streamable-http-client/-server`, `transport-worker`. 26M downloads, pushed daily. |
| rust-mcp-sdk | 2.0.0 | MIT | no | REJECT | Third-party alternative; official SDK is healthier and better adopted. |
| tower-mcp | 0.22.2 | MIT OR Apache-2.0 | no | BORROW PATTERN | Copy the one-tool-is-one-`tower::Service` layering if we want per-tool timeout/retry/approval middleware over rmcp. |
| mcp-core / mcp-sdk / mcp-attr | — | Apache / MIT | no | REJECT | All unmaintained since 2025. |
| schemars | 1.2.2 | MIT | **yes** | ADOPT NOW | rig-core and rmcp both already require it — pin one 1.x in `[workspace.dependencies]` now or get two copies. Tool schemas and ACP schemas both derive from it. |
| jsonschema | 0.56.0 (2026-09-10) | MIT | partial (needs a `getrandom` backend) | ADOPT AT PHASE 2 | Validate tool arguments at the harness boundary before a fact is written. Node-side only. |
| garde / valico | 0.23 / 4.0 | MIT/Apache | — | REJECT | Struct-level and stale respectively; JSON Schema is the tool contract. |
| minijinja | 2.24.0 | Apache-2.0 | **yes** (deps: `memo-map`, `serde`) | ADOPT AT PHASE 2 | Prompt and system-message templating that also compiles into the wasm core. Smallest dep tree of any Rust template engine. |
| tera | 2.4.0 | MIT | no (globwalk, chrono) | REJECT | Filesystem globbing is a required dep; heavier for no gain. |
| tiktoken-rs | 0.12.0 | MIT | **yes** (pure Rust) | ADOPT AT PHASE 2 | Context-window budgeting for the OpenAI-compatible provider. |
| bpe-openai | 0.3.1 | MIT | **yes** | ADOPT AT PHASE 2 | Prefer over tiktoken-rs: github/rust-gems, linear-time and **incremental**, so a streaming transcript is re-counted per token instead of per turn. |
| tokenizers (HF) | 0.23.2 | Apache-2.0 | no (esaxx-rs C++, rayon) | REJECT | Training-grade; we only count. |
| ignore | 0.4.33 | Unlicense OR MIT | no (fs) | ADOPT AT PHASE 2 | `.gitignore`-correct parallel walk for any file-listing tool. Ripgrep-proven, 171M downloads. |
| grep-searcher (+ grep-regex) | 0.1.17 | Unlicense OR MIT | no (fs) | ADOPT AT PHASE 2 | Line/multiline search with the same semantics users expect from ripgrep. |
| tree-sitter | 0.27.0 | MIT | no (C core) | ADOPT AT PHASE 2+ | Only when the first coding harness lands; PRD says coding is one harness, not the product. |
| ast-grep-core | 0.45.3 | MIT | no | BORROW PATTERN | Copy its pattern-as-code structural matching over tree-sitter rather than hand-rolling queries. |
| wasmtime | 48.0.2 | Apache-2.0 WITH LLVM-exception | no | ADOPT AT PHASE 2+ | The in-process sandbox for untrusted tools on desktop and cloud nodes. Component model + WASI capability model; note 2026-04 component-model CVEs — pin and track advisories. |
| wasmi | 2.0.0 | MIT OR Apache-2.0 | **yes** (no_std, pure Rust) | ADOPT AT PHASE 2+ | The only wasm sandbox that runs *inside* the wasm core, so a browser surface can execute a pure tool locally. Slower; correct fallback tier. |
| extism | 1.30.0 | BSD-3-Clause | no (wraps wasmtime) | BORROW PATTERN | Copy the plugin manifest + host-function ABI shape; do not take the layer — it hides the wasmtime knobs we need for fuel and memory limits. |
| wasmer | 7.4.0 | MIT | no | REJECT | Wasmtime is the Bytecode Alliance line and already our WASI story. |
| cap-std | 4.0.3 | Apache-2.0-LLVM OR Apache-2.0 OR MIT | no | ADOPT AT PHASE 2+ | Capability-oriented `Dir` handles: a tool gets a directory, not a path, which makes workspace confinement a type. |
| landlock | 0.4.7 | MIT OR Apache-2.0 | no (Linux) | ADOPT AT PHASE 2+ | Unprivileged self-confinement of a spawned tool process. 15M downloads, maintained by landlock-lsm. |
| seccompiler | 0.5.0 | Apache-2.0 OR BSD-3-Clause | no (Linux) | ADOPT AT PHASE 2+ | rust-vmm syscall filter; the second layer under landlock. |
| process-wrap | 10.0.0 | Apache-2.0 OR MIT | no | ADOPT AT PHASE 2 | Process groups, job objects, reliable kill-on-drop for the ACP child process and stdio MCP servers. rmcp already depends on it — free. |
| portable-pty | 0.9.0 | MIT | no | ADOPT AT PHASE 2+ | Only if a harness needs a real terminal tool; wezterm-maintained. |
| evalbox-sandbox / sandlock | 0.2.0 / 0.1.0 | MIT(/Apache) | no | BORROW PATTERN | Too new to depend on (194 and 26 downloads). Copy the recipe: Landlock v5 rules + ~40-syscall seccomp allowlist + `NO_NEW_PRIVS` + rlimits + capability drop. |
| birdcage | 0.8.1 | **GPL-3.0-or-later** | no | REJECT | License rule. |
| bubblewrap (crate) | 0.0.1 | MIT | no | REJECT | 33 downloads, not a real binding. Shell out to the `bwrap` binary if we ever want it. |
| firecracker (crate) | 0.2.3 | MIT OR Apache-2.0 | no | REJECT | Not the AWS project; real Firecracker is a VMM binary driven over a REST socket, not a library. |
| microsandbox | 0.6.18 | Apache-2.0 | no | REJECT | A microVM daemon, not a library. Revisit only as cloud-node infrastructure in Phase 3. |
| iii (iii-hq/iii) | — | **Elastic License 2.0 (FLAGGED)** | n/a | ADOPT AT PHASE 3 | 18.7k stars, active. Roadmap's spike criteria stand. ELv2 forbids offering it as a managed service — fine beside a cloud node, a lawyer question for on-prem packaging in Phase 6. Never in the node core. |

### Notes

1. Harness port, mirroring `ModelProvider` — one stream, no I/O in the trait: `fn start(&self, req: OperationRequest) -> BoxStream<'_, Result<HarnessEvent, HarnessError>>`, plus `fn resolve(&self, ApprovalId, Decision) -> Result<(), HarnessError>` and `fn cancel(&self, OperationId)`.
2. `HarnessEvent` is a closed enum whose every variant is already a fact: `Token`, `ToolCallStarted{id,name,args}`, `ToolCallDelta`, `ToolCallEnded{outcome}`, `ApprovalRequested{id,request}`, `Ended{outcome}`. `HarnessError` is typed (ADR 0016).
3. Approval is an in-band *event* plus an out-of-band `resolve` call, never a callback: that is what lets any surface answer first and gives a late answer a typed stale result (Phase 2 item 2). A callback binds the answer to one surface.
4. `ModelProvider` becomes a sub-port the harness consumes, not a peer of it; `Harness` joins `ChatRuntime`'s capability bundle so a runtime without it cannot construct the feature, and the mock harness is one impl of the same trait.
5. **The first external harness adapter should speak ACP** (Agent Client Protocol), JSON-RPC 2.0 over child-process stdio. Arut's node is the ACP *client*; the external harness is the ACP *agent*.
6. ACP over A2A because A2A is the wrong axis (agent-to-agent); ACP over inventing one because `session/update` and `session/request_permission` map one-to-one onto notes 1-3, and 25+ agents plus Zed, JetBrains, Google and GitHub already speak it.
7. Split the ACP dependency: `agent-client-protocol-schema` (serde-only, wasm-clean) in the protocol and feature crates; `agent-client-protocol` (async-io, async-process, rustix) only in `runtimes/local`, behind the external-process adapter.
8. MCP and ACP are complementary, not competing: ACP carries the conversation between node and harness, MCP carries tools into the harness. `rig-agent`'s optional `rmcp` feature already wires MCP tools into a rig agent loop, so that bridge is upstream.
9. Take `rmcp` with `default-features = false` and only `transport-async-rw`, so an MCP server can ride an existing `RpcChannel` or iroh stream; add `transport-child-process` for local stdio servers; never pull its reqwest transports into the core.
10. Pin `schemars` 1.x in `[workspace.dependencies]` before Phase 2 starts — rig-core, rmcp, agent-client-protocol and tower-mcp all require it, and two majors in the tree means two incompatible `JsonSchema` traits.
11. Sandboxing is tiered and chosen by node, not by tool: wasmtime in-process on desktop and cloud, wasmi inside the wasm core, landlock + seccompiler + cap-std around spawned native tools. None of it is needed for the Phase 2 mock slice.
12. Only these belong in the wasm core: `agent-client-protocol-schema`, `schemars`, `minijinja`, `bpe-openai` or `tiktoken-rs`, and `rmcp`'s type layer. Everything else here is a node-side runtime dependency, which ADR 0014 already anticipated for `rig`.
13. rig 0.42 split `rig-agent` (run loop) out of `rig-core` (providers, streaming, `PortableTool`); ADR 0014 predates the split, so the `ModelProvider` impl should depend on `rig-core` alone and leave `rig-agent` to the harness crate.
14. Phase 1 item 5 needs only `rig-core` + the OpenAI-compatible base-URL override; if rig's `CompletionModel` fights the port, `async-openai` 0.42 plus `eventsource-stream` is a two-day drop-in with the same shape.
15. Licenses: everything ADOPTed is MIT, Apache-2.0, BSD-3, Unlicense-or-MIT, or Apache-with-LLVM-exception. Rejected on licence: `birdcage` (GPL-3.0-or-later). Flagged: iii (Elastic 2.0) — fine beside a cloud node, a question for Phase 6 on-prem packaging.

## Observability, config, testing, tooling

| Crate | Version | License | wasm-no-bindgen | Verdict | Why |
| --- | --- | --- | --- | --- | --- |
| tracing | 0.1.44 (2025-12) | MIT | Yes | ADOPT NOW | Only deps `pin-project-lite`+`tracing-core`; zero-cost when unsubscribed; matches settled choice. |
| tracing-subscriber | 0.3.23 (2026-03) | MIT | Yes | ADOPT NOW | `Registry`+`fmt`/`EnvFilter` layers compose on every target incl. wasm. |
| tracing-opentelemetry | 0.33.0 (2026-05) | MIT | No | ADOPT AT PHASE 2 | Unconditionally pulls `js-sys` on wasm32 (needs wasm-bindgen glue); cfg-gate out of the wasm core; wire when the OTel pipeline (Phase 2, iii tracing) exists. |
| opentelemetry + opentelemetry-sdk | 0.32.x (2026-05) | Apache-2.0 | No | ADOPT AT PHASE 2 | Same `js-sys` leak transitively; fine natively, off by default per architecture. |
| opentelemetry-otlp | 0.32.0 (2026-05) | Apache-2.0 | No | ADOPT AT PHASE 2 | Every transport (reqwest/hyper/tonic) needs Tokio; native-host exporter only, never in wasm build. |
| console-subscriber (tokio-console) | 0.5.0 (2025-10) | MIT | No | ADOPT AT PHASE 1 | Opt-in dev feature for the `local` Tokio runtime once Spawner/Host land; never shipped, never CI. |
| open-feature | 0.3.0 (2026-03) | Apache-2.0 | Yes | ADOPT NOW | Spec-conformant CNCF SDK; only non-optional dep is `tokio` with `sync` feature — wasm-clean core. |
| local/file provider | n/a | n/a | Yes | BORROW PATTERN | No official in-memory/local provider crate exists; hand-roll a `FeatureProvider` reading the figment-resolved config, matching "local file provider first." |
| open-feature-flagd | 0.2.2 (2026-07) | Apache-2.0 | No | ADOPT AT PHASE n | Default features pull tonic+reqwest; use minimal in-process resolver, native hosts only, when remote rollout is needed. |
| open-feature-ofrep | 0.1.2 (2026-07) | Apache-2.0 | No | REJECT for now | Thin REST client, hard `reqwest` dep, no need until a flag backend is chosen. |
| figment | 0.10.19 (2024-05) | MIT/Apache-2.0 | Yes | ADOPT AT PHASE 1 | Matches the global→node→workspace→conversation chain exactly (`Env`,`Toml`,`Json`,`Serialized`); stale but small and complete, not abandoned. |
| config | 0.15.25 (2026-06) | MIT/Apache-2.0 | Yes | REJECT | Actively developed but heavier dep surface (nom/ron/json5); figment already covers the merge-chain need. |
| confique | 0.4.0 (2025-10) | MIT/Apache-2.0 | Yes | REJECT | Less ecosystem-proven; no advantage over figment for this shape of chain. |
| keyring | 4.2.0 (2026-08) | MIT/Apache-2.0 | No | ADOPT NOW | Desktop-only secret-service/Keychain/Credential-Manager abstraction; matches settled choice; no wasm backend exists or can exist. |
| apple-/android-native-keyring-store | 1.0.x (2025-08) | MIT/Apache-2.0 | No | BORROW PATTERN | Real iOS Keychain / Android Keystore backends now exist under `keyring-core`, but are ~1yr old; copy the `keyring-core` trait shape, wire mobile secrets through the platform runtime's native FFI directly rather than depending on these young crates yet. |
| secrecy | 0.10.3 (2024-10) | Apache-2.0/MIT | Yes | ADOPT NOW | `Secret<T>`/`ExposeSecret`, no unsafe, depends only on zeroize; wrap provider keys and root-key material in memory everywhere incl. wasm. |
| zeroize | 1.9.0 (2026-06) | Apache-2.0/MIT | Yes | ADOPT NOW | Foundational RustCrypto crate, explicit wasm support, no reason not to use it under secrecy. |
| thiserror | 2.0.20 (2026-08) | MIT/Apache-2.0 | Yes | ADOPT NOW | See ADR 0016 note below. |
| snafu | 0.9.2 (2026-07) | MIT/Apache-2.0 | Yes (no_std) | REJECT | Idiomatic use pushes narrative `.context()` strings at construction sites, fighting "no strings from core." |
| miette | 7.6.0 (2025-04) | Apache-2.0 | n/a (dev-only) | ADOPT NOW, scoped to tools/ | Diagnostic pretty-printer, not a base error type; confine to `tools/conformance` and any xtask/CLI, never `features/`or `substrates/`. |
| proptest | 1.11.0 (2026-03) | MIT/Apache-2.0 | Partial (needs getrandom `wasm_js`) | ADOPT NOW | Dev-dependency only, so the wasm caveat doesn't matter; use for Machine/Authority input-space tests. |
| quickcheck | 1.1.0 (2026-02) | Unlicense/MIT | Unverified | REJECT | Materially staler maintenance and thinner shrinking than proptest. |
| insta | 1.48.0 (2026-06) | Apache-2.0 | n/a (dev-only) | ADOPT NOW | Snapshot projections and protobuf golden outputs; irrelevant to ADR 0016 (test artifacts, not shipped strings). |
| loom | 0.7.2 (2024-04) | MIT | No (dev-only, cfg-gated) | ADOPT AT PHASE 1 | Model-check `substrates/watch` and `substrates/authority` dedup/apply interleavings, same technique tokio uses on its own `watch`; requires `cfg(loom)` shims, can't drive a real multi-thread Tokio runtime, bound the state space. |
| criterion | 0.8.2 (2026-02) | Apache-2.0/MIT | n/a (dev-only) | ADOPT AT PHASE 1 | Mature default once compaction/log/stream perf work starts; no wasm relevance either way. |
| divan | 0.1.21 (2025-04) | MIT/Apache-2.0 | n/a (dev-only) | REJECT for now | Redundant with criterion; slower release cadence; revisit only if criterion compile time becomes a real pain point. |
| cargo-nextest | 0.9.144 (2026-09) | MIT/Apache-2.0 | n/a | ADOPT NOW | Drop-in `cargo test` replacement, faster, deterministic, no network after install. |
| cargo-machete | 0.9.2 (2026-04) | MIT | n/a | ADOPT NOW | Sub-second unused-dep scan, no compile needed. |
| cargo-shear | 1.13.4 (2026-08) | MIT | n/a | BORROW PATTERN | Newer machete alternative (oxc parser); trial later, don't run both. |
| cargo-deny | 0.20.2 (2026-07) | MIT/Apache-2.0 | n/a | ADOPT NOW, CI-only | License/advisory/duplicate gate; directly enforces ADR 0017's split; needs network + a maintained `deny.toml`, so PR-gate not local. |
| cargo-semver-checks | 0.50.0 (2026-08) | Apache-2.0/MIT | n/a | ADOPT AT PHASE 1, CI-only | Scope to `protocols`, `protocols/rpc`, substrates that are MIT/Apache-2.0 and meant to be reused per ADR 0017; needs a baseline build. |
| cargo-udeps | 0.1.61 (2026-04) | MIT/Apache-2.0 | n/a | ADOPT AT PHASE n, CI-only | Nightly + full rebuild; scheduled job to catch what machete/shear miss. |
| cargo-hack | 0.6.45 (2026-05) | Apache-2.0/MIT | n/a | ADOPT AT PHASE 1, CI-only | Feature-powerset builds; worth it once wasm/native feature flags multiply; combinatorial cost rules out local. |
| cargo-mutants | 27.1.0 (2026-06) | MIT | n/a | ADOPT AT PHASE 2, CI-only (scheduled) | Cost ≈ test-suite time × mutant count; run weekly once the conformance/Machine suites are the thing being graded, not per-PR. |
| bacon | 3.25.0 (2026-08) | AGPL-3.0 | n/a | REJECT (repo tooling) | AGPL; also an interactive watcher, not a scripted check — fine as a developer's personal install, never wired into `mise.toml` or CI. |

Notes (≤15 lines):
- `mise run check` (fast, local, no network): add `cargo nextest run` in place of `cargo test`, and `cargo machete`. Both are sub-second-to-seconds and deterministic.
- CI-only jobs, not local: `cargo deny check` (PR gate, needs advisory-db + `deny.toml`), `cargo semver-checks` (PR gate, scoped to `protocols`/`protocols/rpc`), `cargo hack --feature-powerset` and `cargo udeps` (nightly/scheduled), `cargo mutants` (weekly scheduled). None belong in the commit-time loop.
- ADR 0016 error-type approach: use `thiserror` 2.x on every core error/outcome enum. `#[error("...")]` only backs `Display` for logs/telemetry; the enum's typed variants and payload fields are the contract surfaces read, so no user-facing string ever needs to exist in the attribute. Reject `snafu` — its `.context()`/`whatever!` idiom encourages narrative strings at construction time, working against the ADR's discipline. Keep `miette` strictly in `tools/` for pretty CLI diagnostics (conformance runner output), never on a `features/` or `substrates/` error type.
- OTel crates (`opentelemetry*`, `tracing-opentelemetry`) leak a `js-sys` dependency on any `wasm32` target even with `wasm-bindgen` absent from your own code — cfg them out of the wasm build explicitly rather than relying on "off by default" alone.
- `keyring` stays desktop-only (Linux secret-service, macOS/Windows). For iOS/Android, don't add `keyring`'s new native-store backends yet (both ~1 year old); go through the platform runtime's own Keychain/Keystore FFI as the architecture already implies, wrapping values in `secrecy::Secret` uniformly across all hosts including wasm.
- `loom` and `proptest` are dev-dependencies only, so their partial/no wasm support is irrelevant; both should live in `substrates/watch`, `substrates/authority`, and `tools/conformance` test targets.
