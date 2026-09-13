//! Generated composer service implementation; converts calls to authority operations.
use super::ReplaceComposer;
use super::authority::ComposerAuthority;
use super::wire::scope_from_wire;
use arut_protocol::chat::composer::v1::{
    ComposerService, GetComposerRequest, GetComposerResponse, ReplaceComposerRequest,
    ReplaceComposerResponse, WatchComposerRequest, WatchComposerResponse,
};
use arut_rpc::{Code, Request, Response, RpcFuture, RpcStream, Status};
use std::sync::Arc;

pub(crate) struct ComposerServiceImpl {
    authority: Arc<ComposerAuthority>,
    runtime: Option<Arc<dyn Send + Sync>>,
}
impl ComposerServiceImpl {
    pub fn new(authority: Arc<ComposerAuthority>) -> Self {
        Self {
            authority,
            runtime: None,
        }
    }
    pub(crate) fn with_runtime(mut self, runtime: Arc<dyn Send + Sync>) -> Self {
        self.runtime = Some(runtime);
        self
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
            let snapshot = snapshot.ok_or_else(invalid_scope)?.map_err(storage)?;
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
        let outcome = self.authority.replace(ReplaceComposer {
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
        let after_revision = request.message.after_revision;
        let Some(scope) = request.message.scope.and_then(scope_from_wire) else {
            return Box::pin(async { Err(invalid_scope()) });
        };
        // The scope ID is a draft's address and can carry a person's own words,
        // so the span names the stream and the cursor and nothing else.
        tracing::debug!(
            stream = "composer",
            after_revision,
            "serving a composer stream from a cursor"
        );
        let changes = self.authority.changes(&scope);
        Box::pin(async move {
            let changes = changes.map_err(storage)?;
            let stream = futures_util::stream::unfold(changes, |changes| async move {
                let snapshot = changes.changed().await?;
                Some((
                    Ok(WatchComposerResponse {
                        snapshot: Some(snapshot.into()),
                    }),
                    changes,
                ))
            });
            Ok(Response::new(
                Box::pin(stream) as RpcStream<WatchComposerResponse>
            ))
        })
    }
}

fn storage(_: arut_storage::StorageError) -> Status {
    Status::new(Code::Internal, "failed to read composer state")
}

fn invalid_scope() -> Status {
    Status::new(Code::InvalidArgument, "composer scope is required")
}
