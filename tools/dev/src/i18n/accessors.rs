//! Message keys for the consumers whose platform has no resource file.
//!
//! No message id is typed by hand (ADR 0022), but the mechanism that enforces
//! that differs per platform. Apple, Android and Windows each read a generated
//! resource file, so all their consumers need is a checked way to name a key:
//! a Swift function over `String(localized:)`, a C# `const string`. TypeScript
//! has no resource format, so it gets the key union, the argument types, the
//! arguments each message interpolates, and the Fluent text itself.
//!
//! Android is the one platform that needs both halves. AAPT2 generates
//! `R.string` from the `strings.xml` this generator already emits, which covers
//! every key a layout names; what it cannot cover is the runtime lookup an
//! error path does, from a Fluent id the core supplied to the resource that
//! renders it. That map is [`kotlin`], so no surface writes it by hand.
//!
//! Every emitter here is a pure function of the catalog, like the resource
//! emitters in [`crate::i18n::targets`], and for the same reason.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use arut_i18n::{available_locales, locale_resources};
use arut_i18n_catalog::{Locale, Message};

use super::{BANNER, resource_name};

/// `composer-error-revision-conflict` -> `ComposerErrorRevisionConflict`.
fn pascal(id: &str) -> String {
    id.split('-').map(capitalize).collect()
}

/// `composer-error-revision-conflict` -> `composerErrorRevisionConflict`.
fn camel(id: &str) -> String {
    let mut parts = id.split('-');
    let first = parts.next().unwrap_or_default().to_owned();
    parts.fold(first, |mut out, part| {
        out.push_str(&capitalize(part));
        out
    })
}

fn capitalize(part: &str) -> String {
    let mut characters = part.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + characters.as_str()
    })
}

/// The one locale whose message set defines every accessor. The generator has
/// already proved every other locale defines exactly the same ids.
fn source_of<'a>(locales: &'a [Locale], tag: &str) -> &'a BTreeMap<String, Message> {
    locales.iter().find(|locale| locale.tag == tag).map_or_else(
        || unreachable!("the default locale is always parsed"),
        |locale| &locale.messages,
    )
}

/// `surfaces/apple/shared/Sources/ArutSurface/Generated/L10n.swift`.
///
/// One line per message: Foundation does the lookup, the argument
/// substitution and the language negotiation, so the generated half is the
/// key and the parameter list.
#[must_use]
pub(crate) fn swift(default_locale: &str, locales: &[Locale]) -> String {
    let messages = source_of(locales, default_locale);
    let mut out = String::new();
    let _ = writeln!(out, "// {BANNER}");
    out.push_str(
        "//\n\
         // One function per message in product/i18n/locales, over the\n\
         // Localizable.xcstrings catalog generated beside it, so this surface\n\
         // localizes the way any other Apple app does.\n\
         \n\
         import Foundation\n\
         \n\
         enum L10n {\n",
    );
    for (id, message) in messages {
        let arguments = message.arguments();
        let parameters = arguments
            .iter()
            .map(|argument| {
                let swift_type = if argument.numeric { "Int" } else { "String" };
                format!("{}: {swift_type}", camel(&argument.name))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let lookup =
            format!("String(localized: \"{id}\", table: \"Localizable\", bundle: .module)");
        let body = if arguments.is_empty() {
            lookup
        } else {
            let values = arguments
                .iter()
                .map(|argument| camel(&argument.name))
                .collect::<Vec<_>>()
                .join(", ");
            format!("String(format: {lookup}, {values})")
        };
        let _ = writeln!(
            out,
            "    static func {}({parameters}) -> String {{ {body} }}",
            camel(id)
        );
    }
    out.push_str("}\n");
    out
}

/// `surfaces/windows/Generated/L10n.cs`.
///
/// Key constants plus the two lookups that need a body. `Get` exists because
/// `ResourceLoader` takes a string and `string.Format` takes the arguments;
/// `Quantity` exists because `.resw` is a flat map with no plural mechanism,
/// so the CLDR category has to be chosen in C#.
#[must_use]
pub(crate) fn csharp(default_locale: &str, locales: &[Locale]) -> String {
    let messages = source_of(locales, default_locale);
    let mut out = String::new();
    let _ = writeln!(out, "// {BANNER}");
    out.push_str(
        "//\n\
         // One constant per message in product/i18n/locales, naming its entry in\n\
         // the Resources.resw generated beside it. ResourceLoader does the lookup\n\
         // and the language resolution, so this surface localizes the way any\n\
         // other WinUI app does. XAML that carries x:Uid needs no constant: the\n\
         // resw also holds the Uid.Property entries WinUI resolves on its own.\n\
         \n\
         using System.Globalization;\n\
         using Microsoft.Windows.ApplicationModel.Resources;\n\
         \n\
         namespace Arut.Surface.Windows;\n\
         \n\
         internal static class L10n\n\
         {\n\
         \x20   private static readonly ResourceLoader Resources = new();\n\
         \n\
         \x20   /// <summary>The string named by <paramref name=\"key\"/>, with its arguments substituted.</summary>\n\
         \x20   public static string Get(string key, params object[] arguments) =>\n\
         \x20       arguments.Length == 0\n\
         \x20           ? Resources.GetString(key)\n\
         \x20           : string.Format(Resources.GetString(key), arguments);\n\
         \n\
         \x20   /// <summary>The plural entry of <paramref name=\"key\"/> for <paramref name=\"count\"/>.</summary>\n\
         \x20   public static string Quantity(string key, long count) =>\n\
         \x20       Resources.GetString($\"{key}_{Category(count)}\");\n\
         \n",
    );
    out.push_str(&csharp_plural_rule(locales));
    for id in messages.keys() {
        let _ = writeln!(
            out,
            "    public const string {} = \"{}\";",
            pascal(id),
            resource_name(id)
        );
    }
    out.push_str("}\n");
    out
}

/// The CLDR plural rule of every shipped locale, as one `switch`.
///
/// `.resw` has no plural mechanism, so this is the only place the category can
/// be chosen. A new locale adds an arm here through `cldr_rule`; a locale with
/// categories beyond one-or-other renders wrongly until it does.
fn csharp_plural_rule(locales: &[Locale]) -> String {
    let mut out = String::from(
        "    private static string Category(long count) =>\n\
         \x20       CultureInfo.CurrentUICulture.TwoLetterISOLanguageName switch\n\
         \x20       {\n",
    );
    for locale in locales {
        let _ = writeln!(
            out,
            "            \"{}\" => {},",
            locale.tag,
            cldr_rule(&locale.tag)
        );
    }
    out.push_str("            _ => \"other\",\n        };\n\n");
    out
}

/// The CLDR cardinal rule of one language, as a C# expression over `count`.
fn cldr_rule(tag: &str) -> &'static str {
    match tag {
        "en" | "de" | "nl" | "sv" | "da" | "it" | "es" | "pt" => "count == 1 ? \"one\" : \"other\"",
        "fr" => "count is 0 or 1 ? \"one\" : \"other\"",
        // Correct for a language with one category, wrong for anything with
        // few/many; add that language's rule here when its locale lands.
        _ => "\"other\"",
    }
}

/// `bindings/kotlin/src/main/kotlin/dev/arut/bindings/generated/Messages.kt`.
///
/// The Fluent id a typed error names, mapped to the Android resource AAPT2
/// generated from the `strings.xml` beside it. Both live in the binding
/// library, so `R` is the library's own and the application gets the same
/// strings through Gradle's resource merging.
#[must_use]
pub(crate) fn kotlin(default_locale: &str, locales: &[Locale]) -> String {
    let messages = source_of(locales, default_locale);
    let mut out = String::new();
    let _ = writeln!(out, "// {BANNER}");
    out.push_str(
        "//\n\
         // Every message id in product/i18n/locales, beside the Android resource\n\
         // generated for it. A surface resolving a Fluent id the core supplied\n\
         // reads this map; a key a layout names reaches R.string directly.\n\
         \n\
         package dev.arut.bindings.generated\n\
         \n\
         import dev.arut.bindings.R\n\
         \n\
         /** Android resource identifiers, keyed by the Fluent message id that names them. */\n\
         object Messages {\n\
         \x20   val byKey: Map<String, Int> =\n\
         \x20       mapOf(\n",
    );
    for (id, message) in messages {
        let kind = if message.selector().is_some() {
            "plurals"
        } else {
            "string"
        };
        let _ = writeln!(
            out,
            "            \"{id}\" to R.{kind}.{},",
            resource_name(id)
        );
    }
    out.push_str("        )\n}\n");
    out
}

/// `bindings/typescript/src/generated/l10n.ts`.
///
/// A key union, the argument types each parameterised message needs, and one
/// `t`. Naming a message that does not exist, or forgetting its arguments, is
/// a type error.
#[must_use]
pub(crate) fn typescript(default_locale: &str, locales: &[Locale]) -> String {
    let messages = source_of(locales, default_locale);
    let mut out = String::new();
    let _ = writeln!(out, "// {BANNER}");
    out.push_str(
        "//\n\
         // Every message id, the arguments each one interpolates, and one `t` over\n\
         // whatever bundle the surface loaded. Nothing outside this file writes a\n\
         // key, so a message that does not exist is a type error.\n\
         \n\
         /** What `t` needs from a loaded string source. */\n\
         export interface L10nBundle {\n\
         \x20 format(key: string, args?: Record<string, string | number>): string;\n\
         }\n\
         \n\
         /** Every message the product can show. */\n\
         export type MessageKey =\n",
    );
    let keys: Vec<&String> = messages.keys().collect();
    for (index, id) in keys.iter().enumerate() {
        let end = if index + 1 == keys.len() { ";" } else { "" };
        let _ = writeln!(out, "  | \"{id}\"{end}");
    }
    out.push_str("\n/** The arguments each parameterised message interpolates. */\nexport interface Args {\n");
    for (id, message) in messages {
        let arguments = message.arguments();
        if arguments.is_empty() {
            continue;
        }
        let fields = arguments
            .iter()
            .map(|argument| {
                let typescript_type = if argument.numeric { "number" } else { "string" };
                format!("{}: {typescript_type}", argument.name)
            })
            .collect::<Vec<_>>()
            .join("; ");
        let _ = writeln!(out, "  \"{id}\": {{ {fields} }};");
    }
    out.push_str(
        "}\n\
         \n\
         /** The arguments each message interpolates, in the order every generated\n\
          * consumer reads them. A typed error crosses FFI carrying its values in\n\
          * this order. */\n\
         export const placeables: Record<MessageKey, readonly string[]> = {\n",
    );
    for (id, message) in messages {
        let names = message
            .arguments()
            .iter()
            .map(|argument| format!("\"{}\"", argument.name))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "  \"{id}\": [{names}],");
    }
    out.push_str(
        "};\n\
         \n\
         /** Format one message. */\n\
         export function t(bundle: L10nBundle, key: Exclude<MessageKey, keyof Args>): string;\n\
         export function t<K extends keyof Args>(bundle: L10nBundle, key: K, args: Args[K]): string;\n\
         export function t(\n\
         \x20 bundle: L10nBundle,\n\
         \x20 key: MessageKey,\n\
         \x20 args?: Record<string, string | number>,\n\
         ): string {\n\
         \x20 return bundle.format(key, args);\n\
         }\n",
    );
    out
}

/// `bindings/typescript/src/generated/catalog.ts`: the Fluent source itself.
///
/// One copy reaches web, Chromium and the VS Code extension host through this
/// package, instead of three copies beside three bundles. Past roughly ten
/// locales this becomes a map of loaders and the call site does not change.
#[must_use]
pub(crate) fn typescript_catalog() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "// {BANNER}");
    out.push_str(
        "//\n\
         // The Fluent source as a module, so the browser and editor surfaces read\n\
         // one copy through this package rather than fetching files beside their\n\
         // own bundles. `@fluent/bundle` parses the text; `@fluent/langneg`\n\
         // negotiates over `locales`.\n\
         \n\
         /** Every locale the product ships, in the order it prefers them. */\n\
         export const locales = [\n",
    );
    for tag in available_locales() {
        let _ = writeln!(out, "  \"{tag}\",");
    }
    out.push_str("] as const;\n\n/** The Fluent text of each locale: every `.ftl` file it ships, in order. */\nexport const catalog: Record<(typeof locales)[number], string> = {\n");
    for tag in available_locales() {
        let text: String = locale_resources(tag)
            .iter()
            .map(|(_, source)| *source)
            .collect::<Vec<_>>()
            .join("\n");
        let literal = serde_json::to_string(&text).expect("a string serializes");
        let _ = writeln!(out, "  {tag}: {literal},");
    }
    out.push_str("};\n");
    out
}

#[cfg(test)]
mod tests {
    use super::{camel, csharp, kotlin, pascal, swift, typescript, typescript_catalog};
    use crate::i18n::targets::tests::locale;
    use arut_i18n_catalog::Locale;

    const SOURCE: &str = "chat-error-no-conversation = There's no conversation yet.\ncomposer-error-revision-conflict = now at { NUMBER($current) }\nwelcome = Hello { $who }\n";
    const PLURAL: &str =
        "unread = { $count ->\n    [one] one unread\n   *[other] { NUMBER($count) } unread\n }\n";

    fn only(source: &str) -> Vec<Locale> {
        vec![locale("en", source)]
    }

    #[test]
    fn names_convert_between_the_conventions_each_language_uses() {
        assert_eq!(
            pascal("composer-error-revision-conflict"),
            "ComposerErrorRevisionConflict"
        );
        assert_eq!(
            camel("composer-error-revision-conflict"),
            "composerErrorRevisionConflict"
        );
    }

    #[test]
    fn swift_gets_one_typed_function_per_message_over_the_string_catalog() {
        let out = swift("en", &only(SOURCE));
        assert!(
            out.contains("static func chatErrorNoConversation() -> String { String(localized: \"chat-error-no-conversation\", table: \"Localizable\", bundle: .module) }"),
            "{out}"
        );
        assert!(
            out.contains("static func composerErrorRevisionConflict(current: Int) -> String { String(format: String(localized: \"composer-error-revision-conflict\", table: \"Localizable\", bundle: .module), current) }"),
            "{out}"
        );
    }

    #[test]
    fn csharp_gets_key_constants_and_the_two_lookups_that_need_a_body() {
        let out = csharp("en", &only(SOURCE));
        assert!(
            out.contains(
                "public const string ChatErrorNoConversation = \"chat_error_no_conversation\";"
            ),
            "{out}"
        );
        assert!(
            out.contains("public static string Get(string key, params object[] arguments)"),
            "{out}"
        );
        assert!(
            out.contains("public static string Quantity(string key, long count)"),
            "{out}"
        );
        assert!(
            out.contains("\"en\" => count == 1 ? \"one\" : \"other\","),
            "{out}"
        );
    }

    #[test]
    fn typescript_gets_a_key_union_and_an_argument_map() {
        let out = typescript("en", &only(SOURCE));
        assert!(
            out.contains("  | \"chat-error-no-conversation\"\n"),
            "{out}"
        );
        assert!(out.contains("  | \"welcome\";\n"), "{out}");
        assert!(
            out.contains("  \"composer-error-revision-conflict\": { current: number };"),
            "{out}"
        );
        assert!(out.contains("  \"welcome\": { who: string };"), "{out}");
        assert!(
            out.contains("export function t(bundle: L10nBundle, key: Exclude<MessageKey, keyof Args>): string;"),
            "{out}"
        );
    }

    #[test]
    fn kotlin_maps_every_id_to_the_android_resource_generated_for_it() {
        let out = kotlin("en", &only(SOURCE));
        assert!(
            out.contains(
                "\"composer-error-revision-conflict\" to R.string.composer_error_revision_conflict,"
            ),
            "{out}"
        );
        let plural = kotlin("en", &only(PLURAL));
        assert!(
            plural.contains("\"unread\" to R.plurals.unread,"),
            "{plural}"
        );
    }

    #[test]
    fn typescript_names_the_arguments_each_message_interpolates() {
        let out = typescript("en", &only(SOURCE));
        assert!(out.contains("  \"welcome\": [\"who\"],"), "{out}");
        assert!(
            out.contains("  \"chat-error-no-conversation\": [],"),
            "{out}"
        );
    }

    #[test]
    fn a_plural_selector_is_numeric_in_every_accessor() {
        let locales = only(PLURAL);
        assert!(swift("en", &locales).contains("static func unread(count: Int) -> String"));
        assert!(typescript("en", &locales).contains("\"unread\": { count: number };"));
    }

    #[test]
    fn the_typescript_catalog_carries_the_shipped_fluent_text() {
        let out = typescript_catalog();
        assert!(
            out.contains("export const locales = [\n  \"en\",\n] as const;"),
            "{out}"
        );
        assert!(out.contains("chat-error-cancelled ="), "{out}");
    }

    #[test]
    fn every_emitter_is_a_function_of_its_input_alone() {
        let locales = only(SOURCE);
        assert_eq!(swift("en", &locales), swift("en", &only(SOURCE)));
        assert_eq!(csharp("en", &locales), csharp("en", &only(SOURCE)));
        assert_eq!(kotlin("en", &locales), kotlin("en", &only(SOURCE)));
        assert_eq!(typescript("en", &locales), typescript("en", &only(SOURCE)));
    }
}
