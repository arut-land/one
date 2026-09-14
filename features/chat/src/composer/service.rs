//! Generated composer service implementation; converts calls to authority operations.
use super::ReplaceComposer;
use super::authority::ComposerAuthority;
use arut_protocol::chat::composer::v1::{
    ComposerService, GetComposerRequest, GetComposerResponse, ReplaceComposerRequest,
    ReplaceComposerResponse, WatchComposerRequest, WatchComposerResponse,
};
use arut_rpc::{Code, Request, Response, RpcFuture, RpcStream, Status};
use std::sync::Arc;

pub(crate) struct ComposerServiceImpl {
    authority: Arc<ComposerAuthority>,
}
impl ComposerServiceImpl {
    pub fn new(authority: Arc<ComposerAuthority>) -> Self {
        Self { authority }
    }
}

/// A generated method's answer, already decided, as the future it must return.
fn ready<T: Send + 'static>(result: Result<Response<T>, Status>) -> RpcFuture<Response<T>> {
    Box::pin(std::future::ready(result))
}

impl ComposerService for ComposerServiceImpl {
    fn get_composer(
        &self,
        request: Request<GetComposerRequest>,
    ) -> RpcFuture<Response<GetComposerResponse>> {
        ready((|| {
            let scope = request.message.scope.unwrap_or_default().try_into()?;
            let snapshot = self.authority.snapshot(&scope).map_err(storage)?;
            Ok(Response::new(GetComposerResponse {
                snapshot: Some(snapshot.into()),
            }))
        })())
    }

    fn replace_composer(
        &self,
        request: Request<ReplaceComposerRequest>,
    ) -> RpcFuture<Response<ReplaceComposerResponse>> {
        let message = request.message;
        ready((|| {
            let outcome = self
                .authority
                .replace(ReplaceComposer {
                    scope: message.scope.unwrap_or_default().try_into()?,
                    command_id: message.command_id,
                    authority_epoch: message.authority_epoch,
                    base_revision: message.base_revision,
                    text: message.text,
                })
                .map_err(|_| Status::new(Code::Internal, "failed to persist composer state"))?;
            Ok(Response::new(ReplaceComposerResponse {
                outcome: Some(outcome.into()),
            }))
        })())
    }

    fn watch_composer(
        &self,
        request: Request<WatchComposerRequest>,
    ) -> RpcFuture<Response<RpcStream<WatchComposerResponse>>> {
        let after_revision = request.message.after_revision;
        // The scope ID is a draft's address and can carry a person's own words,
        // so the span names the stream and the cursor and nothing else.
        tracing::debug!(
            stream = "composer",
            after_revision,
            "serving a composer stream from a cursor"
        );
        ready((|| {
            let scope = request.message.scope.unwrap_or_default().try_into()?;
            let changes = self.authority.changes(&scope).map_err(storage)?;
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
        })())
    }
}

fn storage(_: arut_storage::StorageError) -> Status {
    Status::new(Code::Internal, "failed to read composer state")
}
