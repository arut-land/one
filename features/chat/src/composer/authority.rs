use super::{
    AppliedCommand, ComposerCheckpoint, ComposerScope, ComposerScopeCheckpoint, ComposerSnapshot,
    DraftReplaced, ReplaceComposer, ReplaceOutcome,
};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
pub struct ComposerAuthority {
    inner: Mutex<HashMap<ComposerScope, ScopeState>>,
}

#[derive(Clone)]
struct ScopeState {
    snapshot: ComposerSnapshot,
    commands: HashMap<String, AppliedCommand>,
    facts: Vec<DraftReplaced>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromoteError {
    RevisionConflict(ComposerSnapshot),
    TextMismatch(ComposerSnapshot),
}

impl ScopeState {
    fn empty(scope: ComposerScope) -> Self {
        Self {
            snapshot: ComposerSnapshot::empty(scope),
            commands: HashMap::new(),
            facts: Vec::new(),
        }
    }
}

impl ComposerAuthority {
    pub fn from_checkpoint(checkpoint: ComposerCheckpoint) -> Self {
        let scopes = checkpoint
            .chat_scopes
            .into_iter()
            .filter(|checkpoint| checkpoint.snapshot.scope.is_durable())
            .map(|checkpoint| {
                let scope = checkpoint.snapshot.scope.clone();
                let state = ScopeState {
                    snapshot: checkpoint.snapshot,
                    commands: checkpoint
                        .commands
                        .into_iter()
                        .map(|command| (command.command.command_id.clone(), command))
                        .collect(),
                    facts: checkpoint.facts,
                };
                (scope, state)
            })
            .collect();
        Self {
            inner: Mutex::new(scopes),
        }
    }

    pub fn snapshot(&self, scope: &ComposerScope) -> ComposerSnapshot {
        let mut scopes = self.inner.lock().expect("composer authority lock poisoned");
        scopes
            .entry(scope.clone())
            .or_insert_with(|| ScopeState::empty(scope.clone()))
            .snapshot
            .clone()
    }

    pub fn replace(&self, command: ReplaceComposer) -> ReplaceOutcome {
        self.replace_durable(command, |_| Ok::<_, core::convert::Infallible>(()))
            .expect("infallible composer persistence failed")
    }

    pub fn replace_durable<E>(
        &self,
        command: ReplaceComposer,
        persist: impl FnOnce(&ComposerCheckpoint) -> Result<(), E>,
    ) -> Result<ReplaceOutcome, E> {
        let mut scopes = self.inner.lock().expect("composer authority lock poisoned");
        let current = scopes
            .entry(command.scope.clone())
            .or_insert_with(|| ScopeState::empty(command.scope.clone()));
        if command.authority_epoch != current.snapshot.authority_epoch {
            return Ok(ReplaceOutcome::AuthorityMismatch {
                current_epoch: current.snapshot.authority_epoch,
            });
        }
        if let Some(applied) = current.commands.get(&command.command_id) {
            return Ok(ReplaceOutcome::Applied {
                fact: applied.fact.clone(),
                snapshot: snapshot_from_fact(&applied.fact),
                duplicate: true,
            });
        }
        if command.base_revision != current.snapshot.revision {
            return Ok(ReplaceOutcome::RevisionConflict {
                snapshot: current.snapshot.clone(),
            });
        }

        let mut next = current.clone();
        next.snapshot.text.clone_from(&command.text);
        next.snapshot.revision = next
            .snapshot
            .revision
            .checked_add(1)
            .expect("composer revision overflowed");
        let fact = DraftReplaced {
            scope: command.scope.clone(),
            command_id: command.command_id.clone(),
            authority_epoch: command.authority_epoch,
            revision: next.snapshot.revision,
            text: command.text.clone(),
        };
        let outcome = ReplaceOutcome::Applied {
            fact: fact.clone(),
            snapshot: next.snapshot.clone(),
            duplicate: false,
        };
        next.commands.insert(
            command.command_id.clone(),
            AppliedCommand {
                command,
                fact: fact.clone(),
            },
        );
        next.facts.push(fact);

        if next.snapshot.scope.is_durable() {
            let mut candidate = scopes.clone();
            candidate.insert(next.snapshot.scope.clone(), next.clone());
            persist(&checkpoint(&candidate))?;
        }
        scopes.insert(next.snapshot.scope.clone(), next);
        Ok(outcome)
    }

    pub fn facts_after(&self, scope: &ComposerScope, revision: u64) -> Vec<DraftReplaced> {
        self.inner
            .lock()
            .expect("composer authority lock poisoned")
            .get(scope)
            .map(|state| {
                state
                    .facts
                    .iter()
                    .filter(|fact| fact.revision > revision)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn promote_pending<T, E>(
        &self,
        pending_scope_id: &str,
        expected_revision: u64,
        text: &str,
        chat_id: &str,
        commit_chat: impl FnOnce() -> Result<T, E>,
    ) -> Result<Result<(T, ComposerSnapshot), PromoteError>, E> {
        let pending_scope = ComposerScope::pending(pending_scope_id);
        let mut scopes = self.inner.lock().expect("composer authority lock poisoned");
        let pending = scopes
            .entry(pending_scope.clone())
            .or_insert_with(|| ScopeState::empty(pending_scope));
        if pending.snapshot.revision != expected_revision {
            return Ok(Err(PromoteError::RevisionConflict(
                pending.snapshot.clone(),
            )));
        }
        if pending.snapshot.text != text {
            return Ok(Err(PromoteError::TextMismatch(pending.snapshot.clone())));
        }

        let committed = commit_chat()?;
        let chat_scope = ComposerScope::chat(chat_id);
        let chat_snapshot = ComposerSnapshot::empty(chat_scope.clone());
        scopes.insert(chat_scope, ScopeState::empty(chat_snapshot.scope.clone()));
        scopes.insert(
            ComposerScope::pending(pending_scope_id),
            ScopeState::empty(ComposerScope::pending(pending_scope_id)),
        );
        Ok(Ok((committed, chat_snapshot)))
    }

    pub fn checkpoint(&self) -> ComposerCheckpoint {
        checkpoint(&self.inner.lock().expect("composer authority lock poisoned"))
    }
}

fn checkpoint(scopes: &HashMap<ComposerScope, ScopeState>) -> ComposerCheckpoint {
    let mut chat_scopes = scopes
        .values()
        .filter(|state| state.snapshot.scope.is_durable())
        .map(|state| {
            let mut commands = state.commands.values().cloned().collect::<Vec<_>>();
            commands.sort_by_key(|command| command.fact.revision);
            ComposerScopeCheckpoint {
                snapshot: state.snapshot.clone(),
                commands,
                facts: state.facts.clone(),
            }
        })
        .collect::<Vec<_>>();
    chat_scopes
        .sort_by(|left, right| scope_id(&left.snapshot.scope).cmp(scope_id(&right.snapshot.scope)));
    ComposerCheckpoint { chat_scopes }
}

fn scope_id(scope: &ComposerScope) -> &str {
    match scope {
        ComposerScope::Pending(id) | ComposerScope::Chat(id) => id,
    }
}

fn snapshot_from_fact(fact: &DraftReplaced) -> ComposerSnapshot {
    ComposerSnapshot {
        scope: fact.scope.clone(),
        authority_epoch: fact.authority_epoch,
        text: fact.text.clone(),
        revision: fact.revision,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replace(
        scope: ComposerScope,
        command_id: &str,
        revision: u64,
        text: &str,
    ) -> ReplaceComposer {
        ReplaceComposer {
            scope,
            command_id: command_id.into(),
            authority_epoch: 1,
            base_revision: revision,
            text: text.into(),
        }
    }

    #[test]
    fn scopes_have_independent_revisions_deduplication_and_facts() {
        let authority = ComposerAuthority::default();
        let one = ComposerScope::chat("one");
        let two = ComposerScope::chat("two");
        let first = replace(one.clone(), "same-command", 0, "first");

        authority.replace(first.clone());
        authority.replace(replace(two.clone(), "same-command", 0, "second"));

        assert_eq!(authority.snapshot(&one).text, "first");
        assert_eq!(authority.snapshot(&two).text, "second");
        assert_eq!(authority.facts_after(&one, 0).len(), 1);
        assert!(matches!(
            authority.replace(first),
            ReplaceOutcome::Applied {
                duplicate: true,
                ..
            }
        ));
    }

    #[test]
    fn pending_scopes_are_excluded_from_checkpoints() {
        let authority = ComposerAuthority::default();
        authority.replace(replace(
            ComposerScope::pending("new"),
            "pending",
            0,
            "draft",
        ));
        authority.replace(replace(ComposerScope::chat("saved"), "chat", 0, "durable"));

        let checkpoint = authority.checkpoint();
        assert_eq!(checkpoint.chat_scopes.len(), 1);
        assert_eq!(
            checkpoint.chat_scopes[0].snapshot.scope,
            ComposerScope::chat("saved")
        );
    }

    #[test]
    fn promotion_validates_and_clears_the_pending_scope() {
        let authority = ComposerAuthority::default();
        let pending = ComposerScope::pending("new");
        authority.replace(replace(pending.clone(), "draft", 0, "hello"));

        let result = authority
            .promote_pending("new", 1, "hello", "chat", || Ok::<_, ()>(42))
            .unwrap()
            .unwrap();

        assert_eq!(result.0, 42);
        assert_eq!(result.1.scope, ComposerScope::chat("chat"));
        assert_eq!(authority.snapshot(&pending).revision, 0);
        assert_eq!(authority.snapshot(&pending).text, "");
    }
}
