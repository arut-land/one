//! Every user-facing string the product owns, once, in Fluent (ADR 0022).
//!
//! `locales/<lang>/*.ftl` is the single source. Rust and wasm consumers read it
//! through [`Localizer`]; `tools/i18n` turns the same files into
//! `Localizable.xcstrings`, `strings.xml`, `.resw`, and a served copy for
//! `@fluent/bundle`, so a native surface localizes through its own platform API
//! and never through this crate.
//!
//! The core still returns typed values and no text (ADR 0016). What lives here
//! is the sentence a surface shows for one of those values, keyed by the
//! convention `<enum-in-kebab-case>-<variant-in-kebab-case>`; the other half of
//! the convention is `message_key` next to each error enum in
//! `arut_feature_chat::errors`, and `tests/error_keys.rs` fails when the two
//! disagree or when a locale is missing a key.
//!
//! Locale choice belongs to the surface: it passes the preferred-language list
//! its platform already resolved (GTK's `glib::language_names()`, a browser's
//! `navigator.languages`) and this crate negotiates against what it ships.
//! Nothing here touches the environment, the filesystem, or a clock, so it
//! builds for `wasm32-unknown-unknown` without wasm-bindgen.

mod generated;

use fluent_bundle::FluentResource;
use fluent_bundle::concurrent::FluentBundle;
use fluent_langneg::{NegotiationStrategy, negotiate_languages};
use unic_langid::LanguageIdentifier;

pub use fluent_bundle::{FluentArgs, FluentError, FluentValue};
pub use generated::Message;

/// The locale every negotiation falls back to, and the one the `.ftl` files are
/// authored in.
pub const DEFAULT_LOCALE: &str = "en";

/// One shipped locale and the `.ftl` files it is made of.
///
/// Adding a language is this table plus the directory it names; the generator
/// and the key test both walk it, so nothing else has a list of locales.
const LOCALES: &[(&str, &[(&str, &str)])] = &[(
    "en",
    &[
        ("errors.ftl", include_str!("../locales/en/errors.ftl")),
        ("ui.ftl", include_str!("../locales/en/ui.ftl")),
    ],
)];

/// Every locale this crate ships, in the order it prefers them.
pub fn available_locales() -> impl Iterator<Item = &'static str> {
    LOCALES.iter().map(|(tag, _)| *tag)
}

/// The raw `(file name, Fluent source)` pairs of one locale, or an empty slice
/// if it is not shipped. The generator and the tests read the source; running
/// code reads it through a [`Localizer`].
#[must_use]
pub fn locale_resources(locale: &str) -> &'static [(&'static str, &'static str)] {
    LOCALES
        .iter()
        .find(|(tag, _)| *tag == locale)
        .map_or(&[], |(_, resources)| *resources)
}

/// A negotiated chain of Fluent bundles: the person's best locale first, the
/// default locale last, so a message a translation has not reached yet still
/// renders in English rather than as its key.
pub struct Localizer {
    bundles: Vec<FluentBundle<FluentResource>>,
    locales: Vec<LanguageIdentifier>,
}

impl Localizer {
    /// Negotiate against `preferred`, a surface-supplied language list in
    /// descending order of preference. Tags the platform cannot parse are
    /// skipped; an empty or entirely unusable list yields [`DEFAULT_LOCALE`].
    #[must_use]
    pub fn negotiate<S: AsRef<str>>(preferred: &[S]) -> Self {
        let requested: Vec<LanguageIdentifier> = preferred
            .iter()
            .filter_map(|tag| normalize(tag.as_ref()))
            .collect();
        let available: Vec<LanguageIdentifier> = available_locales()
            .filter_map(|tag| tag.parse().ok())
            .collect();
        let default: LanguageIdentifier = DEFAULT_LOCALE.parse().expect("default locale parses");
        let chosen = negotiate_languages(
            &requested,
            &available,
            Some(&default),
            NegotiationStrategy::Filtering,
        );
        Self::over(chosen.into_iter().cloned())
    }

    /// A localizer for exactly one locale, with no fallback behind it. Used by
    /// the tests that prove a locale is complete on its own.
    #[must_use]
    pub fn for_locale(locale: &str) -> Self {
        Self::over(locale.parse::<LanguageIdentifier>().ok())
    }

    fn over(locales: impl IntoIterator<Item = LanguageIdentifier>) -> Self {
        let mut bundles = Vec::new();
        let mut chosen = Vec::new();
        for locale in locales {
            let mut bundle = FluentBundle::new_concurrent(vec![locale.clone()]);
            // Fluent wraps placeables in bidi isolation marks by default, which
            // are invisible in a browser but land as literal U+2068/U+2069 in a
            // GTK label or a native resource string.
            bundle.set_use_isolating(false);
            // fluent-rs ships no built-in functions, so NUMBER() -- the marker
            // that tells `tools/i18n` to emit an integer placeholder on native
            // surfaces -- has to be supplied here or every message that uses it
            // resolves to an error.
            bundle
                .add_function("NUMBER", |positional, _named| {
                    positional.first().cloned().unwrap_or(FluentValue::Error)
                })
                .expect("NUMBER is registered once per bundle");
            for (_, source) in locale_resources(&locale.to_string()) {
                // A malformed shipped resource is a build-time mistake that
                // `every_shipped_resource_parses` catches; at runtime the
                // parsed prefix is still better than no strings at all.
                let (Ok(resource) | Err((resource, _))) =
                    FluentResource::try_new((*source).to_owned());
                let _ = bundle.add_resource(resource);
            }
            chosen.push(locale);
            bundles.push(bundle);
        }
        Self {
            bundles,
            locales: chosen,
        }
    }

    /// The negotiated locales, best first.
    #[must_use]
    pub fn locales(&self) -> &[LanguageIdentifier] {
        &self.locales
    }

    /// Whether any negotiated locale defines `key`.
    #[must_use]
    pub fn has(&self, key: &str) -> bool {
        self.bundles
            .iter()
            .any(|bundle| bundle.get_message(key).is_some_and(|m| m.value().is_some()))
    }

    /// Format one message, taking the first negotiated locale that defines it.
    ///
    /// A [`Message`] is generated from the `.ftl` source, so there is no way to
    /// ask for a string that does not exist or to pass an argument of the wrong
    /// type; both are compile errors.
    #[must_use]
    pub fn format(&self, message: &Message) -> String {
        self.format_key(message.key(), Some(&message.args()))
    }

    /// Format by raw key. Private on purpose: ADR 0022 says no message id is
    /// typed by hand, and [`Message`] is how a caller names one.
    fn format_key(&self, key: &str, args: Option<&FluentArgs<'_>>) -> String {
        for bundle in &self.bundles {
            let Some(message) = bundle.get_message(key) else {
                continue;
            };
            let Some(pattern) = message.value() else {
                continue;
            };
            let mut errors = Vec::new();
            let text = bundle.format_pattern(pattern, args, &mut errors);
            if errors.is_empty() {
                return text.into_owned();
            }
        }
        key.to_owned()
    }
}

impl Default for Localizer {
    fn default() -> Self {
        Self::for_locale(DEFAULT_LOCALE)
    }
}

/// Turn a platform locale name into a language identifier.
///
/// POSIX names arrive as `en_GB.UTF-8` or `sr_RS@latin`, and glib appends the
/// `C` locale, which is not a language.
fn normalize(tag: &str) -> Option<LanguageIdentifier> {
    let tag = tag.split(['.', '@']).next().unwrap_or(tag);
    if tag.is_empty() || tag == "C" || tag == "POSIX" {
        return None;
    }
    tag.replace('_', "-").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_LOCALE, LOCALES, Localizer, Message, available_locales, normalize};
    use fluent_bundle::FluentResource;

    #[test]
    fn every_shipped_resource_parses() {
        for (locale, resources) in LOCALES {
            for (file, source) in *resources {
                if let Err((_, errors)) = FluentResource::try_new((*source).to_owned()) {
                    panic!("{locale}/{file} is not valid Fluent: {errors:?}");
                }
            }
        }
    }

    #[test]
    fn the_default_locale_is_shipped() {
        assert!(available_locales().any(|locale| locale == DEFAULT_LOCALE));
    }

    #[test]
    fn a_platform_locale_list_negotiates_down_to_a_shipped_language() {
        let localizer = Localizer::negotiate(&["en_GB.UTF-8", "en", "C"]);
        assert_eq!(localizer.locales().len(), 1);
        assert_eq!(localizer.locales()[0].to_string(), "en");
    }

    #[test]
    fn an_unknown_language_falls_back_to_the_default_locale() {
        let localizer = Localizer::negotiate(&["qps-ploc"]);
        assert_eq!(localizer.locales()[0].to_string(), DEFAULT_LOCALE);
        assert_eq!(
            localizer.format(&Message::ChatErrorNoConversation),
            "There's no conversation to send this to yet."
        );
    }

    #[test]
    fn an_empty_preference_list_still_produces_strings() {
        let localizer = Localizer::negotiate::<&str>(&[]);
        assert_eq!(localizer.format(&Message::ActionSend), "Send");
    }

    #[test]
    fn a_number_argument_reaches_the_sentence() {
        let localizer = Localizer::default();
        assert_eq!(
            localizer.format(&Message::ComposerErrorRevisionConflict { current: 7 }),
            "Someone else edited this draft first, so your edit didn't go through; it is now at revision 7."
        );
    }

    #[test]
    fn an_unknown_key_comes_back_as_itself() {
        assert_eq!(
            Localizer::default().format_key("no-such-key", None),
            "no-such-key"
        );
    }

    #[test]
    fn posix_decorations_and_the_c_locale_are_not_languages() {
        assert_eq!(normalize("en_GB.UTF-8").unwrap().to_string(), "en-GB");
        assert!(normalize("C").is_none());
        assert!(normalize("POSIX").is_none());
        assert!(normalize("").is_none());
    }
}
