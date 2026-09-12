use super::authority::ComposerAuthority;
use super::{ComposerScope, ComposerSnapshot, ReplaceComposer, ReplaceOutcome};
use arut_protocol::chat::composer::v1::{
    ComposerAuthorityMismatch, ComposerCommitApplied, ComposerRevisionConflict,
    ComposerScope as WireScope, ComposerService, ComposerSnapshot as WireSnapshot,
    GetComposerRequest, GetComposerResponse, ReplaceComposerRequest, ReplaceComposerResponse,
    WatchComposerRequest, WatchComposerResponse, composer_scope, replace_composer_response,
};
use arut_rpc::{Code, Request, Response, RpcFuture, RpcStream, Status};
use std::sync::Arc;

pub struct ComposerServiceImpl {
    authority: Arc<ComposerAuthority>,
}
impl ComposerServiceImpl {
    pub fn new(authority: Arc<ComposerAuthority>) -> Self {
        Self { authority }
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
        let outcome = self.authority.try_replace(ReplaceComposer {
            scope,
            command_id: message.command_id,
            authority_epoch: message.authority_epoch,
            base_revision: message.base_revision,
            text: message.text,
        });
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
    ) -> RpcFuture<Response<RpcStream<WatchComposerResponse>>> {
        let Some(scope) = request.message.scope.and_then(scope_from_wire) else {
            return Box::pin(async { Err(invalid_scope()) });
        };
        let authority = self.authority.clone();
        let changes = authority.changes(&scope);
        Box::pin(async move {
            let stream = futures_util::stream::unfold(
                (authority, changes, scope),
                |(authority, changes, scope)| async move {
                    changes.changed().await?;
                    let snapshot = authority.snapshot(&scope);
                    Some((
                        Ok(WatchComposerResponse {
                            snapshot: Some(snapshot.into()),
                        }),
                        (authority, changes, scope),
                    ))
                },
            );
            Ok(Response::new(
                Box::pin(stream) as RpcStream<WatchComposerResponse>
            ))
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

impl From<ReplaceOutcome> for replace_composer_response::Outcome {
    fn from(outcome: ReplaceOutcome) -> Self {
        match outcome {
            ReplaceOutcome::Applied {
                snapshot,
                duplicate,
            } => Self::Applied(ComposerCommitApplied {
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
