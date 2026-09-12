use arut_feature_chat::composer::ComposerCheckpoint;
use arut_feature_chat::composer::authority::ComposerAuthority;
use arut_feature_chat::composer::service::{
    ComposerServiceImpl, checkpoint_from_wire, checkpoint_to_wire,
};
use arut_feature_chat::service::ChatServiceImpl;
use arut_protocol::capability::v1::CapabilityServiceRouter;
use arut_protocol::capability_manifest::CapabilityServiceImpl;
use arut_protocol::chat::composer::v1::{
    ComposerAuthorityCheckpoint as WireCheckpoint, ComposerServiceRouter,
};
use arut_protocol::chat::v1::ChatServiceRouter;
use arut_rpc::{Code, Request, RpcChannel, RpcRegistry, RpcService, Status};
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use prost::Message;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

pub fn app(data_path: PathBuf) -> Result<Router, String> {
    let authority = Arc::new(ComposerAuthority::from_checkpoint(load_checkpoint(
        &data_path,
    )?));
    let persistence_path = Arc::new(data_path);
    let composer = Arc::new(ComposerServiceImpl::with_persistence(
        Arc::clone(&authority),
        move |checkpoint| persist_checkpoint(&persistence_path, checkpoint),
    ));
    let chat = Arc::new(ChatServiceImpl::new(authority));
    let composer_router: Arc<dyn RpcService> = Arc::new(ComposerServiceRouter::new(composer));
    let chat_router: Arc<dyn RpcService> = Arc::new(ChatServiceRouter::new(chat));
    let registry = RpcRegistry::default()
        .register(composer_router)
        .expect("composer RPC routes must be unique")
        .register(chat_router)
        .expect("chat RPC routes must be unique");
    let capabilities = Arc::new(CapabilityServiceImpl::new(registry.registrations()));
    let capability_router: Arc<dyn RpcService> =
        Arc::new(CapabilityServiceRouter::new(capabilities));
    let registry = Arc::new(
        registry
            .register(capability_router)
            .expect("capability RPC routes must be unique"),
    );

    Ok(Router::new()
        .route("/health", get(|| async { StatusCode::NO_CONTENT }))
        .route("/{*procedure}", post(invoke))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers([header::CONTENT_TYPE]),
        )
        .with_state(registry))
}

async fn invoke(
    State(registry): State<Arc<RpcRegistry>>,
    AxumPath(procedure): AxumPath<String>,
    body: Bytes,
) -> ProtoResponse {
    let procedure = format!("/{procedure}");
    match registry
        .unary(&procedure, Request::new(body.to_vec()))
        .await
    {
        Ok(response) => ProtoResponse::ok(response.message),
        Err(error) => ProtoResponse::error(error),
    }
}

struct ProtoResponse {
    status: StatusCode,
    body: Vec<u8>,
}

impl ProtoResponse {
    fn ok(body: Vec<u8>) -> Self {
        Self {
            status: StatusCode::OK,
            body,
        }
    }

    fn error(error: Status) -> Self {
        Self {
            status: match error.code {
                Code::InvalidArgument => StatusCode::BAD_REQUEST,
                Code::Unauthenticated => StatusCode::UNAUTHORIZED,
                Code::PermissionDenied => StatusCode::FORBIDDEN,
                Code::NotFound | Code::Unimplemented => StatusCode::NOT_FOUND,
                Code::AlreadyExists | Code::Aborted => StatusCode::CONFLICT,
                Code::FailedPrecondition => StatusCode::PRECONDITION_FAILED,
                Code::ResourceExhausted => StatusCode::TOO_MANY_REQUESTS,
                Code::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            body: error.message.into_bytes(),
        }
    }
}

impl IntoResponse for ProtoResponse {
    fn into_response(self) -> Response {
        let mut response = (self.status, self.body).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-protobuf"),
        );
        response
    }
}

fn load_checkpoint(path: &Path) -> Result<ComposerCheckpoint, String> {
    if !path.exists() {
        return Ok(ComposerCheckpoint::default());
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    WireCheckpoint::decode(bytes.as_slice())
        .map(checkpoint_from_wire)
        .map_err(|error| error.to_string())
}

fn persist_checkpoint(path: &Path, checkpoint: &ComposerCheckpoint) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    let mut file = std::fs::File::create(&temporary).map_err(|error| error.to_string())?;
    file.write_all(&checkpoint_to_wire(checkpoint).encode_to_vec())
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    std::fs::rename(&temporary, path).map_err(|error| error.to_string())?;
    sync_parent(path)
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arut_feature_chat::composer::{ComposerScope, ReplaceComposer, ReplaceOutcome};
    use arut_protocol::capability::v1::{CapabilityServiceClient, GetCapabilitiesRequest};
    use arut_protocol::chat::composer::v1::{
        COMPOSER_SERVICE_DESCRIPTOR, ComposerServiceClient, ReplaceComposerRequest,
    };
    use arut_protocol::chat::v1::{ChatServiceClient, StartChatRequest};
    use arut_rpc::Request;
    use arut_transport_http::HttpRpcChannel;

    fn wire_scope(scope: &ComposerScope) -> arut_protocol::chat::composer::v1::ComposerScope {
        arut_feature_chat::composer::service::scope_to_wire(scope)
    }

    #[tokio::test]
    async fn generated_clients_reach_nested_composer_and_chat_over_http() {
        let path =
            std::env::temp_dir().join(format!("arut-composer-http-test-{}.pb", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server_path = path.clone();
        let server = tokio::spawn(async move {
            axum::serve(listener, app(server_path).unwrap())
                .await
                .unwrap();
        });
        let channel: Arc<dyn RpcChannel> =
            Arc::new(HttpRpcChannel::new(format!("http://{address}")));
        let composer = ComposerServiceClient::remote(channel.clone());
        let chat = ChatServiceClient::remote(channel.clone());
        let capabilities = CapabilityServiceClient::remote(channel);

        composer
            .replace_composer(Request::new(ReplaceComposerRequest {
                scope: Some(wire_scope(&ComposerScope::pending("account"))),
                command_id: "draft".into(),
                authority_epoch: 1,
                base_revision: 0,
                text: "over HTTP".into(),
            }))
            .await
            .unwrap();
        let request = StartChatRequest {
            pending_scope_id: "account".into(),
            command_id: "start".into(),
            expected_revision: 1,
            text: "over HTTP".into(),
        };
        let first = chat
            .start_chat(Request::new(request.clone()))
            .await
            .unwrap()
            .message;
        let retry = chat
            .start_chat(Request::new(request))
            .await
            .unwrap()
            .message;
        assert_eq!(retry.chat_id, first.chat_id);
        assert_eq!(retry.messages, first.messages);

        let manifest = capabilities
            .get_capabilities(Request::new(GetCapabilitiesRequest {}))
            .await
            .unwrap()
            .message
            .manifest
            .unwrap();
        assert_eq!(manifest.services.len(), 2);
        assert!(
            manifest
                .services
                .iter()
                .any(|service| service.package == COMPOSER_SERVICE_DESCRIPTOR.package)
        );
        server.abort();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn checkpoint_restores_multiple_chat_scopes_but_not_pending() {
        let path =
            std::env::temp_dir().join(format!("arut-composer-restart-{}.pb", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let authority = ComposerAuthority::default();
        for (scope, command, text) in [
            (ComposerScope::chat("one"), "one", "first"),
            (ComposerScope::chat("two"), "two", "second"),
            (ComposerScope::pending("account"), "pending", "ephemeral"),
        ] {
            authority
                .replace_durable(
                    ReplaceComposer {
                        scope,
                        command_id: command.into(),
                        authority_epoch: 1,
                        base_revision: 0,
                        text: text.into(),
                    },
                    |checkpoint| persist_checkpoint(&path, checkpoint),
                )
                .unwrap();
        }

        let restored = ComposerAuthority::from_checkpoint(load_checkpoint(&path).unwrap());
        assert_eq!(restored.snapshot(&ComposerScope::chat("one")).text, "first");
        assert_eq!(
            restored.snapshot(&ComposerScope::chat("two")).text,
            "second"
        );
        assert_eq!(
            restored.snapshot(&ComposerScope::pending("account")).text,
            ""
        );
        assert!(matches!(
            restored.replace(ReplaceComposer {
                scope: ComposerScope::chat("one"),
                command_id: "one".into(),
                authority_epoch: 1,
                base_revision: 0,
                text: "first".into(),
            }),
            ReplaceOutcome::Applied {
                duplicate: true,
                ..
            }
        ));
        let _ = std::fs::remove_file(path);
    }
}
