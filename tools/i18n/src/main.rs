//! Turn `product/i18n/locales/*.ftl` into each platform's native resources.
//!
//! One source, two kinds of output (ADR 0022). The resources each platform's
//! localization API reads: Apple's string catalog, Android's `strings.xml` per
//! locale, a Windows `.resw` per locale, and a served copy of the raw Fluent for
//! the surfaces that read it with `@fluent/bundle`. And a typed accessor per
//! message for every consumer -- a Rust `Message` enum, `L10n` in Swift, Kotlin
//! and C#, and `t` in TypeScript -- so no message id is ever typed by hand.
//!
//! The `.ftl` files reach this binary through `arut_i18n`'s own embedded copies,
//! so the generator and the Rust runtime can never read different sources.
//!
//! Run it with `mise run i18n`. It is deterministic and idempotent: `mise run
//! check` runs it and fails if any generated file changed, which is the whole
//! enforcement mechanism behind "the `.ftl` is the source".

mod accessors;
mod catalog;
mod targets;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs, io};

use arut_i18n::{DEFAULT_LOCALE, available_locales, locale_resources};

/// Where each output lands, relative to the repository root.
const XCSTRINGS: &str = "surfaces/apple/shared/Sources/ArutSurface/Resources/Localizable.xcstrings";
const ANDROID_RES: &str = "surfaces/android/src/main/res";
const WINDOWS_STRINGS: &str = "surfaces/windows/Strings";
/// The web surface serves these and the Chromium extension packages the same
/// entry point, so it needs its own copy; the VS Code extension ships one too,
/// because its host resolves errors before the webview ever sees them.
const FLUENT_COPIES: [&str; 3] = [
    "surfaces/web/public/locales",
    "surfaces/chromium/public/locales",
    "surfaces/vscode/locales",
];
/// The typed accessors, one file per consumer.
const RUST_MESSAGES: &str = "product/i18n/src/generated.rs";
const SWIFT_L10N: &str = "surfaces/apple/shared/Sources/ArutSurface/Generated/L10n.swift";
const KOTLIN_PACKAGE: &str = "dev.arut.surface";
const KOTLIN_L10N: &str = "surfaces/android/src/main/kotlin/dev/arut/surface/generated/L10n.kt";
const CSHARP_L10N: &str = "surfaces/windows/Generated/L10n.cs";
const TYPESCRIPT_L10N: &str = "bindings/typescript/src/generated/l10n.ts";

fn main() -> ExitCode {
    let root = match env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => repository_root(),
    };
    match generate(&root) {
        Ok(written) => {
            for path in written {
                println!("{}", path.display());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// The repository this binary was compiled inside, so `mise run i18n` needs no
/// argument and no working directory.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("tools/i18n sits two levels below the repository root")
        .to_path_buf()
}

fn generate(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut locales = Vec::new();
    let mut refusals = Vec::new();
    for tag in available_locales() {
        match catalog::parse(tag, locale_resources(tag)) {
            Ok(locale) => locales.push(locale),
            Err(mut refused) => refusals.append(&mut refused),
        }
    }
    if !refusals.is_empty() {
        let listed = refusals
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!(
            "{} message(s) cannot be generated:\n{listed}",
            refusals.len()
        ));
    }

    // A typed accessor compiles against one key set, so every locale has to
    // define the same ids before a single file is written.
    catalog::require_identical_key_sets(&locales)?;

    let mut written = Vec::new();
    write(
        root.join(RUST_MESSAGES),
        &accessors::rust(DEFAULT_LOCALE, &locales),
        &mut written,
    )?;
    write(
        root.join(SWIFT_L10N),
        &accessors::swift(DEFAULT_LOCALE, &locales),
        &mut written,
    )?;
    write(
        root.join(KOTLIN_L10N),
        &accessors::kotlin(DEFAULT_LOCALE, &locales, KOTLIN_PACKAGE),
        &mut written,
    )?;
    write(
        root.join(CSHARP_L10N),
        &accessors::csharp(DEFAULT_LOCALE, &locales),
        &mut written,
    )?;
    write(
        root.join(TYPESCRIPT_L10N),
        &accessors::typescript(DEFAULT_LOCALE, &locales),
        &mut written,
    )?;
    write(
        root.join(XCSTRINGS),
        &targets::xcstrings(DEFAULT_LOCALE, &locales),
        &mut written,
    )?;
    for locale in &locales {
        let values = if locale.tag == DEFAULT_LOCALE {
            "values".to_owned()
        } else {
            format!("values-{}", locale.tag)
        };
        write(
            root.join(ANDROID_RES).join(values).join("strings.xml"),
            &targets::strings_xml(locale),
            &mut written,
        )?;
        write(
            root.join(WINDOWS_STRINGS)
                .join(&locale.tag)
                .join("Resources.resw"),
            &targets::resw(locale),
            &mut written,
        )?;
    }

    // The Fluent copies are wholesale: removing the tree first is what makes a
    // deleted locale actually disappear from the served files.
    for destination in FLUENT_COPIES {
        let destination = root.join(destination);
        remove(&destination)?;
        for tag in available_locales() {
            for (file, source) in locale_resources(tag) {
                write(destination.join(tag).join(file), source, &mut written)?;
            }
        }
        let manifest = format!(
            "{{\n  \"locales\": [{}],\n  \"files\": [{}]\n}}\n",
            quoted(available_locales()),
            quoted(
                locale_resources(DEFAULT_LOCALE)
                    .iter()
                    .map(|(file, _)| *file)
            ),
        );
        write(destination.join("locales.json"), &manifest, &mut written)?;
    }
    written.sort();
    Ok(written)
}

fn quoted<'a>(values: impl Iterator<Item = &'a str>) -> String {
    values
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Write only when the bytes differ, so a no-op run leaves every mtime alone.
fn write(path: PathBuf, contents: &str, written: &mut Vec<PathBuf>) -> Result<(), String> {
    let parent = path.parent().expect("every output has a directory");
    fs::create_dir_all(parent).map_err(|error| failed("create", parent, &error))?;
    let unchanged = fs::read_to_string(&path).is_ok_and(|existing| existing == contents);
    if !unchanged {
        fs::write(&path, contents).map_err(|error| failed("write", &path, &error))?;
    }
    written.push(path);
    Ok(())
}

fn remove(path: &Path) -> Result<(), String> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(failed("remove", path, &error)),
    }
}

fn failed(action: &str, path: &Path, error: &io::Error) -> String {
    format!("could not {action} {}: {error}", path.display())
}

#[cfg(test)]
mod tests {
    use super::{generate, repository_root};

    #[test]
    fn the_shipped_locales_all_generate() {
        let root = tempdir();
        let written = generate(&root).expect("the shipped .ftl files are generatable");
        assert!(
            written
                .iter()
                .any(|path| path.ends_with("Localizable.xcstrings"))
        );
        assert!(
            written
                .iter()
                .any(|path| path.ends_with("values/strings.xml"))
        );
        assert!(
            written
                .iter()
                .any(|path| path.ends_with("en/Resources.resw"))
        );
        assert!(written.iter().any(|path| path.ends_with("en/errors.ftl")));
        assert!(written.iter().any(|path| path.ends_with("generated.rs")));
        assert!(written.iter().any(|path| path.ends_with("L10n.swift")));
        assert!(written.iter().any(|path| path.ends_with("L10n.kt")));
        assert!(written.iter().any(|path| path.ends_with("L10n.cs")));
        assert!(written.iter().any(|path| path.ends_with("l10n.ts")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_second_run_produces_the_same_bytes() {
        let root = tempdir();
        let first = generate(&root).expect("first run");
        let before: Vec<String> = first
            .iter()
            .map(|path| std::fs::read_to_string(path).expect("written"))
            .collect();
        let second = generate(&root).expect("second run");
        assert_eq!(first, second);
        let after: Vec<String> = second
            .iter()
            .map(|path| std::fs::read_to_string(path).expect("written"))
            .collect();
        assert_eq!(before, after);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_repository_root_is_the_one_holding_the_source() {
        assert!(repository_root().join("product/i18n/locales").is_dir());
    }

    fn tempdir() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "arut-i18n-gen-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a temporary directory");
        root
    }
}
