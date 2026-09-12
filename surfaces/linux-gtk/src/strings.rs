use arut_feature_chat::{
    composer::product::ComposerStatus,
    errors::{ChatError, ComposerError, NodeFailure},
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

/// The chat status caption; `error` supplies the sentence when `status` is `Failed`.
pub fn chat(status: ChatStatus, error: Option<ChatError>) -> String {
    match (status, error) {
        (ChatStatus::Failed, Some(error)) => chat_error(error),
        (ChatStatus::Failed, None) => "Could not send the message. Try again.".to_owned(),
        (ChatStatus::Sending, _) => "Sending…".to_owned(),
        (ChatStatus::Idle, _) => String::new(),
    }
}

/// The composer status caption; `error` supplies the sentence when `status` is `Failed`.
pub fn composer(status: ComposerStatus, error: Option<ComposerError>) -> String {
    match (status, error) {
        (ComposerStatus::Failed, Some(error)) => composer_error(error),
        (ComposerStatus::Failed, None) => {
            "Could not synchronize the draft. Check the node connection.".to_owned()
        }
        (ComposerStatus::Connecting, _) => "Connecting draft…".to_owned(),
        (ComposerStatus::Synced, _) => String::new(),
    }
}

pub fn role(role: ChatRole) -> &'static str {
    match role {
        ChatRole::User => "You",
        ChatRole::Assistant => "Arut",
    }
}

/// One sentence for every `NodeFailure` variant (ADR 0016).
pub fn node_failure(failure: NodeFailure) -> &'static str {
    match failure {
        NodeFailure::Unreachable => "The node could not be reached.",
        NodeFailure::TimedOut => "The node did not answer in time.",
        NodeFailure::Cancelled => "The request was cancelled before the node answered.",
        NodeFailure::Refused => "The node refused the request.",
        NodeFailure::Overloaded => "The node is over its limits. Try again shortly.",
        NodeFailure::Rejected => "The node could not accept this as it stands.",
        NodeFailure::Missing => "What this names is no longer on the node.",
        NodeFailure::Conflict => "Something else changed first. Try again.",
        NodeFailure::Unsupported => "The node does not support this yet.",
        NodeFailure::Internal => "The node failed to carry this out.",
    }
}

/// One sentence for every `ComposerError` variant (ADR 0016).
pub fn composer_error(error: ComposerError) -> String {
    match error {
        ComposerError::Node(failure) => node_failure(failure).to_owned(),
        ComposerError::RevisionConflict { current } => {
            format!("Someone else edited this draft first; it is now at revision {current}.")
        }
        ComposerError::AuthorityChanged { current_epoch } => format!(
            "This conversation moved to a new authority (epoch {current_epoch}). Try again."
        ),
        ComposerError::SnapshotMissing => {
            "The node answered without the draft. It may be out of sync.".to_owned()
        }
        ComposerError::OutcomeMissing => {
            "The node did not say what happened to the edit.".to_owned()
        }
        ComposerError::ScopeMissing => "The node did not say which draft it meant.".to_owned(),
        ComposerError::ScopeMismatch => "The node answered about a different draft.".to_owned(),
    }
}

/// One sentence for every `ChatError` variant (ADR 0016).
pub fn chat_error(error: ChatError) -> String {
    match error {
        ChatError::Node(failure) => node_failure(failure).to_owned(),
        ChatError::NoConversation => "There is no conversation to send this to yet.".to_owned(),
        ChatError::Cancelled => {
            "This conversation was closed before the message could send.".to_owned()
        }
        ChatError::Draft(error) => composer_error(error),
        ChatError::ChatIdMissing => {
            "The node started a conversation but did not name it.".to_owned()
        }
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
