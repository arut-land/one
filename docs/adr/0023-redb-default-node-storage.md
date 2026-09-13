---
status: accepted
amends: 0010
---

# Node persistence uses redb

ADR [0010](0010-storage-as-three-ports.md) keeps three storage ports. The native node now uses redb for fact logs and key-value data. Its write transactions atomically refresh, deduplicate, and append, and its command index avoids scanning the log for retries. Memory remains for tests and wasm. The directory implementation and storage selector are removed.

Each feature log has one database file. The primary namespace uses `node.redb`; additional namespaces use digest-named files. A node lease prevents concurrent owners. Existing redb data remains readable; directory data is not migrated automatically.

The async server retains a small `spawn_blocking` dispatch wrapper because redb transactions perform synchronous I/O. Transactions replace file-locking machinery, not executor isolation. Persistent attachment storage remains Phase 1 work behind `BlobStore`.
