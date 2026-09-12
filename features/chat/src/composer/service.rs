use super::authority::ComposerAuthority;
use super::{
    ComposerCheckpoint, ComposerScope, ComposerSnapshot, DraftReplaced, ReplaceComposer,
    ReplaceOutcome,
};
use arut_protocol::chat::composer::v1::{
    AppliedComposerCommand, ComposerAuthorityMismatch, ComposerCommitApplied,
    ComposerRevisionConflict, ComposerScope as WireScope,
    ComposerScopeCheckpoint as WireScopeCheckpoint, ComposerService,
    ComposerSnapshot as WireSnapshot, DraftReplaced as WireDraftReplaced, GetComposerRequest,
    GetComposerResponse, ReplaceComposerRequest, ReplaceComposerResponse, WatchComposerRequest,
    WatchComposerResponse, composer_scope, replace_composer_response,
};
use arut_rpc::{Code, Request, Response, RpcFuture, Status};
use std::sync::Arc;

type PersistCheckpoint = dyn Fn(&ComposerCheckpoint) -> Result<(), String> + Send + Sync;

pub struct ComposerServiceImpl {
    authority: Arc<ComposerAuthority>,
    persist: Arc<PersistCheckpoint>,
}

impl ComposerServiceImpl {
    pub fn new(authority: Arc<ComposerAuthority>) -> Self {
        Self {
            authority,
            persist: Arc::new(|_| Ok(())),
        }
    }

    pub fn with_persistence(
        authority: Arc<ComposerAuthority>,
        persist: impl Fn(&ComposerCheckpoint) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            authority,
            persist: Arc::new(persist),
        }
    }
}

impl ComposerService for ComposerServiceImpl {
    fn get_composer(
        &self,
        request: Request<GetComposerRequest>,
    ) -> RpcFuture<Response<GetComposerResponse>> {
        let snapshot = request
            .message
            .scope
            .and_then(scope_from_wire)
            .map(|scope| self.authority.snapshot(&scope));
        Box::pin(async move {
            let snapshot = snapshot.ok_or_else(invalid_scope)?;
            Ok(Response::new(GetComposerResponse {
                snapshot: Some(snapshot.into()),
            }))
        })
    }

    fn replace_composer(
        &self,
        request: Request<ReplaceComposerRequest>,
    ) -> RpcFuture<Response<ReplaceComposerResponse>> {
        let message = request.message;
        let Some(scope) = message.scope.and_then(scope_from_wire) else {
            return Box::pin(async { Err(invalid_scope()) });
        };
        let outcome = self.authority.replace_durable(
            ReplaceComposer {
                scope,
                command_id: message.command_id,
                authority_epoch: message.authority_epoch,
                base_revision: message.base_revision,
                text: message.text,
            },
            |checkpoint| (self.persist)(checkpoint),
        );
        Box::pin(async move {
            let outcome = outcome
                .map_err(|_| Status::new(Code::Internal, "failed to persist composer state"))?;
            Ok(Response::new(ReplaceComposerResponse {
                outcome: Some(outcome.into()),
            }))
        })
    }

    fn watch_composer(
        &self,
        request: Request<WatchComposerRequest>,
    ) -> RpcFuture<Response<WatchComposerResponse>> {
        let message = request.message;
        let Some(scope) = message.scope.and_then(scope_from_wire) else {
            return Box::pin(async { Err(invalid_scope()) });
        };
        let facts = self.authority.facts_after(&scope, message.after_revision);
        let snapshot = self.authority.snapshot(&scope);
        Box::pin(async move {
            Ok(Response::new(WatchComposerResponse {
                facts: facts.into_iter().map(Into::into).collect(),
                snapshot: Some(snapshot.into()),
            }))
        })
    }
}

fn invalid_scope() -> Status {
    Status::new(Code::InvalidArgument, "composer scope is required")
}

pub fn scope_to_wire(scope: &ComposerScope) -> WireScope {
    WireScope {
        scope_id: Some(match scope {
            ComposerScope::Pending(id) => composer_scope::ScopeId::PendingScopeId(id.clone()),
            ComposerScope::Chat(id) => composer_scope::ScopeId::ChatId(id.clone()),
        }),
    }
}

pub fn scope_from_wire(scope: WireScope) -> Option<ComposerScope> {
    match scope.scope_id? {
        composer_scope::ScopeId::PendingScopeId(id) if !id.is_empty() => {
            Some(ComposerScope::Pending(id))
        }
        composer_scope::ScopeId::ChatId(id) if !id.is_empty() => Some(ComposerScope::Chat(id)),
        _ => None,
    }
}

impl From<ComposerSnapshot> for WireSnapshot {
    fn from(snapshot: ComposerSnapshot) -> Self {
        Self {
            scope: Some(scope_to_wire(&snapshot.scope)),
            authority_epoch: snapshot.authority_epoch,
            text: snapshot.text,
            revision: snapshot.revision,
        }
    }
}

impl From<DraftReplaced> for WireDraftReplaced {
    fn from(fact: DraftReplaced) -> Self {
        Self {
            scope: Some(scope_to_wire(&fact.scope)),
            command_id: fact.command_id,
            authority_epoch: fact.authority_epoch,
            revision: fact.revision,
            text: fact.text,
        }
    }
}

impl From<ReplaceOutcome> for replace_composer_response::Outcome {
    fn from(outcome: ReplaceOutcome) -> Self {
        match outcome {
            ReplaceOutcome::Applied {
                fact,
                snapshot,
                duplicate,
            } => Self::Applied(ComposerCommitApplied {
                fact: Some(fact.into()),
                snapshot: Some(snapshot.into()),
                duplicate,
            }),
            ReplaceOutcome::RevisionConflict { snapshot } => {
                Self::RevisionConflict(ComposerRevisionConflict {
                    snapshot: Some(snapshot.into()),
                })
            }
            ReplaceOutcome::AuthorityMismatch { current_epoch } => {
                Self::AuthorityMismatch(ComposerAuthorityMismatch { current_epoch })
            }
        }
    }
}

pub fn checkpoint_to_wire(
    checkpoint: &ComposerCheckpoint,
) -> arut_protocol::chat::composer::v1::ComposerAuthorityCheckpoint {
    arut_protocol::chat::composer::v1::ComposerAuthorityCheckpoint {
        chat_scopes: checkpoint
            .chat_scopes
            .iter()
            .map(|scope| WireScopeCheckpoint {
                snapshot: Some(scope.snapshot.clone().into()),
                commands: scope
                    .commands
                    .iter()
                    .map(|applied| AppliedComposerCommand {
                        command: Some(ReplaceComposerRequest {
                            scope: Some(scope_to_wire(&applied.command.scope)),
                            command_id: applied.command.command_id.clone(),
                            authority_epoch: applied.command.authority_epoch,
                            base_revision: applied.command.base_revision,
                            text: applied.command.text.clone(),
                        }),
                        fact: Some(applied.fact.clone().into()),
                    })
                    .collect(),
                facts: scope.facts.clone().into_iter().map(Into::into).collect(),
            })
            .collect(),
    }
}

pub fn checkpoint_from_wire(
    checkpoint: arut_protocol::chat::composer::v1::ComposerAuthorityCheckpoint,
) -> ComposerCheckpoint {
    ComposerCheckpoint {
        chat_scopes: checkpoint
            .chat_scopes
            .into_iter()
            .filter_map(|scope| {
                let snapshot = snapshot_from_wire(scope.snapshot?)?;
                if !snapshot.scope.is_durable() {
                    return None;
                }
                Some(super::ComposerScopeCheckpoint {
                    snapshot,
                    commands: scope
                        .commands
                        .into_iter()
                        .filter_map(|applied| {
                            Some(super::AppliedCommand {
                                command: command_from_wire(applied.command?)?,
                                fact: fact_from_wire(applied.fact?)?,
                            })
                        })
                        .collect(),
                    facts: scope.facts.into_iter().filter_map(fact_from_wire).collect(),
                })
            })
            .collect(),
    }
}

fn snapshot_from_wire(snapshot: WireSnapshot) -> Option<ComposerSnapshot> {
    Some(ComposerSnapshot {
        scope: scope_from_wire(snapshot.scope?)?,
        authority_epoch: snapshot.authority_epoch,
        text: snapshot.text,
        revision: snapshot.revision,
    })
}

fn command_from_wire(command: ReplaceComposerRequest) -> Option<ReplaceComposer> {
    Some(ReplaceComposer {
        scope: scope_from_wire(command.scope?)?,
        command_id: command.command_id,
        authority_epoch: command.authority_epoch,
        base_revision: command.base_revision,
        text: command.text,
    })
}

fn fact_from_wire(fact: WireDraftReplaced) -> Option<DraftReplaced> {
    Some(DraftReplaced {
        scope: scope_from_wire(fact.scope?)?,
        command_id: fact.command_id,
        authority_epoch: fact.authority_epoch,
        revision: fact.revision,
        text: fact.text,
    })
}
