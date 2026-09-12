# 0002: Conversation authority and execution binding

## Status

Accepted as target architecture. Chat-scoped Composer implements local authority fencing, independent drafts, and durable commands; migration and failover remain target work.

## Context

One user may view and control conversations from Android, iOS, desktop, web, or integrations while execution runs on a Mac, Windows machine, managed cloud worker, or another node. Changing the network route must not accidentally change where execution occurs. Retries and concurrent approvals must not execute mutations twice.

## Decision

The conversation session is the authority unit. A workspace may host many conversations and share machine-bound resources, but each conversation has one active authority that orders durable facts and accepts mutations.

Every mutating command carries a stable command ID. State-sensitive commands also carry typed preconditions such as operation ID, expected revision, and authority epoch. The first valid decision wins; later decisions return a typed stale or already-resolved outcome.

Execution location is bound to the conversation. Transport and route selection only determine how a client reaches that authority.

Migration transfers authority through a monotonic epoch and fencing protocol. Automatic failover requires a coordinator that can prevent two nodes from committing concurrently. Cloud-backed conversations may use the backend log for this coordination. Local-only sessions without a coordinator cannot safely fail over during a partition.

Surfaces publish native lifecycle facts. A shared session coordinator owns checkpoint and failover policy.

## Consequences

- Surfaces can reconnect or change routes without changing execution identity.
- At-least-once delivery is safe because mutations are idempotent at the authority.
- Offline commands can be queued only according to their typed policy and preconditions.
- Migration must distinguish portable product state from machine-bound workspace state.
- Authority epoch and command identity become protocol-level concepts.
