# 0004: Authoritative log, replication, and streams

## Status

Accepted and partially implemented. Chat-scoped Composer is the first typed command/fact subsystem; the reusable log, replication, compaction, and resumable-stream substrates remain pending.

## Context

Conversation state must appear consistently across independently connected surfaces. Durable messages and approvals need ordering and recovery, while model tokens, terminal bytes, progress, and file-watch bursts are too frequent to store as ordinary product state.

## Decision

The conversation authority appends typed durable product facts to an ordered log. Feature reducers build bounded projections from those facts. Periodic snapshots accelerate startup and permit log compaction.

A generic log substrate provides sequencing, cursors, acknowledgements, command deduplication, snapshot installation, encryption envelopes, and route-independent replication. Feature code continues to own validation, event meaning, reducers, and conflict policy.

High-rate transient data uses resumable streams with stable stream IDs and monotonic offsets. Clients resume from acknowledged offsets while retention permits. If continuity cannot be restored, clients install a current snapshot or final durable result.

Direct peer-to-peer, relay, and store-and-forward routes preserve the same replication and stream semantics. A synchronization server may store an encrypted replica without becoming the authority for a local conversation.

## Consequences

- New surfaces can reconstruct the same product projection.
- The durable log remains compact enough for replay and audit.
- Reconnection does not depend on a specific transport such as SSE or WebSocket.
- CRDTs are not required for authority-owned state.
- Retention, compaction, snapshot compatibility, and backpressure require explicit policy.
