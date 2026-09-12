//! Every sentence this surface shows, as a generated [`Message`].
//!
//! The strings themselves live once in `product/i18n` (ADR 0022) and no message
//! id is written here: `Message` is generated from the `.ftl` source, so naming
//! a string that does not exist, or passing the wrong payload, does not
//! compile. Each `match` below is exhaustive for the same reason -- a new
//! variant of a core enum stops this surface building until it says what to
//! show for it.
//!
//! GTK gives us the person's language list, so the negotiation happens here
//! rather than in the core, which never learns the locale.

use arut_feature_chat::{
    composer::product::ComposerStatus,
    errors::{ChatError, ComposerError, NodeFailure},
    product::{ChatRole, ChatStatus},
};
use arut_i18n::{Localizer, Message};
use arut_product_session::FeatureAvailability;
use arut_rpc::{Code, Status};
use gtk::glib;
use std::sync::OnceLock;

/// The process-wide localizer, negotiated once from GTK's own language list.
///
/// `glib::language_names()` is the list GLib already resolved from `LANGUAGE`,
/// `LC_MESSAGES` and the rest, best first and with the `C` locale last;
/// `Localizer` drops the entries that are not languages.
fn localizer() -> &'static Localizer {
    static LOCALIZER: OnceLock<Localizer> = OnceLock::new();
    LOCALIZER.get_or_init(|| {
        let preferred: Vec<String> = glib::language_names()
            .iter()
            .map(ToString::to_string)
            .collect();
        Localizer::negotiate(&preferred)
    })
}

pub fn show(message: &Message) -> String {
    localizer().format(message)
}

pub fn availability(value: FeatureAvailability) -> String {
    show(&match value {
        FeatureAvailability::Unknown => Message::AvailabilityUnknown,
        FeatureAvailability::Available => Message::AvailabilityAvailable,
        FeatureAvailability::ReportedUnavailable => Message::AvailabilityReportedUnavailable,
        FeatureAvailability::NotAdvertised => Message::AvailabilityNotAdvertised,
        FeatureAvailability::ManifestUnreachable => Message::AvailabilityManifestUnreachable,
    })
}

/// The chat status caption; `error` supplies the sentence when `status` is `Failed`.
pub fn chat(status: ChatStatus, error: Option<ChatError>) -> String {
    match (status, error) {
        (ChatStatus::Failed, Some(error)) => chat_error(error),
        (ChatStatus::Failed, None) => show(&Message::ChatStatusFailed),
        (ChatStatus::Sending, _) => show(&Message::ChatStatusSending),
        (ChatStatus::Idle, _) => String::new(),
    }
}

/// The composer status caption; `error` supplies the sentence when `status` is `Failed`.
pub fn composer(status: ComposerStatus, error: Option<ComposerError>) -> String {
    match (status, error) {
        (ComposerStatus::Failed, Some(error)) => composer_error(error),
        (ComposerStatus::Failed, None) => show(&Message::ComposerStatusFailed),
        (ComposerStatus::Connecting, _) => show(&Message::ComposerStatusConnecting),
        (ComposerStatus::Synced, _) => String::new(),
    }
}

pub fn role(role: ChatRole) -> String {
    show(&match role {
        ChatRole::User => Message::ChatRoleYou,
        ChatRole::Assistant => Message::ChatRoleAssistant,
    })
}

/// One sentence for every `NodeFailure` variant (ADR 0016, ADR 0022).
pub fn node_failure(failure: NodeFailure) -> String {
    show(&match failure {
        NodeFailure::Unreachable => Message::NodeFailureUnreachable,
        NodeFailure::TimedOut => Message::NodeFailureTimedOut,
        NodeFailure::Cancelled => Message::NodeFailureCancelled,
        NodeFailure::Refused => Message::NodeFailureRefused,
        NodeFailure::Overloaded => Message::NodeFailureOverloaded,
        NodeFailure::Rejected => Message::NodeFailureRejected,
        NodeFailure::Missing => Message::NodeFailureMissing,
        NodeFailure::Conflict => Message::NodeFailureConflict,
        NodeFailure::Unsupported => Message::NodeFailureUnsupported,
        NodeFailure::Internal => Message::NodeFailureInternal,
    })
}

/// One sentence for every `ComposerError` variant, with its payload where the
/// message asks for one.
pub fn composer_error(error: ComposerError) -> String {
    match error {
        ComposerError::Node(failure) => node_failure(failure),
        ComposerError::RevisionConflict { current } => {
            show(&Message::ComposerErrorRevisionConflict {
                current: current.to_string(),
            })
        }
        ComposerError::AuthorityChanged { current_epoch } => {
            show(&Message::ComposerErrorAuthorityChanged {
                current_epoch: current_epoch.to_string(),
            })
        }
        ComposerError::SnapshotMissing => show(&Message::ComposerErrorSnapshotMissing),
        ComposerError::OutcomeMissing => show(&Message::ComposerErrorOutcomeMissing),
        ComposerError::ScopeMissing => show(&Message::ComposerErrorScopeMissing),
        ComposerError::ScopeMismatch => show(&Message::ComposerErrorScopeMismatch),
    }
}

/// One sentence for every `ChatError` variant.
pub fn chat_error(error: ChatError) -> String {
    match error {
        ChatError::Node(failure) => node_failure(failure),
        ChatError::Draft(error) => composer_error(error),
        ChatError::NoConversation => show(&Message::ChatErrorNoConversation),
        ChatError::Cancelled => show(&Message::ChatErrorCancelled),
        ChatError::ChatIdMissing => show(&Message::ChatErrorChatIdMissing),
    }
}

/// The connect-time transport statuses, which happen before any scope exists to
/// carry a typed failure.
pub fn rpc(error: &Status) -> String {
    show(&match error.code {
        Code::Unavailable => Message::RpcErrorUnavailable,
        Code::Cancelled => Message::RpcErrorCancelled,
        Code::InvalidArgument | Code::OutOfRange => Message::RpcErrorRejected,
        Code::DeadlineExceeded => Message::RpcErrorTimedOut,
        Code::NotFound => Message::RpcErrorNotFound,
        Code::AlreadyExists => Message::RpcErrorAlreadyExists,
        Code::PermissionDenied | Code::Unauthenticated => Message::RpcErrorDenied,
        Code::ResourceExhausted => Message::RpcErrorExhausted,
        Code::FailedPrecondition | Code::Aborted => Message::RpcErrorChanged,
        Code::Unimplemented => Message::RpcErrorUnsupported,
        Code::Internal => Message::RpcErrorInternal,
    })
}
