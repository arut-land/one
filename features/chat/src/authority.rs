//! Serializes chat acceptance, retry identity, and durable draft promotion.
use crate::command::{ChatCommand, PendingDraft, Rejection};
use crate::composer::{ComposerAuthority, ComposerScope, PromoteError};
#[cfg(test)]
use crate::facts::ChatProjection;
use crate::ports::{Clock, IdSource};
use arut_authority::{Authority, Outcome};
use arut_protocol::chat::v1::ChatFact;
use arut_storage::{FactLog, StorageError};
use std::sync::{Arc, Mutex};
pub(crate) struct ChatAuthority {
    composer: Arc<ComposerAuthority>,
    authority: Authority<ChatCommand>,
    start_gate: Mutex<()>,
    ids: Arc<dyn IdSource>,
    clock: Arc<dyn Clock + Send + Sync>,
}
impl ChatAuthority {
    pub(crate) fn new<R: IdSource + Clock + Send + Sync>(
        composer: Arc<ComposerAuthority>,
        log: Arc<dyn FactLog<ChatFact>>,
        ids: Arc<R>,
    ) -> Result<Self, StorageError> {
        let authority = Authority::<ChatCommand>::open(log, 1)?;
        for (scope, revision) in &authority.projection().consumed_drafts {
            composer.recover_pending(scope, *revision)?;
        }
        Ok(Self {
            clock: ids.clone(),
            ids,
            composer,
            authority,
            start_gate: Mutex::new(()),
        })
    }
    #[cfg(test)]
    pub(crate) fn projection(&self) -> ChatProjection {
        self.authority.projection()
    }
    fn commit(&self, command: ChatCommand) -> Result<ChatFact, CommitError> {
        let chat_id = command.chat_id.clone();
        let pending_scope = command
            .pending_scope
            .as_ref()
            .map(|pending| pending.scope_id.clone());
        match self
            .authority
            .execute_with_clock(command, || self.clock.now())?
        {
            Outcome::Applied(record) | Outcome::Duplicate(record) => {
                if (pending_scope.is_none()
                    && (record.fact.chat_id != chat_id || !record.fact.pending_scope_id.is_empty()))
                    || pending_scope.is_some_and(|scope| record.fact.pending_scope_id != scope)
                {
                    return Err(CommitError::CommandConflict);
                }
                Ok(record.fact)
            }
            Outcome::RevisionConflict { .. } => Err(CommitError::RevisionConflict),
            Outcome::AuthorityMismatch { .. } => Err(CommitError::AuthorityMismatch),
            Outcome::Superseded => Err(CommitError::Superseded),
            Outcome::Rejected(rejection) => Err(CommitError::Rejected(rejection)),
        }
    }
}

/// Why an accepted command could not become a fact, before any surface wording.
#[derive(Debug)]
pub(crate) enum CommitError {
    Storage(StorageError),
    Rejected(Rejection),
    RevisionConflict,
    AuthorityMismatch,
    Superseded,
    CommandConflict,
    PendingRevisionConflict,
    PendingTextMismatch,
}
impl From<StorageError> for CommitError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl ChatAuthority {
    pub(crate) fn conversations(&self) -> Vec<arut_protocol::chat::v1::Conversation> {
        self.authority.read_projection(|p| p.conversations.clone())
    }
    pub(crate) fn send(
        &self,
        command_id: String,
        chat_id: String,
        text: String,
    ) -> Result<ChatFact, CommitError> {
        self.commit(ChatCommand {
            command_id,
            chat_id,
            text,
            pending_scope: None,
        })
    }
    pub(crate) fn start(
        &self,
        command_id: String,
        pending_scope_id: String,
        revision: u64,
        text: String,
    ) -> Result<(ChatFact, crate::composer::ComposerSnapshot), CommitError> {
        let _start = self.start_gate.lock().unwrap();
        let fact = if let Some(record) = self.authority.outcome_of(&command_id)? {
            if record.fact.pending_scope_id != pending_scope_id {
                return Err(CommitError::CommandConflict);
            }
            if let Some(revision) = record.fact.pending_revision {
                self.composer.recover_pending(&pending_scope_id, revision)?;
            }
            record.fact
        } else {
            let chat_id = self.ids.new_id();
            let command = ChatCommand {
                command_id,
                chat_id: chat_id.clone(),
                pending_scope: Some(PendingDraft {
                    scope_id: pending_scope_id.clone(),
                    revision,
                }),
                text: text.clone(),
            };
            match self.composer.promote_pending(
                &pending_scope_id,
                revision,
                &text,
                &chat_id,
                || self.commit(command),
            )? {
                Ok((fact, _)) => fact,
                Err(PromoteError::RevisionConflict(_)) => {
                    return Err(CommitError::PendingRevisionConflict);
                }
                Err(PromoteError::TextMismatch(_)) => return Err(CommitError::PendingTextMismatch),
            }
        };
        let snapshot = self
            .composer
            .snapshot(&ComposerScope::chat(&fact.chat_id))?;
        Ok((fact, snapshot))
    }
}
