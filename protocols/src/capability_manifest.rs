//! Derives the capability manifest from the services a node serves.
//!
//! The set is a type ([`arut_rpc::ServiceSet`]), so the manifest is built from
//! generated descriptors with no registration table to walk, no order to sort
//! (the tuple is the order), and no package name written by hand anywhere.
use crate::capability::v1::{
    CapabilityManifest, CapabilityService, CapabilityServiceClient, CapabilityServiceRouter,
    GetCapabilitiesRequest, GetCapabilitiesResponse, MethodCapability, RpcStreamingKind,
    ServiceCapability,
};
use arut_rpc::{
    Request, Response, RpcFuture, RpcRegistry, Service, ServiceDescriptor, ServiceSet, Status,
    StreamingKind,
};
use std::sync::Arc;

/// What a manifest says about one service.
///
/// The comparison behind it is still `==` on the strings the wire carries; what
/// the type buys is that the product writes none of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceAvailability {
    Available,
    /// Advertised, and reported as not usable right now.
    ReportedUnavailable,
    /// Absent from the manifest.
    NotAdvertised,
}

impl CapabilityManifest {
    /// What this manifest says about the service `S` names.
    #[must_use]
    pub fn availability<S: Service>(&self) -> ServiceAvailability {
        self.services
            .iter()
            .find(|service| {
                service.package == S::DESCRIPTOR.package && service.service == S::DESCRIPTOR.name
            })
            .map_or(ServiceAvailability::NotAdvertised, |service| {
                if service.available {
                    ServiceAvailability::Available
                } else {
                    ServiceAvailability::ReportedUnavailable
                }
            })
    }
}

/// The manifest a node serving `S` advertises.
#[must_use]
pub fn manifest<S: ServiceSet>() -> CapabilityManifest {
    CapabilityManifest {
        services: S::DESCRIPTORS.iter().map(advertise).collect(),
    }
}

/// The wire `version` is the Protobuf package version, which is the major one;
/// the minor of ADR 0015's window has no source yet and no field to go in.
fn advertise(descriptor: &ServiceDescriptor) -> ServiceCapability {
    ServiceCapability {
        package: descriptor.package.into(),
        service: descriptor.name.into(),
        methods: descriptor
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
        available: true,
        version: format!("v{}", descriptor.version.major),
        ..ServiceCapability::default()
    }
}

struct CapabilityServiceImpl {
    manifest: CapabilityManifest,
}

impl CapabilityService for CapabilityServiceImpl {
    fn get_capabilities(
        &self,
        _request: Request<GetCapabilitiesRequest>,
    ) -> RpcFuture<Response<GetCapabilitiesResponse>> {
        let manifest = self.manifest.clone();
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

fn service<S: ServiceSet>() -> Arc<CapabilityServiceImpl> {
    Arc::new(CapabilityServiceImpl {
        manifest: manifest::<S>(),
    })
}

/// Direct manifest client for the service set a root composed.
#[must_use]
pub fn capability_client<S: ServiceSet>() -> CapabilityServiceClient {
    CapabilityServiceClient::direct(service::<S>())
}

/// Adds the manifest service for `S` to a root's registry.
///
/// # Errors
/// Returns `AlreadyExists` if the manifest procedure is already registered.
pub fn with_capabilities<S: ServiceSet>(registry: RpcRegistry) -> Result<RpcRegistry, Status> {
    registry.register(Arc::new(CapabilityServiceRouter::new(service::<S>())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::composer::v1::ComposerServiceId;
    use crate::chat::v1::{CHAT_SERVICE_DESCRIPTOR, ChatServiceId};

    #[test]
    fn a_manifest_derives_its_shape_from_the_declared_service_set() {
        let manifest = manifest::<(ChatServiceId, ComposerServiceId)>();

        assert_eq!(
            manifest.services[0].package,
            CHAT_SERVICE_DESCRIPTOR.package
        );
        assert_eq!(
            manifest.services[0].methods[0].procedure,
            CHAT_SERVICE_DESCRIPTOR.methods[0].procedure
        );
        assert_eq!(manifest.services[0].version, "v1");
        assert_eq!(
            manifest.availability::<ChatServiceId>(),
            ServiceAvailability::Available
        );
    }

    #[test]
    fn a_service_missing_from_a_manifest_is_not_advertised() {
        assert_eq!(
            manifest::<(ChatServiceId,)>().availability::<ComposerServiceId>(),
            ServiceAvailability::NotAdvertised
        );
    }
}
