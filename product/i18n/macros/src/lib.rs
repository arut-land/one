//! `#[derive(Localized)]`: give a typed error its Fluent message id, and refuse
//! to compile if the string source does not have one.
//!
//! ADR 0022 keys a message by the enum and variant that names it:
//! `ComposerError::RevisionConflict` is `composer-error-revision-conflict`. This
//! derive applies that convention and checks it against
//! `product/i18n/locales/en/*.ftl` while it expands, so a variant added without
//! a message is a compile error at the definition site rather than a test
//! failure somewhere else.
//!
//! A variant whose own key is absent but which wraps exactly one value -- the
//! `Node(NodeFailure)` and `Draft(ComposerError)` shapes -- delegates to that
//! value, because a node failure reads the same whichever scope met it.
//!
//! The source is read through `arut-i18n-catalog`, the same parser
//! `product/i18n/build.rs` and `arut-dev generate` use, so this derive cannot
//! accept a message the generator would later refuse.
//!
//! This is a proc macro, so a crate that derives it takes a build-time
//! dependency and no runtime one: the expansion names nothing from `arut-i18n`,
//! only `&'static str`. That is what lets a `features/` crate use it without
//! depending on anything above it. The typed `Message` bridge, which does need
//! `arut-i18n`, lives in `arut-i18n` itself.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, parse_macro_input};

/// The string source, found relative to this crate rather than to the crate
/// deriving, so any crate in the workspace expands against the same files.
const LOCALES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../locales/en");

/// Derive `message_key` and `MESSAGE_KEYS` from the enum and variant names,
/// checked against the Fluent source.
#[proc_macro_derive(Localized)]
pub fn localized(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "Localized describes one typed outcome per variant, so it only applies to an enum",
        ));
    };
    let (files, messages) = read_source()
        .map_err(|reason| syn::Error::new_spanned(input, format!("{LOCALES}: {reason}")))?;
    let name = &input.ident;
    let prefix = kebab(&name.to_string());

    let mut arms = Vec::new();
    let mut keys = Vec::new();
    for variant in &data.variants {
        let variant_name = &variant.ident;
        let key = format!("{prefix}-{}", kebab(&variant_name.to_string()));
        if let Some(arguments) = messages.get(&key) {
            check_arguments(variant, &key, arguments)?;
            let pattern = match &variant.fields {
                Fields::Unit => quote!(Self::#variant_name),
                Fields::Named(_) => quote!(Self::#variant_name { .. }),
                Fields::Unnamed(_) => quote!(Self::#variant_name(..)),
            };
            arms.push(quote!(#pattern => #key));
            keys.push(key);
            continue;
        }
        // No message of its own: the wrapping shape delegates to what it wraps,
        // and the wrapped enum's own `MESSAGE_KEYS` covers the keys it can
        // produce.
        match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let inner = format_ident!("inner");
                arms.push(quote!(Self::#variant_name(#inner) => #inner.message_key()));
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    variant,
                    format!(
                        "`{name}::{variant_name}` has no string: add `{key}` to \
                         product/i18n/locales/en, or give the variant a single field to \
                         delegate to"
                    ),
                ));
            }
        }
    }

    // Naming the source files makes cargo rebuild the deriving crate when a
    // message changes, which is what keeps the check above honest.
    let tracked = files.iter().map(|file| {
        let path = file.to_string_lossy().into_owned();
        quote!(
            const _: &str = ::core::include_str!(#path);
        )
    });

    Ok(quote! {
        #(#tracked)*

        impl #name {
            /// Every Fluent message id this enum names itself, in variant order.
            ///
            /// A variant that delegates to what it wraps contributes nothing
            /// here; the wrapped enum lists its own keys.
            pub const MESSAGE_KEYS: &'static [&'static str] = &[#(#keys),*];

            /// The Fluent message id for this variant (ADR 0022).
            ///
            /// Derived from the enum and variant names and checked against the
            /// string source while this crate compiled: it is a key, never text.
            #[must_use]
            pub const fn message_key(self) -> &'static str {
                match self {
                    #(#arms,)*
                }
            }
        }
    })
}

/// A variant has to supply every argument its message interpolates.
fn check_arguments(
    variant: &syn::Variant,
    key: &str,
    arguments: &BTreeSet<String>,
) -> syn::Result<()> {
    let supplied: BTreeSet<String> = match &variant.fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .filter_map(|field| field.ident.as_ref().map(|ident| camel(&ident.to_string())))
            .collect(),
        Fields::Unit | Fields::Unnamed(_) => BTreeSet::new(),
    };
    let missing: Vec<&String> = arguments.difference(&supplied).collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            variant,
            format!(
                "`{key}` interpolates {missing:?}, which this variant has no field for; \
                 name a field after each argument or take it out of the message"
            ),
        ))
    }
}

/// Every message id in the default locale, with the arguments it interpolates,
/// beside the files it was read from.
type Source = (Vec<PathBuf>, BTreeMap<String, BTreeSet<String>>);

/// Read the default locale off disk while this macro expands.
fn read_source() -> Result<Source, String> {
    let directory = Path::new(LOCALES);
    let mut files: Vec<PathBuf> = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ftl"))
        .collect();
    files.sort();
    let sources: Vec<(String, String)> = files
        .iter()
        .map(|file| {
            let name = file
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned();
            fs::read_to_string(file)
                .map(|text| (name, text))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<_, _>>()?;
    let borrowed: Vec<(&str, &str)> = sources
        .iter()
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    let locale = arut_i18n_catalog::parse("en", &borrowed).map_err(|refusals| {
        refusals
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let messages = locale
        .messages
        .into_iter()
        .map(|(id, message)| {
            let arguments = message
                .arguments()
                .into_iter()
                .map(|argument| argument.name)
                .collect();
            (id, arguments)
        })
        .collect();
    Ok((files, messages))
}

/// `ComposerError` -> `composer-error`, `TimedOut` -> `timed-out`.
fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (index, character) in name.char_indices() {
        if character.is_ascii_uppercase() {
            if index != 0 {
                out.push('-');
            }
            out.push(character.to_ascii_lowercase());
        } else {
            out.push(character);
        }
    }
    out
}

/// `current_epoch` -> `currentEpoch`, the shape a Fluent argument takes.
fn camel(name: &str) -> String {
    let mut parts = name.split('_');
    let first = parts.next().unwrap_or_default().to_owned();
    parts.fold(first, |mut out, part| {
        let mut characters = part.chars();
        if let Some(initial) = characters.next() {
            out.extend(initial.to_uppercase());
            out.push_str(characters.as_str());
        }
        out
    })
}

#[cfg(test)]
mod tests {
    use super::{camel, kebab, read_source};

    #[test]
    fn names_become_the_key_convention() {
        assert_eq!(kebab("ComposerError"), "composer-error");
        assert_eq!(kebab("TimedOut"), "timed-out");
        assert_eq!(kebab("ChatIdMissing"), "chat-id-missing");
        assert_eq!(camel("current_epoch"), "currentEpoch");
        assert_eq!(camel("current"), "current");
    }

    #[test]
    fn the_string_source_is_where_this_crate_says_it_is() {
        let (files, messages) = read_source().expect("the default locale is readable");
        assert!(!files.is_empty());
        assert!(messages.contains_key("chat-error-cancelled"));
        assert!(
            messages["composer-error-revision-conflict"].contains("current"),
            "the argument of a message is read off its pattern"
        );
    }
}
