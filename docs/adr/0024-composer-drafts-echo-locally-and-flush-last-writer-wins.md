---
status: accepted
amends: 0018
---

# Composer drafts echo locally and flush last-writer-wins

`ComposerClient::replace` was an `async fn` whose future carried the result of that one write, so per-keystroke writes queued behind the composer's operation lock and could not be coalesced; a surface that bound its text field to the acknowledged state watched the caret wait for the network. We decided that `replace(text)` writes `text` into `ComposerState.text` at call time, before the returned future is polled, and that the future resolves when a flush carrying that text or a later one is acknowledged. Pending writes coalesce behind one in-flight request and the latest text wins. `ChatClient::send` awaits that flush before it takes the operation lock, so a send after unflushed keystrokes commits what the person typed instead of failing on a pending-text mismatch. A surface binds its text field straight to `ComposerState.text` and keeps no queue.

## Considered options

- **Per-keystroke acknowledged futures.** Rejected: this is what existed. Each keystroke holds the lock for a round trip, coalescing is impossible by construction, and the cost lands on the typing path.
- **A write queue on each surface.** Rejected: five queues with five orderings is the duplication ADR 0007 exists to prevent, and the ordering rule belongs beside the revision check that enforces it.

## Consequences

- One hundred rapid `replace` calls issue two `replace_composer` requests, and the last acknowledged text is the last one typed.
- Awaiting `replace` is unchanged for callers, so no existing `.await` site moved.
- The echo is local state, not an accepted fact. ADR 0018 stands: the composer service still decides what it accepts by revision and epoch, and a rejected flush replaces the echoed text with the authority's value.
