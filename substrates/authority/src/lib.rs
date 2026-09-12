//! # Authority
//!
//! Command defines scope, facts, pure application, preconditions, expiry, queueability,
//! and optimism. Authority serializes acceptance, fences epochs before deduplication,
//! checks preconditions, appends through FactLog, and reduces the accepted fact.
//! Outcomes distinguish application, duplicate delivery, revision conflict, stale
//! authority, supersession, and rejection. Projection reducers have no I/O.
//!
//! Snapshot and compaction are explicit maintenance operations. Draft replication
//! stays outside this durable authority, as required by ADR 0018.

//! Generic acceptance with pure command application and projection reduction.
use arut_storage::{Fact, FactLog, Record, Snapshot, StorageError};
use serde::{Serialize, de::DeserializeOwned};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Optimism {
    Immediate,
    Authoritative,
}
pub trait Projection: Default + Clone + Serialize + DeserializeOwned + Send + 'static {
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
    const QUEUEABLE: bool;
    const OPTIMISM: Optimism;
    fn command_id(&self) -> &str;
    fn scope(&self) -> &Self::Scope;
    fn epoch(&self) -> u64;
    fn precondition(&self) -> Precondition {
        Precondition::None
    }
    fn expires_at(&self) -> Option<u64> {
        None
    }
    fn apply(self, current: &Self::Projection) -> Result<Self::Fact, Self::Rejection>;
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
            .map(|s| serde_json::from_slice(&s.data))
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
        self.state.lock().unwrap().projection.clone()
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
        let mut state = self.state.lock().unwrap();
        if command.epoch() != self.epoch {
            return Ok(Outcome::AuthorityMismatch {
                current_epoch: self.epoch,
            });
        }
        for record in self.log.read_from(state.cursor)? {
            if record.epoch > self.epoch {
                return Ok(Outcome::AuthorityMismatch {
                    current_epoch: record.epoch,
                });
            }
            state.projection.reduce(&record.fact);
            state.cursor = record.sequence;
        }
        if let Some(record) = self.log.outcome_of(command.command_id())? {
            return Ok(Outcome::Duplicate(record));
        }
        if command.expires_at().is_some_and(|expiry| now >= expiry) {
            return Ok(Outcome::Superseded);
        }
        match command.precondition() {
            Precondition::Revision(expected) => {
                let current = state.projection.revision(command.scope());
                if expected != current {
                    return Ok(Outcome::RevisionConflict { current });
                }
            }
            Precondition::Epoch(epoch) if epoch != self.epoch => {
                return Ok(Outcome::AuthorityMismatch {
                    current_epoch: self.epoch,
                });
            }
            Precondition::OperationOpen(id) if !state.projection.operation_open(&id) => {
                return Ok(Outcome::Superseded);
            }
            _ => {}
        }
        let id = command.command_id().to_owned();
        let fact = match command.apply(&state.projection) {
            Ok(fact) => fact,
            Err(error) => return Ok(Outcome::Rejected(error)),
        };
        let record = match self.log.append(state.cursor, self.epoch, &id, fact) {
            Ok(record) => record,
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
        Ok(Outcome::Applied(record))
    }
    pub fn checkpoint(&self, compact: bool) -> Result<(), StorageError> {
        let state = self.state.lock().unwrap();
        self.log.save_snapshot(Snapshot {
            sequence: state.cursor,
            epoch: self.epoch,
            data: serde_json::to_vec(&state.projection)?,
        })?;
        if compact {
            self.log.compact(state.cursor)?;
        }
        Ok(())
    }
}

/// Pure control-flow machines can emit effects for a host driver.
pub trait Machine {
    type Input;
    type Effect;
    fn handle(&mut self, input: Self::Input) -> Vec<Self::Effect>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    #[derive(Clone, Default, Serialize, Deserialize)]
    struct Counter(u64);
    impl Projection for Counter {
        type Fact = u64;
        type Scope = String;
        fn reduce(&mut self, fact: &u64) {
            self.0 += fact;
        }
        fn revision(&self, _: &String) -> u64 {
            self.0
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
        type Fact = u64;
        type Projection = Counter;
        type Rejection = ();
        const QUEUEABLE: bool = true;
        const OPTIMISM: Optimism = Optimism::Authoritative;
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
        fn apply(self, _: &Counter) -> Result<u64, ()> {
            Ok(1)
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
        assert_eq!(restarted.projection().0, 1);
        assert!(matches!(
            restarted.execute(add("one", 0)).unwrap(),
            Outcome::Duplicate(_)
        ));
        restarted.execute(add("two", 1)).unwrap();
        assert_eq!(restarted.projection().0, 2);
    }
}
