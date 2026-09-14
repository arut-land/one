//! Serializes chat acceptance, retry identity, and durable draft promotion.
use crate::ConversationTitle;
use crate::command::{ChatCommand, CommandId, PendingDraft, Rejection};
use crate::composer::{ComposerAuthority, ComposerScope, PromoteError};
#[cfg(test)]
use crate::facts::ChatProjection;
use crate::ports::{Clock, IdSource};
use arut_authority::{Authority, Outcome};
use arut_protocol::chat::v1::{ChatFact, chat_fact};
use arut_storage::{FactLog, StorageError};
use std::sync::{Arc, Mutex};

/// Why an accepted command could not become a fact, before any surface wording.
#[derive(Debug)]
pub(crate) enum CommitError {
    Storage(StorageError),
    RevisionConflict,
    AuthorityChanged,
    Superseded,
    Rejected(Rejection),
}
impl From<StorageError> for CommitError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}
impl From<Rejection> for CommitError {
    fn from(rejection: Rejection) -> Self {
        Self::Rejected(rejection)
    }
}

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
        authority.read_projection(|projection| {
            projection
                .consumed_drafts
                .iter()
                .try_for_each(|(scope, revision)| composer.recover_pending(scope, *revision))
        })?;
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
        let expected = command.clone();
        match self
            .authority
            .execute_with_clock(command, || self.clock.now())?
        {
            Outcome::Applied(record) | Outcome::Duplicate(record) => {
                if !expected.matches(&record.fact) {
                    return Err(Rejection::CommandConflict.into());
                }
                Ok(record.fact)
            }
            Outcome::RevisionConflict { .. } => Err(CommitError::RevisionConflict),
            Outcome::AuthorityMismatch { .. } => Err(CommitError::AuthorityChanged),
            Outcome::Superseded => Err(CommitError::Superseded),
            Outcome::Rejected(rejection) => Err(CommitError::Rejected(rejection)),
        }
    }
    pub(crate) fn conversations(&self) -> Vec<arut_protocol::chat::v1::Conversation> {
        self.authority.read_projection(|p| p.conversations.clone())
    }
    pub(crate) fn send(
        &self,
        command_id: CommandId,
        chat_id: String,
        text: String,
    ) -> Result<ChatFact, CommitError> {
        self.commit(ChatCommand::Send {
            command_id,
            chat_id,
            text,
        })
    }
    pub(crate) fn rename(
        &self,
        command_id: CommandId,
        chat_id: String,
        title: ConversationTitle,
    ) -> Result<ChatFact, CommitError> {
        self.commit(ChatCommand::Rename {
            command_id,
            chat_id,
            title,
        })
    }
    pub(crate) fn delete(
        &self,
        command_id: CommandId,
        chat_id: String,
    ) -> Result<ChatFact, CommitError> {
        self.commit(ChatCommand::Delete {
            command_id,
            chat_id,
        })
    }
    pub(crate) fn start(
        &self,
        command_id: CommandId,
        pending_scope_id: String,
        revision: u64,
        text: String,
    ) -> Result<(ChatFact, crate::composer::ComposerSnapshot), CommitError> {
        let _start = self.start_gate.lock().map_err(|_| StorageError::Corrupt)?;
        let fact = if let Some(record) = self.authority.outcome_of(command_id.as_str())? {
            let expected = ChatCommand::Start {
                command_id,
                chat_id: record.fact.chat_id.clone(),
                pending: PendingDraft {
                    scope_id: pending_scope_id.clone(),
                    revision,
                },
                text,
            };
            if !expected.matches(&record.fact) {
                return Err(Rejection::CommandConflict.into());
            }
            let Some(chat_fact::Change::Started(started)) = &record.fact.change else {
                return Err(Rejection::CommandConflict.into());
            };
            self.composer
                .recover_pending(&pending_scope_id, started.pending_revision)?;
            record.fact
        } else {
            let chat_id = self.ids.new_id();
            let command = ChatCommand::Start {
                command_id,
                chat_id: chat_id.clone(),
                pending: PendingDraft {
                    scope_id: pending_scope_id.clone(),
                    revision,
                },
                text: text.clone(),
            };
            match self.composer.promote_pending(
                &pending_scope_id,
                revision,
                &text,
                &chat_id,
                || self.commit(command),
            )? {
                Ok(fact) => fact,
                Err(PromoteError::RevisionConflict) => {
                    return Err(Rejection::PendingRevisionConflict.into());
                }
                Err(PromoteError::TextMismatch) => {
                    return Err(Rejection::PendingTextMismatch.into());
                }
            }
        };
        let snapshot = self
            .composer
            .snapshot(&ComposerScope::chat(&fact.chat_id))?;
        Ok((fact, snapshot))
    }
}
