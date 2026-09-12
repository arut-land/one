---
status: accepted
---

# The backend pairs, relays, and stores encrypted backups; it is never the source of truth

A hosted service is convenient for reaching devices across networks and for recovery, but a local-first product must work without it. We decided the backend does three things: pairs devices, relays envelopes it cannot read, and stores encrypted backups and checkpoints. Cloud execution is not a backend feature; it is the same daemon deployed on a server and paired like any other node. Accounts, when they arrive, vouch for device keys and hold escrowed root keys, and still never hold conversation truth.

## Consequences

- One small backend binary, deployable anywhere a process runs.
- Enterprise on-premises deployment is the same binary with a different host.
- The backend cannot be the coordinator for local-only failover; that stays explicit until a coordinator exists.
