//! Derives capability clients and routers from composed service registrations.
use crate::capability::v1::{
    CapabilityManifest, CapabilityService, GetCapabilitiesRequest, GetCapabilitiesResponse,
    MethodCapability, RpcStreamingKind, ServiceCapability,
};
use arut_rpc::{Request, Response, RpcFuture, ServiceRegistration, StreamingKind};

pub struct CapabilityServiceImpl {
    registrations: Vec<ServiceRegistration>,
}

impl CapabilityServiceImpl {
    pub fn new(registrations: impl IntoIterator<Item = ServiceRegistration>) -> Self {
        Self {
            registrations: registrations.into_iter().collect(),
        }
    }

    pub fn manifest(&self) -> CapabilityManifest {
        let mut services = self
            .registrations
            .iter()
            .map(|registration| {
                let metadata = registration.metadata.get();
                ServiceCapability {
                    package: registration.descriptor.package.into(),
                    service: registration.descriptor.name.into(),
                    methods: registration
                        .descriptor
                        .methods
                        .iter()
                        .map(|method| MethodCapability {
                            name: method.name.into(),
                            procedure: method.procedure.into(),
                            input: method.input.into(),
                            output: method.output.into(),
                            streaming: streaming_kind(method.streaming) as i32,
                        })
                        .collect(),
                    available: metadata.available,
                    unavailable_reason: metadata.unavailable_reason,
                    permissions: metadata.permissions,
                    limits: metadata.limits.into_iter().collect(),
                    version: registration.descriptor.version.into(),
                    extensions: metadata.extensions.into_iter().collect(),
                }
            })
            .collect::<Vec<_>>();
        services.sort_by(|left, right| {
            (&left.package, &left.service).cmp(&(&right.package, &right.service))
        });
        CapabilityManifest { services }
    }
}

impl CapabilityService for CapabilityServiceImpl {
    fn get_capabilities(
        &self,
        _request: Request<GetCapabilitiesRequest>,
    ) -> RpcFuture<Response<GetCapabilitiesResponse>> {
        let manifest = self.manifest();
        Box::pin(async move {
            Ok(Response::new(GetCapabilitiesResponse {
                manifest: Some(manifest),
            }))
        })
    }
}

fn streaming_kind(kind: StreamingKind) -> RpcStreamingKind {
    match kind {
        StreamingKind::Unary => RpcStreamingKind::Unary,
        StreamingKind::Server => RpcStreamingKind::Server,
        StreamingKind::Client => RpcStreamingKind::Client,
        StreamingKind::Bidirectional => RpcStreamingKind::Bidirectional,
    }
}

/// Direct manifest client for a root's composed feature list.
pub fn capability_client(
    registrations: impl IntoIterator<Item = ServiceRegistration>,
) -> crate::capability::v1::CapabilityServiceClient {
    crate::capability::v1::CapabilityServiceClient::direct(std::sync::Arc::new(
        CapabilityServiceImpl::new(registrations),
    ))
}

/// Adds the manifest service after the root's feature routers have been registered.
pub fn with_capabilities(
    registry: arut_rpc::RpcRegistry,
) -> Result<arut_rpc::RpcRegistry, arut_rpc::Status> {
    let service = std::sync::Arc::new(CapabilityServiceImpl::new(registry.registrations()));
    registry.register(std::sync::Arc::new(
        crate::capability::v1::CapabilityServiceRouter::new(service),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::v1::{CHAT_SERVICE_DESCRIPTOR, ChatServiceRouter};
    use arut_rpc::{RpcRegistry, ServiceMetadata, ServiceRuntimeMetadata};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    struct UnusedChat;

    impl crate::chat::v1::ChatService for UnusedChat {
        fn list_conversations(
            &self,
            _: Request<crate::chat::v1::ListConversationsRequest>,
        ) -> RpcFuture<Response<crate::chat::v1::ListConversationsResponse>> {
            unreachable!()
        }

        fn send_message(
            &self,
            _request: Request<crate::chat::v1::SendMessageRequest>,
        ) -> RpcFuture<Response<crate::chat::v1::SendMessageResponse>> {
            unreachable!()
        }

        fn start_chat(
            &self,
            _request: Request<crate::chat::v1::StartChatRequest>,
        ) -> RpcFuture<Response<crate::chat::v1::StartChatResponse>> {
            unreachable!()
        }
    }

    #[test]
    fn derives_protocol_shape_and_reads_current_runtime_metadata() {
        let metadata = ServiceMetadata::new(ServiceRuntimeMetadata {
            permissions: vec!["conversation.read".into()],
            limits: BTreeMap::from([("requests_per_minute".into(), 60)]),
            ..ServiceRuntimeMetadata::default()
        });
        let registry = RpcRegistry::default()
            .register_with_metadata(
                Arc::new(ChatServiceRouter::new(Arc::new(UnusedChat))),
                metadata.clone(),
            )
            .unwrap();
        let service = CapabilityServiceImpl::new(registry.registrations());

        let manifest = service.manifest();
        assert_eq!(
            manifest.services[0].package,
            CHAT_SERVICE_DESCRIPTOR.package
        );
        assert_eq!(
            manifest.services[0].methods[0].procedure,
            CHAT_SERVICE_DESCRIPTOR.methods[0].procedure
        );
        assert_eq!(manifest.services[0].limits["requests_per_minute"], 60);
        assert_eq!(manifest.services[0].version, "v1");

        metadata.set(ServiceRuntimeMetadata {
            available: false,
            unavailable_reason: "runtime suspended".into(),
            ..ServiceRuntimeMetadata::default()
        });
        let unavailable = service.manifest();
        assert!(!unavailable.services[0].available);
        assert_eq!(
            unavailable.services[0].unavailable_reason,
            "runtime suspended"
        );
    }
}
