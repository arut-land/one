# Storage

FactLog appends immutable, sequenced command outcomes and supports cursor reads,
snapshots, compaction, deduplication, and epoch fencing. DirectoryLog locks across
processes, writes one JSON record per append, fsyncs the file, atomically renames
it, and fsyncs the directory. Compaction requires a snapshot and retains outcomes
for retry deduplication. It never rewrites all state on append.

BlobStore stores SHA-256 addressed raw bytes and verifies them on read. KeyValue
stores each small value independently. Directory keys are hashed to prevent path
traversal. Memory implementations provide the same ports for in-process sessions.
