//! Every sentence this surface shows, looked up in the shared Fluent source.
//!
//! The strings themselves live once in `product/i18n` (ADR 0022); this module
//! is the mapping from a typed core value to the message id that describes it,
//! which for the error enums is `message_key` on the enum itself. GTK gives us
//! the person's language list, so the negotiation happens here rather than in
//! the core, which never learns the locale.

use arut_feature_chat::{
    composer::product::ComposerStatus,
    errors::{ChatError, ComposerError, NodeFailure},
    product::{ChatRole, ChatStatus},
};
use arut_i18n::Localizer;
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

fn message(key: &str) -> String {
    localizer().message(key)
}

pub fn availability(value: FeatureAvailability) -> String {
    message(match value {
        FeatureAvailability::Unknown => "availability-unknown",
        FeatureAvailability::Available => "availability-available",
        FeatureAvailability::ReportedUnavailable => "availability-reported-unavailable",
        FeatureAvailability::NotAdvertised => "availability-not-advertised",
        FeatureAvailability::ManifestUnreachable => "availability-manifest-unreachable",
    })
}

/// The chat status caption; `error` supplies the sentence when `status` is `Failed`.
pub fn chat(status: ChatStatus, error: Option<ChatError>) -> String {
    match (status, error) {
        (ChatStatus::Failed, Some(error)) => chat_error(error),
        (ChatStatus::Failed, None) => message("chat-status-failed"),
        (ChatStatus::Sending, _) => message("chat-status-sending"),
        (ChatStatus::Idle, _) => String::new(),
    }
}

/// The composer status caption; `error` supplies the sentence when `status` is `Failed`.
pub fn composer(status: ComposerStatus, error: Option<ComposerError>) -> String {
    match (status, error) {
        (ComposerStatus::Failed, Some(error)) => composer_error(error),
        (ComposerStatus::Failed, None) => message("composer-status-failed"),
        (ComposerStatus::Connecting, _) => message("composer-status-connecting"),
        (ComposerStatus::Synced, _) => String::new(),
    }
}

pub fn role(role: ChatRole) -> String {
    message(match role {
        ChatRole::User => "chat-role-you",
        ChatRole::Assistant => "chat-role-assistant",
    })
}

/// One sentence for every `NodeFailure` variant (ADR 0016, ADR 0022).
pub fn node_failure(failure: NodeFailure) -> String {
    message(failure.message_key())
}

/// One sentence for every `ComposerError` variant, with its payload where the
/// message asks for one.
pub fn composer_error(error: ComposerError) -> String {
    let key = error.message_key();
    match error {
        ComposerError::Node(failure) => node_failure(failure),
        ComposerError::RevisionConflict { current } => localizer().number(key, "current", current),
        ComposerError::AuthorityChanged { current_epoch } => {
            localizer().number(key, "currentEpoch", current_epoch)
        }
        ComposerError::SnapshotMissing
        | ComposerError::OutcomeMissing
        | ComposerError::ScopeMissing
        | ComposerError::ScopeMismatch => message(key),
    }
}

/// One sentence for every `ChatError` variant.
pub fn chat_error(error: ChatError) -> String {
    match error {
        ChatError::Node(failure) => node_failure(failure),
        ChatError::Draft(error) => composer_error(error),
        ChatError::NoConversation | ChatError::Cancelled | ChatError::ChatIdMissing => {
            message(error.message_key())
        }
    }
}

/// The connect-time transport statuses, which happen before any scope exists to
/// carry a typed failure.
pub fn rpc(error: &Status) -> String {
    message(match error.code {
        Code::Unavailable => "rpc-error-unavailable",
        Code::Cancelled => "rpc-error-cancelled",
        Code::InvalidArgument | Code::OutOfRange => "rpc-error-rejected",
        Code::DeadlineExceeded => "rpc-error-timed-out",
        Code::NotFound => "rpc-error-not-found",
        Code::AlreadyExists => "rpc-error-already-exists",
        Code::PermissionDenied | Code::Unauthenticated => "rpc-error-denied",
        Code::ResourceExhausted => "rpc-error-exhausted",
        Code::FailedPrecondition | Code::Aborted => "rpc-error-changed",
        Code::Unimplemented => "rpc-error-unsupported",
        Code::Internal => "rpc-error-internal",
    })
}
