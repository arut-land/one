---
status: accepted
---

# Commands are signed envelopes that any paired device may carry

A person may send a follow-up to a conversation whose authority is unreachable while another of their devices is nearby. We decided that a command is a signed, encrypted envelope addressed to a conversation's authority, and that any paired device or the backend may hold it and forward it when the authority returns. Every command carries a stable command ID and typed preconditions, so duplicate delivery through several carriers is safe and a late arrival returns a typed superseded result instead of executing stale.

## Consequences

- At-least-once delivery everywhere, exactly-once effect at the authority.
- The relay and a nearby laptop are the same mechanism; there is no special offline queue.
- Draft edits are replicated as ephemeral state, not as carried commands, and use last-writer-wins by revision.
