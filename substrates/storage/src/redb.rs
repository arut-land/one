//! Fact logs and key-value data in one redb file with typed tables.
//!
//! The database owns its file exclusively, so other processes reach these facts
//! through the daemon's RPC channel.

use crate::log::{LogStore, commit_policy, compaction_allowed, snapshot_allowed};
use crate::{Fact, FactLog, KeyValue, Record, Result, Snapshot, StorageError};
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

fn watermark(meta: &impl ReadableTable<&'static str, u64>) -> Result<u64> {
    Ok(meta.get(COMPACTED_KEY)?.map_or(0, |value| value.value()))
}

fn saved(table: &impl ReadableTable<&'static str, &'static [u8]>) -> Result<Option<Snapshot>> {
    table
        .get(SNAPSHOT_KEY)?
        .map(|value| Ok(Snapshot::decode(value.value())?))
        .transpose()
}

/// Records strictly after `cursor`, decoded in sequence order.
fn after<F: Fact>(
    records: &impl ReadableTable<u64, &'static [u8]>,
    cursor: u64,
) -> Result<Vec<Record<F>>> {
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

/// A command's record, whether it is still live or already an outcome.
fn outcome<F: Fact>(
    records: &impl ReadableTable<u64, &'static [u8]>,
    commands: &impl ReadableTable<&'static str, u64>,
    outcomes: &impl ReadableTable<&'static str, &'static [u8]>,
    id: &str,
) -> Result<Option<Record<F>>> {
    if let Some(sequence) = commands.get(id)?
        && let Some(value) = records.get(sequence.value())?
    {
        return Ok(Some(StoredRecord::decode(value.value())?.into_record()?));
    }
    outcomes
        .get(id)?
        .map(|value| StoredRecord::decode(value.value())?.into_record())
        .transpose()
}

/// One redb write transaction, holding the log for the length of one commit.
struct Commit<F> {
    write: WriteTransaction,
    fact: PhantomData<F>,
}

impl<F: Fact> LogStore<F> for Commit<F> {
    fn head(&mut self) -> Result<(u64, u64)> {
        let compacted = watermark(&self.write.open_table(META)?)?;
        let snapshot = saved(&self.write.open_table(SNAPSHOT)?)?;
        let newest = self
            .write
            .open_table(RECORDS)?
            .last()?
            .map(|(_, value)| StoredRecord::decode(value.value()))
            .transpose()?;
        let read = |from_record: fn(&StoredRecord) -> u64, from_snapshot: fn(&Snapshot) -> u64| {
            newest
                .as_ref()
                .map(from_record)
                .or_else(|| snapshot.as_ref().map(from_snapshot))
        };
        Ok((
            read(|record| record.sequence, |snapshot| snapshot.sequence)
                .unwrap_or(0)
                .max(compacted),
            read(|record| record.epoch, |snapshot| snapshot.epoch).unwrap_or(1),
        ))
    }

    fn watermark(&mut self) -> Result<u64> {
        watermark(&self.write.open_table(META)?)
    }

    fn duplicate(&mut self, command_id: &str) -> Result<Option<Record<F>>> {
        outcome(
            &self.write.open_table(RECORDS)?,
            &self.write.open_table(COMMANDS)?,
            &self.write.open_table(OUTCOMES)?,
            command_id,
        )
    }

    fn after(&mut self, cursor: u64) -> Result<Vec<Record<F>>> {
        after(&self.write.open_table(RECORDS)?, cursor)
    }

    fn push(&mut self, record: &Record<F>) -> Result<()> {
        self.write.open_table(RECORDS)?.insert(
            record.sequence,
            StoredRecord::from(record).encode_to_vec().as_slice(),
        )?;
        self.write
            .open_table(COMMANDS)?
            .insert(record.command_id.as_str(), record.sequence)?;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        self.write.commit()?;
        Ok(())
    }
}

impl<F: Fact> FactLog<F> for RedbLog<F> {
    fn commit(
        &self,
        cursor: Option<u64>,
        epoch: u64,
        id: &str,
        decide: &mut crate::CommitDecision<'_, F>,
    ) -> Result<Option<Record<F>>> {
        let commit = Commit {
            write: self.database.begin_write()?,
            fact: PhantomData,
        };
        commit_policy(commit, cursor, epoch, id, decide)
    }

    fn outcome_of(&self, id: &str) -> Result<Option<Record<F>>> {
        let read = self.database.begin_read()?;
        outcome(
            &read.open_table(RECORDS)?,
            &read.open_table(COMMANDS)?,
            &read.open_table(OUTCOMES)?,
            id,
        )
    }

    fn read_from(&self, cursor: u64) -> Result<Vec<Record<F>>> {
        let read = self.database.begin_read()?;
        let through = watermark(&read.open_table(META)?)?;
        if cursor < through {
            return Err(StorageError::CursorUnavailable { through });
        }
        after(&read.open_table(RECORDS)?, cursor)
    }

    fn snapshot(&self) -> Result<Option<Snapshot>> {
        let read = self.database.begin_read()?;
        saved(&read.open_table(SNAPSHOT)?)
    }

    fn save_snapshot(&self, snapshot: Snapshot) -> Result<()> {
        let write = self.database.begin_write()?;
        let through = watermark(&write.open_table(META)?)?;
        let newest = write
            .open_table(RECORDS)?
            .last()?
            .map_or(through, |(key, _)| key.value());
        snapshot_allowed(&snapshot, newest, through)?;
        {
            let mut table = write.open_table(SNAPSHOT)?;
            table.insert(SNAPSHOT_KEY, snapshot.encode_to_vec().as_slice())?;
        }
        write.commit()?;
        Ok(())
    }

    fn compact(&self, through: u64) -> Result<()> {
        let write = self.database.begin_write()?;
        compaction_allowed(saved(&write.open_table(SNAPSHOT)?)?.as_ref(), through)?;
        let reached = watermark(&write.open_table(META)?)?.max(through);
        {
            let mut records = write.open_table(RECORDS)?;
            let mut commands = write.open_table(COMMANDS)?;
            let mut outcomes = write.open_table(OUTCOMES)?;
            // `extract_from_if` removes each row as it is read, so the range
            // is never materialized and nothing walks it twice.
            for entry in records.extract_from_if(..=through, |_, _| true)? {
                let stored = entry?.1.value().to_vec();
                let command_id = StoredRecord::decode(&stored[..])?.command_id;
                outcomes.insert(command_id.as_str(), stored.as_slice())?;
                commands.remove(command_id.as_str())?;
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

macro_rules! storage_errors {
    ($($error:ty),+ $(,)?) => {$(
        impl From<$error> for StorageError {
            fn from(error: $error) -> Self { RedbError::from(error).into() }
        }
    )+};
}
storage_errors!(
    redb::DatabaseError,
    redb::TransactionError,
    redb::TableError,
    redb::CommitError,
    redb::StorageError
);

/// The on-disk shape of a record; the fact keeps its own Protobuf encoding.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct StoredRecord {
    #[prost(uint64, tag = "1")]
    pub sequence: u64,
    #[prost(uint64, tag = "2")]
    pub epoch: u64,
    #[prost(string, tag = "3")]
    pub command_id: String,
    #[prost(bytes = "vec", tag = "4")]
    pub fact: Vec<u8>,
}
impl<F: Fact> From<&Record<F>> for StoredRecord {
    fn from(record: &Record<F>) -> Self {
        Self {
            sequence: record.sequence,
            epoch: record.epoch,
            command_id: record.command_id.clone(),
            fact: record.fact.encode_to_vec(),
        }
    }
}
impl StoredRecord {
    pub(crate) fn into_record<F: Fact>(self) -> Result<Record<F>> {
        Ok(Record {
            sequence: self.sequence,
            epoch: self.epoch,
            command_id: self.command_id,
            fact: F::decode(&self.fact[..])?,
        })
    }
}
