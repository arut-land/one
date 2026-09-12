//! The typed errors and the string source have to agree (ADR 0016, ADR 0022).
//!
//! The required keys are derived from the enums themselves rather than listed
//! here, so the only way to add a variant is to give it a key, and the only way
//! to keep this test green is to write that key's message in every locale.

use std::collections::BTreeSet;

use arut_feature_chat::errors::{ChatError, ComposerError, NodeFailure};
use arut_i18n::{DEFAULT_LOCALE, Localizer, available_locales, locale_resources};
use fluent_syntax::ast::Entry;
use fluent_syntax::parser;

/// The prefixes `errors.ftl` reserves for the enums below. A message under one
/// of these that no variant asks for is an orphan, which is how a renamed or
/// deleted variant is caught.
const ERROR_PREFIXES: &[&str] = &["node-failure-", "composer-error-", "chat-error-"];

fn required_keys() -> BTreeSet<&'static str> {
    NodeFailure::ALL
        .iter()
        .map(|failure| failure.message_key())
        .chain(ComposerError::ALL.iter().map(|e| e.message_key()))
        .chain(ChatError::ALL.iter().map(|e| e.message_key()))
        .collect()
}

fn message_ids(locale: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for (file, source) in locale_resources(locale) {
        let parsed = parser::parse(*source).unwrap_or_else(|(resource, errors)| {
            panic!("{locale}/{file}: {errors:?} in {resource:?}")
        });
        for entry in parsed.body {
            if let Entry::Message(message) = entry {
                ids.insert(message.id.name.to_owned());
            }
        }
    }
    ids
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
    for locale in available_locales() {
        for id in message_ids(locale) {
            if ERROR_PREFIXES.iter().any(|prefix| id.starts_with(prefix)) {
                assert!(
                    required.contains(id.as_str()),
                    "locale {locale} defines `{id}`, which no error variant asks for"
                );
            }
        }
    }
}

#[test]
fn every_locale_carries_the_same_message_ids_as_the_default_one() {
    let expected = message_ids(DEFAULT_LOCALE);
    for locale in available_locales() {
        let ids = message_ids(locale);
        let missing: Vec<_> = expected.difference(&ids).collect();
        let extra: Vec<_> = ids.difference(&expected).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "locale {locale} differs from {DEFAULT_LOCALE}: missing {missing:?}, extra {extra:?}"
        );
    }
}

#[test]
fn a_variant_with_a_payload_renders_that_payload() {
    let localizer = Localizer::for_locale(DEFAULT_LOCALE);
    let conflict = ComposerError::RevisionConflict { current: 41 };
    assert!(
        localizer
            .number(conflict.message_key(), "current", 41)
            .contains("41")
    );
    let moved = ComposerError::AuthorityChanged { current_epoch: 9 };
    assert!(
        localizer
            .number(moved.message_key(), "currentEpoch", 9)
            .contains('9')
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
