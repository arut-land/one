---
status: accepted
---

# A conversation is authoritative on the node that created it

Every conversation needs exactly one place that orders its facts and accepts its commands, and people expect a phone to work with no laptop online. We decided that the node which creates a conversation is its authority, phones included, and that authority moves only through an explicit hand-off with a new epoch. Other nodes hold replicas and queue commands. Because one node owns ordering, no CRDT is needed for any conversation state.

## Considered options

- **Desktop or cloud as the only authorities.** Rejected: it makes a phone useless alone and forces a daemon before the first conversation.
- **Multi-master with CRDTs.** Rejected: transcripts and operations have one writer at a time; CRDTs would add merge semantics nobody asked for.
