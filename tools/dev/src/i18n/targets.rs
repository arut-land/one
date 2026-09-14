//! One emitter per platform resource format.
//!
//! Each takes the same parsed catalog and writes the file that platform's own
//! localization API reads, so no surface learns about Fluent (ADR 0022). The
//! output is sorted and byte-stable: `mise run check` regenerates and fails on
//! any diff, which only works if the same input always writes the same bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use arut_i18n_catalog::{Argument, Locale, Message, Part, Pattern};

use super::{BANNER, resource_name};

/// `%1$@` / `%1$lld`, positional so a translation may reorder them.
fn apple_placeholder(index: usize, argument: &Argument) -> String {
    format!(
        "%{}${}",
        index + 1,
        if argument.numeric { "lld" } else { "@" }
    )
}

/// `%1$s` / `%1$d`, the only form `Resources.getString(id, ...)` accepts once
/// a string has more than one argument.
fn android_placeholder(index: usize, argument: &Argument) -> String {
    format!(
        "%{}${}",
        index + 1,
        if argument.numeric { "d" } else { "s" }
    )
}

/// `{0}`, which is what `string.Format` over a `ResourceLoader` string expects.
fn windows_placeholder(index: usize, _argument: &Argument) -> String {
    format!("{{{index}}}")
}

/// Render one pattern with a target's placeholder syntax and text escaping.
fn render(
    pattern: &Pattern,
    order: &[Argument],
    placeholder: fn(usize, &Argument) -> String,
    escape: fn(&str) -> String,
) -> String {
    let mut out = String::new();
    for part in &pattern.parts {
        match part {
            Part::Text(text) => out.push_str(&escape(text)),
            Part::Argument(argument) => {
                let index = order
                    .iter()
                    .position(|candidate| candidate.name == argument.name)
                    .expect("every argument is in the shared order");
                out.push_str(&placeholder(index, argument));
            }
        }
    }
    out
}

/// Apple takes `%` literally except in a format specifier.
fn apple_text(text: &str) -> String {
    text.replace('%', "%%")
}

/// Android's XML resource parser eats apostrophes and quotes unless they are
/// escaped, and reads `@` or `?` in first position as a resource reference.
fn android_text(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('%', "%%")
        .replace('\'', "\\'")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    if escaped.starts_with('@') || escaped.starts_with('?') {
        format!("\\{escaped}")
    } else {
        escaped
    }
}

/// `string.Format` reads braces as placeholders, so literal ones are doubled.
fn windows_text(text: &str) -> String {
    xml_text(&text.replace('{', "{{").replace('}', "}}"))
}

fn xml_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Apple's string catalog, the format Xcode reads directly.
///
/// Written through `serde_json` over sorted maps, so the bytes are a function
/// of the input alone.
#[must_use]
pub(crate) fn xcstrings(source_locale: &str, locales: &[Locale]) -> String {
    let ids: Vec<&String> = {
        let mut ids: Vec<&String> = locales
            .iter()
            .flat_map(|locale| locale.messages.keys())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    };
    let mut strings = serde_json::Map::new();
    for id in ids {
        let mut localizations = serde_json::Map::new();
        for locale in locales {
            let Some(message) = locale.messages.get(id) else {
                continue;
            };
            let order = message.arguments();
            let body = match message {
                Message::Simple(pattern) => serde_json::json!({
                    "stringUnit": unit(&render(pattern, &order, apple_placeholder, apple_text)),
                }),
                Message::Plural { variants, .. } => {
                    let mut plural = serde_json::Map::new();
                    for (category, pattern) in variants {
                        plural.insert(
                            category.name().to_owned(),
                            serde_json::json!({
                                "stringUnit": unit(&render(
                                    pattern, &order, apple_placeholder, apple_text,
                                )),
                            }),
                        );
                    }
                    serde_json::json!({ "variations": { "plural": plural } })
                }
            };
            localizations.insert(locale.tag.clone(), body);
        }
        strings.insert(
            (*id).clone(),
            serde_json::json!({
                "extractionState": "manual",
                "localizations": localizations,
            }),
        );
    }
    let catalog = serde_json::json!({
        "sourceLanguage": source_locale,
        "strings": strings,
        "version": "1.0",
    });
    let mut out = serde_json::to_string_pretty(&catalog).expect("a catalog serializes");
    out.push('\n');
    out
}

fn unit(value: &str) -> serde_json::Value {
    serde_json::json!({ "state": "translated", "value": value })
}

/// One Android `values/strings.xml` (or `values-<lang>/strings.xml`).
#[must_use]
pub(crate) fn strings_xml(locale: &Locale) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    let _ = writeln!(out, "<!-- {BANNER} -->");
    out.push_str("<resources>\n");
    for (id, message) in &locale.messages {
        let name = resource_name(id);
        let order = message.arguments();
        match message {
            Message::Simple(pattern) => {
                let value = render(pattern, &order, android_placeholder, android_text);
                let _ = writeln!(out, "    <string name=\"{name}\">{value}</string>");
            }
            Message::Plural { variants, .. } => {
                let _ = writeln!(out, "    <plurals name=\"{name}\">");
                for (category, pattern) in variants {
                    let value = render(pattern, &order, android_placeholder, android_text);
                    let _ = writeln!(
                        out,
                        "        <item quantity=\"{}\">{value}</item>",
                        category.name()
                    );
                }
                out.push_str("    </plurals>\n");
            }
        }
    }
    out.push_str("</resources>\n");
    out
}

/// The XAML properties an `x:Uid` may resolve. WinUI reads `<Uid>.<property>`
/// out of the `.resw` and assigns it, so a label a control carries needs no C#
/// at all.
const UID_PROPERTIES: [(&str, &str); 4] = [
    ("content", "Content"),
    ("text", "Text"),
    ("placeholder", "PlaceholderText"),
    (
        "name",
        "[using:Microsoft.UI.Xaml.Automation]AutomationProperties.Name",
    ),
];

/// The message ids WinUI XAML may name with `x:Uid`.
///
/// An `x:Uid` names a control's caption, so the prefixes are the ones the
/// source reserves for captions: an action a button performs, a label on a
/// region, and the conversation-list controls. An error or a status is chosen
/// in C# from a typed value (ADR 0016) and never sits in XAML, so it gets no
/// UID entries.
const UID_PREFIXES: [&str; 3] = ["action-", "label-", "conversation-"];

fn takes_uid(id: &str) -> bool {
    UID_PREFIXES.iter().any(|prefix| id.starts_with(prefix))
}

/// Every `x:Uid` WinUI XAML may name, which is what `arut-dev check` checks the
/// XAML against.
#[must_use]
pub(crate) fn uid_names(locale: &Locale) -> BTreeSet<String> {
    locale
        .messages
        .iter()
        .filter(|(id, message)| takes_uid(id) && message.selector().is_none())
        .flat_map(|(id, _)| {
            UID_PROPERTIES.map(|(suffix, _)| format!("{}_uid_{suffix}", resource_name(id)))
        })
        .collect()
}

/// One Windows `.resw`.
///
/// `.resw` is a flat name-to-string map with no plural mechanism, so a plural
/// message becomes one entry per category, named `<key>_<category>`; that is
/// the shape WinUI's own plural resources take and what `ResourceLoader`
/// lookups by suffix expect.
///
/// Each caption property gets its own `<key>_uid_<suffix>` UID. This keeps
/// string values separate from PRI scopes and applies only the property the
/// control supports. `arut-dev check` rejects unknown UIDs.
#[must_use]
pub(crate) fn resw(locale: &Locale) -> String {
    let mut entries: BTreeMap<String, (String, Option<&'static str>)> = BTreeMap::new();
    for (id, message) in &locale.messages {
        let name = resource_name(id);
        let order = message.arguments();
        match message {
            Message::Simple(pattern) => {
                let value = render(pattern, &order, windows_placeholder, windows_text);
                if takes_uid(id) {
                    for (suffix, property) in UID_PROPERTIES {
                        entries.insert(
                            format!("{name}_uid_{suffix}.{property}"),
                            (value.clone(), None),
                        );
                    }
                }
                entries.insert(name, (value, None));
            }
            Message::Plural { variants, .. } => {
                for (category, pattern) in variants {
                    entries.insert(
                        format!("{name}_{}", category.name()),
                        (
                            render(pattern, &order, windows_placeholder, windows_text),
                            Some(category.name()),
                        ),
                    );
                }
            }
        }
    }
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    let _ = writeln!(out, "<!-- {BANNER} -->");
    out.push_str(RESW_HEADER);
    for (name, (value, comment)) in entries {
        let _ = writeln!(out, "  <data name=\"{name}\" xml:space=\"preserve\">");
        let _ = writeln!(out, "    <value>{value}</value>");
        if let Some(category) = comment {
            let _ = writeln!(out, "    <comment>plural category: {category}</comment>");
        }
        out.push_str("  </data>\n");
    }
    out.push_str("</root>\n");
    out
}

/// The fixed ResX preamble every `.resw` carries; the schema is part of the
/// format rather than anything this generator decides.
const RESW_HEADER: &str = r#"<root>
  <resheader name="resmimetype">
    <value>text/microsoft-resx</value>
  </resheader>
  <resheader name="version">
    <value>2.0</value>
  </resheader>
  <resheader name="reader">
    <value>System.Resources.ResXResourceReader, System.Windows.Forms, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089</value>
  </resheader>
  <resheader name="writer">
    <value>System.Resources.ResXResourceWriter, System.Windows.Forms, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089</value>
  </resheader>
"#;

#[cfg(test)]
pub(crate) mod tests {
    use super::{resw, strings_xml, uid_names, xcstrings};
    use arut_i18n_catalog::Locale;

    const PLURAL: &str = "unread = You have { $count ->\n    [one] { NUMBER($count) } unread message\n   *[other] { NUMBER($count) } unread messages\n }.\n";

    /// One locale from one inline source; the accessor tests build theirs here.
    pub(crate) fn locale(tag: &str, source: &str) -> Locale {
        arut_i18n_catalog::parse(tag, &[("test.ftl", source)]).expect("parsed")
    }

    fn english(source: &str) -> Locale {
        locale("en", source)
    }

    #[test]
    fn apple_gets_positional_specifiers_typed_by_number() {
        let out = xcstrings("en", &[english("k = { $who } sent { NUMBER($n) }\n")]);
        assert!(out.contains("%2$@ sent %1$lld"), "{out}");
        assert!(out.contains("\"sourceLanguage\": \"en\""), "{out}");
    }

    #[test]
    fn apple_plurals_become_variations() {
        let out = xcstrings("en", &[english(PLURAL)]);
        assert!(out.contains("\"variations\""), "{out}");
        assert!(out.contains("\"one\""), "{out}");
        assert!(out.contains("You have %1$lld unread message."), "{out}");
    }

    #[test]
    fn android_names_are_identifiers_and_placeholders_are_positional() {
        let out = strings_xml(&english("chat-error-x = { $who } sent { NUMBER($n) }\n"));
        assert!(
            out.contains("<string name=\"chat_error_x\">%2$s sent %1$d</string>"),
            "{out}"
        );
    }

    #[test]
    fn android_escapes_the_apostrophes_the_copy_is_full_of() {
        let out = strings_xml(&english("k = Arut can't reach your node\n"));
        assert!(out.contains("Arut can\\'t reach"), "{out}");
    }

    #[test]
    fn android_plurals_become_a_plurals_element() {
        let out = strings_xml(&english(PLURAL));
        assert!(out.contains("<plurals name=\"unread\">"), "{out}");
        assert!(
            out.contains("<item quantity=\"other\">You have %1$d unread messages.</item>"),
            "{out}"
        );
    }

    #[test]
    fn windows_gets_brace_placeholders_and_one_entry_per_plural_category() {
        let simple = resw(&english("k = { $who } sent { NUMBER($n) }\n"));
        assert!(simple.contains("<value>{1} sent {0}</value>"), "{simple}");
        let plural = resw(&english(PLURAL));
        assert!(plural.contains("name=\"unread_one\""), "{plural}");
        assert!(plural.contains("name=\"unread_other\""), "{plural}");
    }

    #[test]
    fn reordered_translations_keep_the_same_native_argument_positions() {
        let english = locale("en", "k = { $who } sent { NUMBER($n) }\n");
        let french = locale("fr", "k = { NUMBER($n) } de { $who }\n");
        assert_eq!(
            english.messages["k"].arguments(),
            french.messages["k"].arguments()
        );
        assert!(strings_xml(&french).contains("%1$d de %2$s"));
        assert!(resw(&french).contains("{0} de {1}"));
        let apple = xcstrings("en", &[english, french]);
        assert!(apple.contains("%2$@ sent %1$lld"));
        assert!(apple.contains("%1$lld de %2$@"));
    }

    #[test]
    fn a_caption_message_also_gets_the_uid_entries_xaml_resolves() {
        let out = resw(&english(
            "action-send = Send\nchat-error-cancelled = Gone\n",
        ));
        assert!(
            out.contains("name=\"action_send_uid_content.Content\""),
            "{out}"
        );
        assert!(
            out.contains("name=\"action_send_uid_name.[using:Microsoft.UI.Xaml.Automation]AutomationProperties.Name\""),
            "{out}"
        );
        assert!(out.contains("name=\"action_send\""), "{out}");
        // A resource value cannot also be a PRI scope containing properties.
        assert!(!out.contains("name=\"action_send."), "{out}");
        for uid in uid_names(&english("action-send = Send\n")) {
            assert_eq!(
                out.matches(&format!("name=\"{uid}.")).count(),
                1,
                "each UID must assign exactly one supported property: {uid}"
            );
        }
        // An error is chosen in C# from a typed value, so it never sits in XAML.
        assert!(!out.contains("chat_error_cancelled.Text"), "{out}");
        assert_eq!(
            uid_names(&english(
                "action-send = Send\nchat-error-cancelled = Gone\n"
            )),
            [
                "action_send_uid_content",
                "action_send_uid_text",
                "action_send_uid_placeholder",
                "action_send_uid_name"
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
        );
    }

    #[test]
    fn xml_targets_escape_markup_characters() {
        let source = "k = a & b < c\n";
        assert!(strings_xml(&english(source)).contains("a &amp; b &lt; c"));
        assert!(resw(&english(source)).contains("a &amp; b &lt; c"));
    }
}
