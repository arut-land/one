---
status: accepted
---

# Storage is three narrow ports; the first implementation is a directory tree

Storage must be swappable per platform and the durability semantics of a fact log differ from those of settings. We decided on three ports: `FactLog` (append, outcome-of, cursor reads, snapshot, compact), `BlobStore` (content-addressed bytes), and `KeyValue` (small unordered state). The first implementation is a directory tree whose structure carries meaning, with JSON only where a document is needed and raw bytes for blobs. SQLite or platform stores plug in later per port. Nothing rewrites whole state on every change.

## Considered options

- **One `Storage` trait.** Rejected: it hides that a log needs append-and-fsync ordering that a key-value store never promises.
- **SQLite first.** Deferred, not rejected: it is the likely native implementation once the ports are proven.
