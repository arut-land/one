# Authority

Command defines scope, facts, pure application, preconditions, expiry, queueability,
and optimism. Authority serializes acceptance, fences epochs before deduplication,
checks preconditions, appends through FactLog, and reduces the accepted fact.
Outcomes distinguish application, duplicate delivery, revision conflict, stale
authority, supersession, and rejection. Projection reducers have no I/O.

Snapshot and compaction are explicit maintenance operations. Draft replication
stays outside this durable authority, as required by ADR 0018.
