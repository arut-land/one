//! Serialized command acceptance over a fact log.
//!
//! A [`Command`] names its retry key and epoch, refuses early through
//! [`Command::precondition`], and applies purely. [`Authority`] fences stale
//! epochs, deduplicates retries, appends through [`FactLog`], and reduces the
//! accepted fact into the projection, all under one lock. Draft replication
//! stays outside this durable authority (ADR 0018).
//!
//! Each acceptance runs in one span carrying the command ID, the epoch, and the
//! outcome. The command ID is the surface's own retry key, which is what makes a
//! duplicate legible in a trace; nothing a person wrote is recorded.

use arut_storage::{Fact, FactLog, Record, Snapshot, StorageError};
use prost::Message;
use std::sync::{Arc, Mutex};

pub trait Projection: Default + Clone + 'static {
    type Fact: Fact;
    type Snapshot: Message + Default;

    fn restore(snapshot: Self::Snapshot) -> Self;
    fn snapshot(&self) -> Self::Snapshot;
    fn reduce(&mut self, fact: &Self::Fact);
}

/// Why a command refuses itself against the projection it would apply to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conflict {
    Revision { current: u64 },
    Superseded,
}

pub trait Command: Send + 'static {
    type Fact: Fact;
    type Projection: Projection<Fact = Self::Fact>;
    type Rejection;
    fn command_id(&self) -> &str;
    fn epoch(&self) -> u64;
    /// Refuses before [`Command::apply`] runs. Accepts by default.
    ///
    /// The projection is the one the fact would apply to, already refreshed, so
    /// a command asks it whatever its own staleness rule needs.
    fn precondition(&self, _current: &Self::Projection) -> Result<(), Conflict> {
        Ok(())
    }
    fn expires_at(&self) -> Option<u64> {
        None
    }
    fn apply(self, current: &Self::Projection, now: u64) -> Result<Self::Fact, Self::Rejection>;
}

#[derive(Debug)]
pub enum Outcome<F, E> {
    Applied(Record<F>),
    Duplicate(Record<F>),
    RevisionConflict { current: u64 },
    AuthorityMismatch { current_epoch: u64 },
    Superseded,
    Rejected(E),
}

struct State<P> {
    projection: P,
    cursor: u64,
}

pub struct Authority<C: Command> {
    log: Arc<dyn FactLog<C::Fact>>,
    epoch: u64,
    state: Mutex<State<C::Projection>>,
}

impl<C: Command> Authority<C> {
    pub fn open(log: Arc<dyn FactLog<C::Fact>>, epoch: u64) -> Result<Self, StorageError> {
        let (mut projection, mut cursor, mut current_epoch) = match log.snapshot()? {
            Some(snapshot) => (
                C::Projection::restore(<C::Projection as Projection>::Snapshot::decode(
                    &snapshot.data[..],
                )?),
                snapshot.sequence,
                snapshot.epoch,
            ),
            None => (C::Projection::default(), 0, 1),
        };
        for record in log.read_from(cursor)? {
            C::Projection::reduce(&mut projection, &record.fact);
            cursor = record.sequence;
            current_epoch = record.epoch;
        }
        if epoch < current_epoch {
            return Err(StorageError::Epoch {
                current: current_epoch,
            });
        }
        Ok(Self {
            log,
            epoch,
            state: Mutex::new(State { projection, cursor }),
        })
    }
    pub fn projection(&self) -> C::Projection {
        self.read_projection(Clone::clone)
    }
    /// Reads under the authority lock; the callback must not re-enter this authority.
    pub fn read_projection<T>(&self, read: impl FnOnce(&C::Projection) -> T) -> T {
        read(&self.state.lock().unwrap().projection)
    }
    pub fn outcome_of(&self, id: &str) -> Result<Option<Record<C::Fact>>, StorageError> {
        self.log.outcome_of(id)
    }
    /// Reads time inside the transaction after deduplication, before application.
    pub fn execute_with_clock(
        &self,
        command: C,
        now: impl Fn() -> u64,
    ) -> Result<Outcome<C::Fact, C::Rejection>, StorageError> {
        let commit = tracing::debug_span!(
            "authority.commit",
            command_id = command.command_id(),
            epoch = command.epoch()
        );
        let _commit = commit.enter();
        let mut state = self.state.lock().map_err(|_| StorageError::Corrupt)?;
        if command.epoch() != self.epoch {
            return Ok(Outcome::AuthorityMismatch {
                current_epoch: self.epoch,
            });
        }
        let id = command.command_id().to_owned();
        let mut outcome = None;
        let cursor = state.cursor;
        let result = self.log.commit(
            Some(cursor),
            self.epoch,
            &id,
            Box::new(|_, records, duplicate| {
                for record in records {
                    state.projection.reduce(&record.fact);
                    state.cursor = record.sequence;
                }
                if let Some(record) = duplicate {
                    outcome = Some(Outcome::Duplicate(record));
                    return Ok(None);
                }
                match decide(command, now(), &state.projection) {
                    Ok(fact) => Ok(Some(fact)),
                    Err(rejected) => {
                        outcome = Some(rejected);
                        Ok(None)
                    }
                }
            }),
        );
        let record = match result {
            Ok(Some(record)) => record,
            Ok(None) => return outcome.ok_or(StorageError::Corrupt),
            Err(StorageError::Conflict { actual }) => {
                return Ok(Outcome::RevisionConflict { current: actual });
            }
            Err(StorageError::Epoch { current }) => {
                return Ok(Outcome::AuthorityMismatch {
                    current_epoch: current,
                });
            }
            Err(error) => return Err(error),
        };
        state.projection.reduce(&record.fact);
        state.cursor = record.sequence;
        tracing::debug!(outcome = "applied", sequence = record.sequence);
        Ok(Outcome::Applied(record))
    }
    pub fn checkpoint(&self, compact: bool) -> Result<(), StorageError> {
        let state = self.state.lock().map_err(|_| StorageError::Corrupt)?;
        tracing::debug!(
            sequence = state.cursor,
            epoch = self.epoch,
            compact,
            "checkpointing the fact log"
        );
        self.log.save_snapshot(Snapshot {
            sequence: state.cursor,
            epoch: self.epoch,
            data: state.projection.snapshot().encode_to_vec(),
        })?;
        if compact {
            self.log.compact(state.cursor)?;
        }
        Ok(())
    }
}

fn decide<C: Command>(
    command: C,
    now: u64,
    projection: &C::Projection,
) -> Result<C::Fact, Outcome<C::Fact, C::Rejection>> {
    if command.expires_at().is_some_and(|expiry| now >= expiry) {
        return Err(Outcome::Superseded);
    }
    match command.precondition(projection) {
        Ok(()) => {}
        Err(Conflict::Revision { current }) => return Err(Outcome::RevisionConflict { current }),
        Err(Conflict::Superseded) => return Err(Outcome::Superseded),
    }
    command.apply(projection, now).map_err(Outcome::Rejected)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Clone, PartialEq, Message)]
    struct Tick {
        #[prost(uint64, tag = "1")]
        amount: u64,
    }
    #[derive(Clone, PartialEq, Message)]
    struct Counter {
        #[prost(uint64, tag = "1")]
        total: u64,
    }
    impl Projection for Counter {
        type Fact = Tick;
        type Snapshot = Self;
        fn restore(snapshot: Self::Snapshot) -> Self {
            snapshot
        }
        fn snapshot(&self) -> Self::Snapshot {
            self.clone()
        }
        fn reduce(&mut self, fact: &Tick) {
            self.total += fact.amount;
        }
    }
    struct Add {
        id: String,
        epoch: u64,
        expected: Option<u64>,
        expiry: Option<u64>,
    }
    impl Command for Add {
        type Fact = Tick;
        type Projection = Counter;
        type Rejection = ();
        fn command_id(&self) -> &str {
            &self.id
        }
        fn epoch(&self) -> u64 {
            self.epoch
        }
        fn precondition(&self, current: &Counter) -> Result<(), Conflict> {
            match self.expected {
                Some(expected) if expected != current.total => Err(Conflict::Revision {
                    current: current.total,
                }),
                _ => Ok(()),
            }
        }
        fn expires_at(&self) -> Option<u64> {
            self.expiry
        }
        fn apply(self, _: &Counter, _: u64) -> Result<Tick, ()> {
            Ok(Tick { amount: 1 })
        }
    }
    fn add(id: &str, expected: u64) -> Add {
        Add {
            id: id.into(),
            epoch: 1,
            expected: Some(expected),
            expiry: None,
        }
    }
    fn execute(authority: &Authority<Add>, command: Add) -> Outcome<Tick, ()> {
        authority.execute_with_clock(command, || 0).unwrap()
    }

    #[test]
    fn retries_do_not_read_the_acceptance_clock() {
        let authority =
            Authority::<Add>::open(Arc::new(arut_storage::MemoryLog::default()), 1).unwrap();
        let reads = std::cell::Cell::new(0);
        let clock = || {
            reads.set(reads.get() + 1);
            123
        };
        assert!(matches!(
            authority.execute_with_clock(add("one", 0), clock).unwrap(),
            Outcome::Applied(_)
        ));
        assert!(matches!(
            authority.execute_with_clock(add("one", 0), clock).unwrap(),
            Outcome::Duplicate(_)
        ));
        assert_eq!(reads.get(), 1);
    }

    #[test]
    fn fences_deduplicates_checks_preconditions_and_replays_after_compaction() {
        let log = Arc::new(arut_storage::MemoryLog::default());
        let authority = Authority::<Add>::open(log.clone(), 1).unwrap();
        assert!(matches!(
            execute(&authority, add("one", 0)),
            Outcome::Applied(_)
        ));
        assert!(matches!(
            execute(&authority, add("one", 0)),
            Outcome::Duplicate(_)
        ));
        assert!(matches!(
            execute(&authority, add("conflict", 0)),
            Outcome::RevisionConflict { current: 1 }
        ));
        let mut stale = add("one", 0);
        stale.epoch = 0;
        assert!(matches!(
            execute(&authority, stale),
            Outcome::AuthorityMismatch { current_epoch: 1 }
        ));
        let mut expired = add("expired", 1);
        expired.expiry = Some(10);
        assert!(matches!(
            authority.execute_with_clock(expired, || 10).unwrap(),
            Outcome::Superseded
        ));
        authority.checkpoint(true).unwrap();
        let restarted = Authority::<Add>::open(log, 1).unwrap();
        assert_eq!(restarted.projection().total, 1);
        assert!(matches!(
            execute(&restarted, add("one", 0)),
            Outcome::Duplicate(_)
        ));
        execute(&restarted, add("two", 1));
        assert_eq!(restarted.projection().total, 2);
    }
}
