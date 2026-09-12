# Watch

`Watch<T>` wraps one Tokio watch sender containing the value and revision.
Subscribers receive the current revision, coalesce intervening updates, and close
when the writer is dropped. Tokio enables only `sync`; callers supply polling.
