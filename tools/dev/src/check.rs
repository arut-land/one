//! The repository checks ARCHITECTURE.md asks for, in one process.
//!
//! Five rules, each one the compiler cannot state on its own:
//!
//! - every local dependency edge satisfies the layer table;
//! - `bindings/ffi` takes no external crate but BoltFFI, so the wasm core stays
//!   the shape `docs/ECOSYSTEM.md` requires;
//! - every crate denies `unsafe_code`, whether through the workspace lints or
//!   its own table;
//! - a relative TypeScript import stays inside its surface or runtime package;
//! - the generated localization resources match `product/i18n/locales`, and
//!   every `x:Uid` in the WinUI XAML has entries in the generated `.resw`.
//!
//! What used to be here and is not: the rule that `runtimes/` and `product/`
//! may not name a feature service implementation, which Rust privacy already
//! enforces (`pub(crate) struct ChatServiceImpl`); the scan for
//! `allow(unsafe_code)`, which is a grep in `mise run check:core-graph`; and
//! the resolved-graph walk for `wasm-bindgen` and Tokio executors, which is
//! `cargo tree` in the same task.

use std::collections::BTreeSet;
use std::error::Error;
use std::path::Path;
use std::process::Command;
use std::{fs, io};

use serde_json::Value;

use crate::i18n;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The layer table from ARCHITECTURE.md: which layer each layer may depend on.
fn allowed(from: &str, to: &str) -> bool {
    let layer = to.split('/').next().unwrap_or(to);
    match from.split('/').next().unwrap_or(from) {
        "features" => matches!(layer, "substrates" | "protocols"),
        "product" => matches!(layer, "product" | "features" | "substrates" | "protocols"),
        "substrates" => matches!(layer, "substrates" | "protocols"),
        "transports" => to == "protocols/rpc",
        "bindings" if from == "bindings/ffi" => {
            matches!(layer, "product" | "features" | "substrates")
        }
        "runtimes" => matches!(
            layer,
            "runtimes" | "features" | "product" | "substrates" | "protocols" | "transports"
        ),
        "surfaces" => matches!(layer, "runtimes" | "product" | "bindings"),
        "protocols" => layer == "protocols",
        "backend" => matches!(layer, "protocols" | "substrates" | "transports"),
        "tools" => true,
        _ => false,
    }
}

/// The edges the table refuses and the architecture allows anyway.
fn exception(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        // ADR 0022: the derive validates Fluent keys while a feature crate
        // compiles and expands to `&'static str` alone. A proc-macro edge is
        // build-time only, so nothing above `features/` reaches its runtime
        // graph, and `product/i18n/macros -> product/i18n/catalog` is an
        // ordinary product edge the table already allows.
        ("features/chat", "product/i18n/macros") |
        // The FFI factory root supplies memory ports; observation stays in the
        // runtime. futures-executor is restricted below to test-only polling.
        ("bindings/ffi", "runtimes/host-polled" | "futures-executor")
    )
}

fn denied(value: &toml::Value) -> bool {
    matches!(
        value.as_str().or_else(|| value.get("level")?.as_str()),
        Some("deny" | "forbid")
    )
}

fn cargo(args: &[&str]) -> Result<String> {
    let output = Command::new("cargo").args(args).output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// Walk every workspace member's declared dependencies and lint table.
fn crates(violations: &mut BTreeSet<String>) -> Result<()> {
    let metadata: Value = serde_json::from_str(&cargo(&["metadata", "--format-version", "1"])?)?;
    let root = Path::new(
        metadata["workspace_root"]
            .as_str()
            .ok_or("missing workspace root")?,
    );
    let workspace: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)?;
    let packages = metadata["packages"].as_array().ok_or("missing packages")?;
    let members = metadata["workspace_members"]
        .as_array()
        .ok_or("missing members")?;
    for package in packages.iter().filter(|p| members.contains(&p["id"])) {
        let manifest = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("missing manifest")?,
        );
        let directory = manifest.parent().ok_or("missing crate directory")?;
        let from = directory
            .strip_prefix(root)?
            .to_str()
            .ok_or("non-UTF8 crate path")?
            .replace('\\', "/");
        let from = from.as_str();
        for dependency in package["dependencies"]
            .as_array()
            .ok_or("missing dependencies")?
        {
            let name = dependency["name"]
                .as_str()
                .ok_or("missing dependency name")?;
            let local = dependency["path"].as_str().map(Path::new);
            let to = local
                .and_then(|path| path.strip_prefix(root).ok())
                .and_then(|path| path.to_str())
                .unwrap_or(name)
                .replace('\\', "/");
            let to = to.as_str();
            let permitted = if local.is_some() {
                allowed(from, to)
            } else {
                // `bindings/ffi` is the wasm core's root: BoltFFI is the only
                // crate allowed to reach it from outside the workspace.
                from != "bindings/ffi" || name == "boltffi"
            };
            let exempt =
                exception(from, to) && (to != "futures-executor" || dependency["kind"] == "dev");
            if !permitted && !exempt {
                violations.insert(format!("{from} -> {to}: forbidden dependency"));
            }
        }
        let manifest_config: toml::Value = toml::from_str(&fs::read_to_string(manifest)?)?;
        let lints = if manifest_config
            .get("lints")
            .and_then(|value| value.get("workspace"))
            .and_then(toml::Value::as_bool)
            == Some(true)
        {
            workspace
                .get("workspace")
                .and_then(|value| value.get("lints"))
        } else {
            manifest_config.get("lints")
        };
        if !lints
            .and_then(|value| value.get("rust"))
            .and_then(|value| value.get("unsafe_code"))
            .is_some_and(denied)
        {
            violations.insert(format!("{from} -> unsafe_code: missing deny/forbid lint"));
        }
    }
    Ok(())
}

/// A relative source path must stay within its surface or runtime package.
fn crosses_package(file: &str, specifier: &str) -> bool {
    if !specifier.starts_with("./") && !specifier.starts_with("../") {
        return false;
    }
    let mut path: Vec<_> = file.split('/').collect();
    path.pop();
    let owner: Vec<_> = path.iter().take(2).copied().collect();
    if !matches!(owner.first(), Some(&"surfaces" | &"runtimes")) {
        return false;
    }
    for component in specifier.split('/') {
        match component {
            "." => {}
            ".." => {
                path.pop();
            }
            component => path.push(component),
        }
    }
    !path.starts_with(&owner)
}

fn tracked_files(root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err("git ls-files failed".into());
    }
    Ok(String::from_utf8(output.stdout)?
        .split('\0')
        .filter(|file| !file.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn typescript_imports(root: &Path, files: &[String], violations: &mut BTreeSet<String>) {
    for file in files {
        if !matches!(
            Path::new(file).extension().and_then(|e| e.to_str()),
            Some("ts" | "tsx")
        ) {
            continue;
        }
        let Ok(source) = fs::read_to_string(root.join(file)) else {
            continue;
        };
        for (line, text) in source.lines().enumerate() {
            for specifier in text.split(['\'', '"', '`']).skip(1).step_by(2) {
                if crosses_package(file, specifier) {
                    violations.insert(format!(
                        "{file}:{} -> {specifier}: import through a workspace package",
                        line + 1
                    ));
                }
            }
        }
    }
}

/// Every `x:Uid` the WinUI XAML names has to exist in the generated `.resw`,
/// or WinUI resolves nothing and the control renders empty.
fn windows_uids(root: &Path, violations: &mut BTreeSet<String>) -> Result<()> {
    let defined = i18n::uid_names()?;
    for file in xaml_files(&root.join("surfaces/windows"))? {
        let relative = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .display()
            .to_string();
        let source = fs::read_to_string(&file)?;
        for (line, text) in source.lines().enumerate() {
            for uid in text
                .split("x:Uid=\"")
                .skip(1)
                .filter_map(|rest| rest.split('"').next())
            {
                if !defined.contains(uid) {
                    violations.insert(format!(
                        "{relative}:{} -> x:Uid=\"{uid}\": no such entry in surfaces/windows/Strings; \
                         add the message to product/i18n/locales under action-, label- or conversation-",
                        line + 1
                    ));
                }
            }
        }
    }
    Ok(())
}

fn xaml_files(directory: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(files),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(xaml_files(&path)?);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "xaml")
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

pub(crate) fn run() -> Result<()> {
    let root = i18n::repository_root();
    let mut violations = BTreeSet::new();
    crates(&mut violations)?;
    typescript_imports(&root, &tracked_files(&root)?, &mut violations);
    windows_uids(&root, &mut violations)?;
    for path in i18n::run(&root, i18n::Run::Check)?.stale {
        violations.insert(format!(
            "{}: does not match product/i18n/locales; run `mise run generate`",
            path.display()
        ));
    }
    for violation in &violations {
        eprintln!("{violation}");
    }
    if !violations.is_empty() {
        return Err(format!("{} check(s) failed", violations.len()).into());
    }
    println!(
        "Layer boundaries, unsafe-code lints, TypeScript package boundaries, localization resources, and Windows x:Uid references passed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{allowed, crosses_package, exception};

    #[test]
    fn boundaries_and_exceptions_are_narrow() {
        for (from, to) in [
            ("features/new", "product/i18n"),
            ("product/session", "runtimes/local"),
            ("substrates/watch", "features/chat"),
            ("substrates/storage", "runtimes/local"),
            ("substrates/watch", "surfaces/linux"),
            ("substrates/authority", "bindings/ffi"),
            ("substrates/watch", "tools/dev"),
            ("transports", "product/session"),
            ("bindings/ffi", "protocols/rpc"),
            ("surfaces/new", "transports"),
            ("runtimes/local", "surfaces/new"),
            ("runtimes/local", "tools/dev"),
            ("runtimes/local", "unknown/new"),
        ] {
            assert!(!allowed(from, to) && !exception(from, to), "{from} -> {to}");
        }
        assert!(allowed("features/new", "protocols/rpc"));
        assert!(allowed("product/i18n/macros", "product/i18n/catalog"));
        assert!(allowed("tools/new", "surfaces/new"));
        assert!(exception("features/chat", "product/i18n/macros"));
        assert!(!exception("features/new", "product/i18n/macros"));
    }

    #[test]
    fn relative_paths_cannot_escape_surface_or_runtime_packages() {
        for (file, specifier) in [
            ("surfaces/web/src/main.tsx", "../../../runtimes/browser/ids"),
            ("surfaces/chromium/src/main.tsx", "../../web/src/main"),
            ("runtimes/browser/ids.ts", "../../surfaces/web/src/main"),
        ] {
            assert!(crosses_package(file, specifier));
        }
        for specifier in ["./view", "../style.css", "@arut/runtime-browser"] {
            assert!(!crosses_package("surfaces/web/src/main.tsx", specifier));
        }
    }
}
