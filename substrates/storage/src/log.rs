//! The one copy of the fact-log commit ordering, over a per-backend store.

use crate::{CommitDecision, Fact, Record, Result, Snapshot, StorageError};

/// Exclusive access to one log for the length of one commit: a mutex guard in
/// memory, a write transaction in redb.
pub(crate) trait LogStore<F: Fact> {
    /// The newest sequence and the epoch it was written under.
    fn head(&mut self) -> Result<(u64, u64)>;
    /// The sequence compaction has retired.
    fn watermark(&mut self) -> Result<u64>;
    fn duplicate(&mut self, command_id: &str) -> Result<Option<Record<F>>>;
    /// Records strictly after `cursor`.
    fn after(&mut self, cursor: u64) -> Result<Vec<Record<F>>>;
    fn push(&mut self, record: &Record<F>) -> Result<()>;
    /// Makes the pushed record durable. Not called when nothing was pushed.
    fn finish(self) -> Result<()>;
}

/// Refreshes, fences, deduplicates, and appends under `store`'s exclusive access.
///
/// `decide` runs once, after fencing and cursor validation, with the current
/// sequence, the records the caller has not seen, and the retry outcome.
pub(crate) fn commit_policy<F: Fact, S: LogStore<F>>(
    mut store: S,
    cursor: Option<u64>,
    epoch: u64,
    command_id: &str,
    decide: Box<CommitDecision<'_, F>>,
) -> Result<Option<Record<F>>> {
    let through = store.watermark()?;
    if cursor.is_some_and(|cursor| cursor < through) {
        return Err(StorageError::CursorUnavailable { through });
    }
    let (sequence, current) = store.head()?;
    if epoch < current {
        return Err(StorageError::Epoch { current });
    }
    if cursor.is_some_and(|cursor| cursor > sequence) {
        return Err(StorageError::Conflict { actual: sequence });
    }
    let duplicate = store.duplicate(command_id)?;
    let unseen = match cursor {
        Some(cursor) => store.after(cursor)?,
        None => Vec::new(),
    };
    let Some(fact) = decide(sequence, &unseen, duplicate)? else {
        return Ok(None);
    };
    let record = Record {
        sequence: sequence.checked_add(1).ok_or(StorageError::Corrupt)?,
        epoch,
        command_id: command_id.into(),
        fact,
    };
    store.push(&record)?;
    store.finish()?;
    Ok(Some(record))
}

/// A snapshot may only name a sequence the log still holds.
pub(crate) fn snapshot_allowed(snapshot: &Snapshot, newest: u64, compacted: u64) -> Result<()> {
    if snapshot.sequence > newest || snapshot.sequence < compacted {
        return Err(StorageError::SnapshotRequired);
    }
    Ok(())
}

/// Compaction may only discard what a snapshot has already captured.
pub(crate) fn compaction_allowed(snapshot: Option<&Snapshot>, through: u64) -> Result<()> {
    if snapshot.is_none_or(|snapshot| snapshot.sequence < through) {
        return Err(StorageError::SnapshotRequired);
    }
    Ok(())
}
