---
status: accepted
---

# Drafts replicate as ephemeral state with last-writer-wins and are not facts

Drafts must follow a person between devices, but the first slice made them the only durable state and rewrote the whole store on every keystroke. We decided that a draft is replicated ephemeral state scoped to a conversation (or to the pending scope before one exists), resolved by last-writer-wins on revision, and shown with an "editing on another device" indicator. Drafts are not appended to the fact log; the transcript is. A draft survives an app restart through the key-value store only on the device that typed it.

## Considered options

- **Character-level merge.** Rejected: the only case for a CRDT here, and messaging products do fine without it.
- **Durable draft facts.** Rejected: a draft is not a milestone and its history has no audit value.
