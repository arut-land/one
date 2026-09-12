---
status: accepted
---

# One active route with health-scored failover in the first release

Many routes are planned (local network, backend relay, WebRTC, Bluetooth) and eventually different features may prefer different routes. We decided that the first release keeps one active route per node with a basic health score and an ordered failover list, and that the local-network route via mDNS discovery ships before WebRTC because it needs no backend. Because facts carry sequence numbers and commands carry IDs, moving later to parallel probing and to per-feature multipath is a change to route policy only, not to any protocol.

## Consequences

- Route changes preserve command identity, cursors, and epochs by construction.
- WebRTC waits for the relay it needs for signaling; Bluetooth waits for a watch surface that needs it.
