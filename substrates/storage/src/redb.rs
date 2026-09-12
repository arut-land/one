//! Fact logs and key-value data in one redb file with typed tables.
//!
//! Protobuf records have sequence and command indexes; compaction retains outcomes
//! for deduplication. Each command decision runs inside one write transaction.
//! The database owns its file exclusively, so other processes use the daemon's
//! RPC channel. The schema version controls migrations when opening a file.

use crate::{
    Fact, FactLog, KeyValue, Record, Result, Snapshot, StorageError, StoredRecord, StoredSnapshot,
};
use prost::Message;
use redb::{
    Database, Error as RedbError, ReadableDatabase, ReadableTable, TableDefinition,
    WriteTransaction,
};
use std::{io::ErrorKind, marker::PhantomData, path::Path, sync::Arc};

/// The transcript itself: sequence to the Protobuf row of that fact.
const RECORDS: TableDefinition<u64, &[u8]> = TableDefinition::new("records");
/// The deduplication index over live records: command ID to its sequence.
const COMMANDS: TableDefinition<&str, u64> = TableDefinition::new("commands");
/// Rows kept past compaction so a retry still learns what its command did.
const OUTCOMES: TableDefinition<&str, &[u8]> = TableDefinition::new("outcomes");
/// One row, under [`SNAPSHOT_KEY`]: the projection the log can resume from.
const SNAPSHOT: TableDefinition<&str, &[u8]> = TableDefinition::new("snapshot");
/// Small unordered values: the `KeyValue` port, drafts included.
const VALUES: TableDefinition<&str, &[u8]> = TableDefinition::new("values");
/// The schema version and the compaction watermark.
const META: TableDefinition<&str, u64> = TableDefinition::new("meta");

const SNAPSHOT_KEY: &str = "snapshot";
const SCHEMA_VERSION_KEY: &str = "schema_version";
const COMPACTED_KEY: &str = "compacted";

/// What this build of the code writes. Bump it and add a step to [`migration`].
const SCHEMA_VERSION: u64 = 1;

/// One redb file: one fact log plus the key-value table beside it.
#[derive(Clone)]
pub struct Redb {
    database: Arc<Database>,
}

impl Redb {
    /// Opens or creates the database at `path` and brings its schema current.
    ///
    /// Fails while another process holds the same file, which is the intended
    /// answer: one node owns its storage.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let database = Database::create(path)?;
        migrate(&database)?;
        Ok(Self {
            database: Arc::new(database),
        })
    }

    /// The fact log in this file. One file holds one log.
    pub fn log<F: Fact>(&self) -> RedbLog<F> {
        RedbLog {
            database: Arc::clone(&self.database),
            fact: PhantomData,
        }
    }
}

/// Every table this version expects, created before anything reads one.
fn create_tables(write: &WriteTransaction) -> Result<u64> {
    write.open_table(RECORDS)?;
    write.open_table(COMMANDS)?;
    write.open_table(OUTCOMES)?;
    write.open_table(SNAPSHOT)?;
    write.open_table(VALUES)?;
    Ok(1)
}

/// One step per version, each a plain function over one write transaction, so a
/// schema change is code to review rather than a migration language to trust.
fn migration(write: &WriteTransaction, from: u64) -> Result<u64> {
    match from {
        0 => create_tables(write),
        _ => Err(StorageError::Corrupt),
    }
}

fn migrate(database: &Database) -> Result<()> {
    let write = database.begin_write()?;
    let mut version = {
        let meta = write.open_table(META)?;
        meta.get(SCHEMA_VERSION_KEY)?
            .map_or(0, |value| value.value())
    };
    if version > SCHEMA_VERSION {
        // A newer build wrote this file. Refuse rather than reinterpret it.
        return Err(StorageError::Corrupt);
    }
    while version < SCHEMA_VERSION {
        version = migration(&write, version)?;
    }
    write
        .open_table(META)?
        .insert(SCHEMA_VERSION_KEY, version)?;
    write.commit()?;
    Ok(())
}

pub struct RedbLog<F> {
    database: Arc<Database>,
    fact: PhantomData<F>,
}

/// The state an append has to fence against, read inside its own transaction.
struct Head {
    sequence: u64,
    epoch: u64,
}

fn head(write: &WriteTransaction) -> Result<Head> {
    let compacted = watermark(write)?;
    let snapshot = saved(write)?;
    let newest = write
        .open_table(RECORDS)?
        .last()?
        .map(|(_, value)| StoredRecord::decode(value.value()))
        .transpose()?;
    Ok(Head {
        sequence: newest
            .as_ref()
            .map(|record| record.sequence)
            .or_else(|| snapshot.as_ref().map(|s| s.sequence))
            .unwrap_or(0)
            .max(compacted),
        epoch: newest
            .as_ref()
            .map(|record| record.epoch)
            .or_else(|| snapshot.as_ref().map(|s| s.epoch))
            .unwrap_or(1),
    })
}

fn watermark(write: &WriteTransaction) -> Result<u64> {
    Ok(write
        .open_table(META)?
        .get(COMPACTED_KEY)?
        .map_or(0, |value| value.value()))
}

fn saved(write: &WriteTransaction) -> Result<Option<Snapshot>> {
    write
        .open_table(SNAPSHOT)?
        .get(SNAPSHOT_KEY)?
        .map(|value| Ok(StoredSnapshot::decode(value.value())?.into()))
        .transpose()
}

/// A command's record, whether it is still live or already an outcome.
fn outcome<F: Fact>(write: &WriteTransaction, id: &str) -> Result<Option<Record<F>>> {
    if let Some(sequence) = write.open_table(COMMANDS)?.get(id)?
        && let Some(value) = write.open_table(RECORDS)?.get(sequence.value())?
    {
        return Ok(Some(StoredRecord::decode(value.value())?.into_record()?));
    }
    write
        .open_table(OUTCOMES)?
        .get(id)?
        .map(|value| StoredRecord::decode(value.value())?.into_record())
        .transpose()
}

impl<F: Fact> FactLog<F> for RedbLog<F> {
    fn commit(
        &self,
        cursor: Option<u64>,
        epoch: u64,
        id: &str,
        decide: &mut crate::CommitDecision<'_, F>,
    ) -> Result<Option<Record<F>>> {
        let write = self.database.begin_write()?;
        let through = watermark(&write)?;
        if cursor.is_some_and(|cursor| cursor < through) {
            return Err(StorageError::CursorUnavailable { through });
        }
        let head = head(&write)?;
        if epoch < head.epoch {
            return Err(StorageError::Epoch {
                current: head.epoch,
            });
        }
        if cursor.is_some_and(|cursor| cursor > head.sequence) {
            return Err(StorageError::Conflict {
                actual: head.sequence,
            });
        }
        let duplicate = outcome(&write, id)?;
        let records = write.open_table(RECORDS)?;
        let mut unseen = Vec::new();
        if let Some(cursor) = cursor {
            for entry in records.range((
                std::ops::Bound::Excluded(cursor),
                std::ops::Bound::Unbounded,
            ))? {
                let (_, value) = entry?;
                unseen.push(StoredRecord::decode(value.value())?.into_record()?);
            }
        }
        drop(records);
        let Some(fact) = decide(head.sequence, &unseen, duplicate)? else {
            return Ok(None);
        };
        let record = Record {
            sequence: head.sequence.checked_add(1).ok_or(StorageError::Corrupt)?,
            epoch,
            command_id: id.into(),
            fact,
        };
        write.open_table(RECORDS)?.insert(
            record.sequence,
            StoredRecord::from(&record).encode_to_vec().as_slice(),
        )?;
        write.open_table(COMMANDS)?.insert(id, record.sequence)?;
        write.commit()?;
        Ok(Some(record))
    }

    fn outcome_of(&self, id: &str) -> Result<Option<Record<F>>> {
        // A write transaction, not a read one: `outcome` reads three tables and
        // must not see a half-finished compaction between them.
        outcome(&self.database.begin_write()?, id)
    }

    fn read_from(&self, cursor: u64) -> Result<Vec<Record<F>>> {
        let read = self.database.begin_read()?;
        let through = read
            .open_table(META)?
            .get(COMPACTED_KEY)?
            .map_or(0, |value| value.value());
        if cursor < through {
            return Err(StorageError::CursorUnavailable { through });
        }
        let records = read.open_table(RECORDS)?;
        let mut result = Vec::new();
        for entry in records.range((
            std::ops::Bound::Excluded(cursor),
            std::ops::Bound::Unbounded,
        ))? {
            let (_, value) = entry?;
            result.push(StoredRecord::decode(value.value())?.into_record()?);
        }
        Ok(result)
    }

    fn snapshot(&self) -> Result<Option<Snapshot>> {
        saved(&self.database.begin_write()?)
    }

    fn save_snapshot(&self, snapshot: Snapshot) -> Result<()> {
        let write = self.database.begin_write()?;
        let through = watermark(&write)?;
        let newest = write
            .open_table(RECORDS)?
            .last()?
            .map_or(through, |(key, _)| key.value());
        if snapshot.sequence > newest || snapshot.sequence < through {
            return Err(StorageError::SnapshotRequired);
        }
        {
            let mut table = write.open_table(SNAPSHOT)?;
            table.insert(
                SNAPSHOT_KEY,
                StoredSnapshot::from(&snapshot).encode_to_vec().as_slice(),
            )?;
        }
        write.commit()?;
        Ok(())
    }

    fn compact(&self, through: u64) -> Result<()> {
        let write = self.database.begin_write()?;
        if saved(&write)?.is_none_or(|snapshot| snapshot.sequence < through) {
            return Err(StorageError::SnapshotRequired);
        }
        let reached = watermark(&write)?.max(through);
        {
            let mut records = write.open_table(RECORDS)?;
            let mut commands = write.open_table(COMMANDS)?;
            let mut outcomes = write.open_table(OUTCOMES)?;
            let compacted: Vec<(u64, Vec<u8>)> = records
                .range(..=through)?
                .map(|entry| {
                    let (key, value) = entry?;
                    Ok((key.value(), value.value().to_vec()))
                })
                .collect::<Result<_>>()?;
            for (sequence, stored) in compacted {
                let command_id = StoredRecord::decode(&stored[..])?.command_id;
                outcomes.insert(command_id.as_str(), stored.as_slice())?;
                commands.remove(command_id.as_str())?;
                records.remove(sequence)?;
            }
            write.open_table(META)?.insert(COMPACTED_KEY, reached)?;
        }
        write.commit()?;
        tracing::debug!(through = reached, "fact log compacted");
        Ok(())
    }
}

impl KeyValue for Redb {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .database
            .begin_read()?
            .open_table(VALUES)?
            .get(key)?
            .map(|value| value.value().to_vec()))
    }

    fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        let write = self.database.begin_write()?;
        write.open_table(VALUES)?.insert(key, value)?;
        write.commit()?;
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<()> {
        let write = self.database.begin_write()?;
        write.open_table(VALUES)?.remove(key)?;
        write.commit()?;
        Ok(())
    }
}

impl From<RedbError> for StorageError {
    fn from(error: RedbError) -> Self {
        match error {
            RedbError::Corrupted(_)
            | RedbError::UpgradeRequired(_)
            | RedbError::TableTypeMismatch { .. }
            | RedbError::TypeDefinitionChanged { .. } => Self::Corrupt,
            // One node owns its file; a second opener is told so, not queued.
            RedbError::DatabaseAlreadyOpen => Self::Io(ErrorKind::ResourceBusy),
            RedbError::ValueTooLarge(_) => Self::Io(ErrorKind::InvalidInput),
            RedbError::Io(error) => Self::Io(error.kind()),
            _ => Self::Io(ErrorKind::Other),
        }
    }
}

impl From<redb::DatabaseError> for StorageError {
    fn from(error: redb::DatabaseError) -> Self {
        RedbError::from(error).into()
    }
}
impl From<redb::TransactionError> for StorageError {
    fn from(error: redb::TransactionError) -> Self {
        RedbError::from(error).into()
    }
}
impl From<redb::TableError> for StorageError {
    fn from(error: redb::TableError) -> Self {
        RedbError::from(error).into()
    }
}
impl From<redb::CommitError> for StorageError {
    fn from(error: redb::CommitError) -> Self {
        RedbError::from(error).into()
    }
}
impl From<redb::StorageError> for StorageError {
    fn from(error: redb::StorageError) -> Self {
        RedbError::from(error).into()
    }
}
