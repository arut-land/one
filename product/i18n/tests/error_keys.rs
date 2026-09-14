//! The typed errors and the string source have to agree (ADR 0016, ADR 0022).
//!
//! The required keys come from `MESSAGE_KEYS`, which `#[derive(Localized)]`
//! emits from the enums themselves, and the defined keys from `Message::KEYS`,
//! which `build.rs` emits from the Fluent source. Neither side is listed by
//! hand here, so the only way to add a variant is to give it a key.
//!
//! Locale parity -- every locale defining the same ids -- is not tested here:
//! `arut_i18n_catalog::require_identical_key_sets` refuses to generate at all
//! when two locales disagree, which is earlier and cheaper than a test.

use std::collections::BTreeSet;

use arut_feature_chat::errors::{ChatError, ComposerError, NodeFailure};
use arut_i18n::{DEFAULT_LOCALE, Localizer, Message, available_locales};

/// The prefixes `errors.ftl` reserves for the enums below. A message under one
/// of these that no variant asks for is an orphan, which is how a renamed or
/// deleted variant is caught.
const ERROR_PREFIXES: &[&str] = &["node-failure-", "composer-error-", "chat-error-"];

fn required_keys() -> BTreeSet<&'static str> {
    NodeFailure::MESSAGE_KEYS
        .iter()
        .chain(ComposerError::MESSAGE_KEYS)
        .chain(ChatError::MESSAGE_KEYS)
        .copied()
        .collect()
}

#[test]
fn every_locale_has_a_message_for_every_error_variant() {
    for locale in available_locales() {
        let localizer = Localizer::for_locale(locale);
        for key in required_keys() {
            assert!(
                localizer.has(key),
                "locale {locale} has no message for `{key}`; add it to locales/{locale}/errors.ftl"
            );
        }
    }
}

#[test]
fn no_error_message_is_left_behind_by_a_variant_that_went_away() {
    let required = required_keys();
    for id in Message::KEYS {
        if ERROR_PREFIXES.iter().any(|prefix| id.starts_with(prefix)) {
            assert!(
                required.contains(id),
                "the string source defines `{id}`, which no error variant asks for"
            );
        }
    }
}

#[test]
fn revision_identifiers_keep_all_unsigned_bits() {
    let current = u64::MAX.to_string();
    let localizer = Localizer::for_locale(DEFAULT_LOCALE);
    let rendered = localizer.format(&Message::ComposerErrorRevisionConflict {
        current: current.clone(),
    });
    assert!(rendered.contains(&current), "{rendered}");
    assert_eq!(
        Message::ComposerErrorRevisionConflict { current }.key(),
        ComposerError::RevisionConflict { current: 41 }.message_key()
    );
}

#[test]
fn a_wrapped_node_failure_keeps_the_node_failures_own_message() {
    let failure = NodeFailure::Overloaded;
    assert_eq!(
        ChatError::Draft(ComposerError::Node(failure)).message_key(),
        failure.message_key()
    );
}
