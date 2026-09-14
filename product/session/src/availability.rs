use arut_protocol::capability::v1::{CapabilityServiceClient, GetCapabilitiesRequest};
use arut_protocol::capability_manifest::ServiceAvailability;
use arut_protocol::chat::composer::v1::ComposerServiceId;
use arut_rpc::Request;

#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionAvailability {
    pub composer: FeatureAvailability,
}

/// Availability reasons; surfaces supply display text.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureAvailability {
    /// No manifest has been read yet.
    Unknown,
    Available,
    /// The node advertises the service but reports it as unavailable now.
    ReportedUnavailable,
    /// The node's manifest does not carry the service.
    NotAdvertised,
    /// The manifest could not be read from the node.
    ManifestUnreachable,
}

impl From<ServiceAvailability> for FeatureAvailability {
    fn from(availability: ServiceAvailability) -> Self {
        match availability {
            ServiceAvailability::Available => Self::Available,
            ServiceAvailability::ReportedUnavailable => Self::ReportedUnavailable,
            ServiceAvailability::NotAdvertised => Self::NotAdvertised,
        }
    }
}

pub(super) async fn read(service: &CapabilityServiceClient) -> SessionAvailability {
    let composer = match service
        .get_capabilities(Request::new(GetCapabilitiesRequest {}))
        .await
    {
        Ok(response) => response
            .message
            .manifest
            .map_or(FeatureAvailability::NotAdvertised, |manifest| {
                manifest.availability::<ComposerServiceId>().into()
            }),
        Err(_) => FeatureAvailability::ManifestUnreachable,
    };
    SessionAvailability { composer }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arut_protocol::capability::v1::{
        CapabilityManifest, CapabilityService, GetCapabilitiesResponse, ServiceCapability,
    };
    use arut_protocol::chat::composer::v1::COMPOSER_SERVICE_DESCRIPTOR;
    use arut_rpc::{Response, RpcFuture, Status};
    use futures_executor::block_on;
    use std::sync::Arc;

    struct Manifest(Result<GetCapabilitiesResponse, Status>);

    impl CapabilityService for Manifest {
        fn get_capabilities(
            &self,
            _: Request<GetCapabilitiesRequest>,
        ) -> RpcFuture<Response<GetCapabilitiesResponse>> {
            let response = self.0.clone();
            Box::pin(async move { response.map(Response::new) })
        }
    }

    #[test]
    fn availability_distinguishes_missing_unavailable_and_unreachable_services() {
        let composer = ServiceCapability {
            package: COMPOSER_SERVICE_DESCRIPTOR.package.into(),
            service: COMPOSER_SERVICE_DESCRIPTOR.name.into(),
            available: true,
            ..Default::default()
        };
        for (services, expected) in [
            (vec![], FeatureAvailability::NotAdvertised),
            (vec![composer.clone()], FeatureAvailability::Available),
            (
                vec![ServiceCapability {
                    available: false,
                    ..composer.clone()
                }],
                FeatureAvailability::ReportedUnavailable,
            ),
            (
                vec![ServiceCapability {
                    package: "unrelated".into(),
                    ..composer
                }],
                FeatureAvailability::NotAdvertised,
            ),
        ] {
            let service =
                CapabilityServiceClient::direct(Arc::new(Manifest(Ok(GetCapabilitiesResponse {
                    manifest: Some(CapabilityManifest { services }),
                }))));
            assert_eq!(block_on(read(&service)).composer, expected);
        }
        for (response, expected) in [
            (
                Ok(GetCapabilitiesResponse { manifest: None }),
                FeatureAvailability::NotAdvertised,
            ),
            (
                Err(Status::new(arut_rpc::Code::Unavailable, "offline")),
                FeatureAvailability::ManifestUnreachable,
            ),
        ] {
            let service = CapabilityServiceClient::direct(Arc::new(Manifest(response)));
            assert_eq!(block_on(read(&service)).composer, expected);
        }
    }
}
