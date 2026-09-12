# Conformance

Reusable suites exercise unary and server-stream RPC ordering, metadata, typed
errors and stream termination; fact-log sequencing, fencing, retries, snapshots,
and compaction; blob content addressing; and independent key-value updates.

The same RPC suite runs against memory, TCP Connect, and Unix IPC. Storage suites
run against memory and directory implementations. Extra tests cover independent
writers, reopening compacted logs, and fragmented/malformed Connect envelopes.
