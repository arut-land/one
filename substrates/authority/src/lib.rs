//! # Authority
//!
//! Command defines scope, facts, pure application, preconditions, and expiry.
//! Authority serializes acceptance, fences epochs before deduplication,
//! checks preconditions, appends through FactLog, and reduces the accepted fact.
//! Outcomes distinguish application, duplicate delivery, revision conflict, stale
//! authority, supersession, and rejection. Projection reducers have no I/O.
//!
//! Snapshot and compaction are explicit maintenance operations. Draft replication
//! stays outside this durable authority, as required by ADR 0018.
//!
//! Every acceptance runs inside one span carrying the command ID, the epoch, and
//! the outcome it reached. The command ID is the surface's own retry key, which
//! is what makes a duplicate legible in a trace; nothing a person wrote is ever
//! recorded. No subscriber is installed here.

//! Generic acceptance with pure command application and projection reduction.
use arut_storage::{Fact, FactLog, Record, Snapshot, StorageError};
use prost::Message;
use std::{
    hash::Hash,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Precondition {
    None,
    Revision(u64),
    Epoch(u64),
    OperationOpen(String),
}
pub trait Projection: Message + Default + Clone + 'static {
    type Fact: Fact;
    type Scope: Eq + Hash + Clone;
    fn reduce(&mut self, fact: &Self::Fact);
    fn revision(&self, scope: &Self::Scope) -> u64;
    fn operation_open(&self, _id: &str) -> bool {
        false
    }
}
pub trait Command: Send + 'static {
    type Scope: Eq + Hash + Clone;
    type Fact: Fact;
    type Projection: Projection<Fact = Self::Fact, Scope = Self::Scope>;
    type Rejection;
    fn command_id(&self) -> &str;
    fn scope(&self) -> &Self::Scope;
    fn epoch(&self) -> u64;
    fn precondition(&self) -> Precondition {
        Precondition::None
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
        let snapshot = log.snapshot()?;
        let mut projection = snapshot
            .as_ref()
            .map(|s| C::Projection::decode(&s.data[..]))
            .transpose()?
            .unwrap_or_default();
        let mut cursor = snapshot.as_ref().map_or(0, |s| s.sequence);
        let mut current_epoch = snapshot.as_ref().map_or(1, |s| s.epoch);
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
    pub fn execute(&self, command: C) -> Result<Outcome<C::Fact, C::Rejection>, StorageError> {
        self.execute_at(command, 0)
    }
    pub fn execute_at(
        &self,
        command: C,
        now: u64,
    ) -> Result<Outcome<C::Fact, C::Rejection>, StorageError> {
        self.execute_with_clock(command, || now)
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
        let mut command = Some(command);
        let mut outcome = None;
        let cursor = state.cursor;
        let result = self.log.commit(
            Some(cursor),
            self.epoch,
            &id,
            &mut |_, records, duplicate| {
                for record in records {
                    state.projection.reduce(&record.fact);
                    state.cursor = record.sequence;
                }
                if let Some(record) = duplicate {
                    outcome = Some(Outcome::Duplicate(record));
                    return Ok(None);
                }
                match self.apply(
                    command.take().ok_or(StorageError::Corrupt)?,
                    now(),
                    &state.projection,
                ) {
                    Ok(fact) => Ok(Some(fact)),
                    Err(rejected) => {
                        outcome = Some(rejected);
                        Ok(None)
                    }
                }
            },
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
    fn apply(
        &self,
        command: C,
        now: u64,
        projection: &C::Projection,
    ) -> Result<C::Fact, Outcome<C::Fact, C::Rejection>> {
        if command.expires_at().is_some_and(|expiry| now >= expiry) {
            return Err(Outcome::Superseded);
        }
        match command.precondition() {
            Precondition::Revision(expected) => {
                let current = projection.revision(command.scope());
                if expected != current {
                    return Err(Outcome::RevisionConflict { current });
                }
            }
            Precondition::Epoch(epoch) if epoch != self.epoch => {
                return Err(Outcome::AuthorityMismatch {
                    current_epoch: self.epoch,
                });
            }
            Precondition::OperationOpen(id) if !projection.operation_open(&id) => {
                return Err(Outcome::Superseded);
            }
            _ => {}
        }
        command.apply(projection, now).map_err(Outcome::Rejected)
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
            data: state.projection.encode_to_vec(),
        })?;
        if compact {
            self.log.compact(state.cursor)?;
        }
        Ok(())
    }
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
        type Scope = String;
        fn reduce(&mut self, fact: &Tick) {
            self.total += fact.amount;
        }
        fn revision(&self, _: &String) -> u64 {
            self.total
        }
    }
    struct Add {
        id: String,
        scope: String,
        epoch: u64,
        precondition: Precondition,
        expiry: Option<u64>,
    }
    impl Command for Add {
        type Scope = String;
        type Fact = Tick;
        type Projection = Counter;
        type Rejection = ();
        fn command_id(&self) -> &str {
            &self.id
        }
        fn scope(&self) -> &String {
            &self.scope
        }
        fn epoch(&self) -> u64 {
            self.epoch
        }
        fn precondition(&self) -> Precondition {
            self.precondition.clone()
        }
        fn expires_at(&self) -> Option<u64> {
            self.expiry
        }
        fn apply(self, _: &Counter, _: u64) -> Result<Tick, ()> {
            Ok(Tick { amount: 1 })
        }
    }
    fn add(id: &str, revision: u64) -> Add {
        Add {
            id: id.into(),
            scope: "counter".into(),
            epoch: 1,
            precondition: Precondition::Revision(revision),
            expiry: None,
        }
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
            authority.execute(add("one", 0)).unwrap(),
            Outcome::Applied(_)
        ));
        assert!(matches!(
            authority.execute(add("one", 0)).unwrap(),
            Outcome::Duplicate(_)
        ));
        assert!(matches!(
            authority.execute(add("conflict", 0)).unwrap(),
            Outcome::RevisionConflict { current: 1 }
        ));
        let mut stale = add("one", 0);
        stale.epoch = 0;
        assert!(matches!(
            authority.execute(stale).unwrap(),
            Outcome::AuthorityMismatch { current_epoch: 1 }
        ));
        let mut expired = add("expired", 1);
        expired.expiry = Some(10);
        assert!(matches!(
            authority.execute_at(expired, 10).unwrap(),
            Outcome::Superseded
        ));
        let mut closed = add("closed", 1);
        closed.precondition = Precondition::OperationOpen("done".into());
        assert!(matches!(
            authority.execute(closed).unwrap(),
            Outcome::Superseded
        ));
        authority.checkpoint(true).unwrap();
        let restarted = Authority::<Add>::open(log, 1).unwrap();
        assert_eq!(restarted.projection().total, 1);
        assert!(matches!(
            restarted.execute(add("one", 0)).unwrap(),
            Outcome::Duplicate(_)
        ));
        restarted.execute(add("two", 1)).unwrap();
        assert_eq!(restarted.projection().total, 2);
    }
}
