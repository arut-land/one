use arut_feature_chat::{
    composer::product::ComposerStatus,
    product::{ChatRole, ChatStatus},
};
use arut_product_session::FeatureAvailability;
use arut_rpc::{Code, Status};

pub fn availability(value: FeatureAvailability) -> &'static str {
    match value {
        FeatureAvailability::Unknown => "Checking node availability…",
        FeatureAvailability::Available => "Ready",
        FeatureAvailability::ReportedUnavailable => "Composer is unavailable on this node.",
        FeatureAvailability::NotAdvertised => "This node does not offer a composer.",
        FeatureAvailability::ManifestUnreachable => "Could not read this node's capabilities.",
    }
}

pub fn chat(status: ChatStatus) -> &'static str {
    match status {
        ChatStatus::Idle => "",
        ChatStatus::Sending => "Sending…",
        ChatStatus::Failed => "Could not send the message. Try again.",
    }
}

pub fn composer(status: ComposerStatus) -> &'static str {
    match status {
        ComposerStatus::Connecting => "Connecting draft…",
        ComposerStatus::Synced => "",
        ComposerStatus::Failed => "Could not synchronize the draft. Check the node connection.",
    }
}

pub fn role(role: ChatRole) -> &'static str {
    match role {
        ChatRole::User => "You",
        ChatRole::Assistant => "Arut",
    }
}

pub fn rpc(error: &Status) -> &'static str {
    match error.code {
        Code::Unavailable => "The local node is unavailable.",
        Code::Cancelled => "The request was cancelled.",
        Code::InvalidArgument | Code::OutOfRange => "The node could not accept this request.",
        Code::DeadlineExceeded => "The node took too long to respond.",
        Code::NotFound => "The requested conversation was not found.",
        Code::AlreadyExists => "This item already exists on the node.",
        Code::PermissionDenied | Code::Unauthenticated => "Access to this node was denied.",
        Code::ResourceExhausted => "The node has no capacity for this request.",
        Code::FailedPrecondition | Code::Aborted => {
            "The node changed before the request completed. Try again."
        }
        Code::Unimplemented => "This node does not support the request.",
        Code::Internal => "The node encountered an internal error.",
    }
}
