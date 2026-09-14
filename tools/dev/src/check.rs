//! The repository checks ARCHITECTURE.md asks for, in one process.
//!
//! Five rules, each one the compiler cannot state on its own:
//!
//! - every local dependency edge satisfies the layer table;
//! - `bindings/ffi` takes no external crate but BoltFFI and `async-task`, so the
//!   wasm core stays the shape `docs/ECOSYSTEM.md` requires;
//! - a relative TypeScript import stays inside its surface or runtime package;
//! - the generated localization resources match `product/i18n/locales`, the
//!   generated FFI handles match `bindings/ffi/handles.toml`, and every `x:Uid`
//!   in the WinUI XAML has entries in the generated `.resw`;
//! - no surface subscribes to a revision stream by hand instead of using its
//!   binding package's observation helper.
//!
//! What used to be here and is not: the rule that `runtimes/` and `product/`
//! may not name a feature service implementation, which Rust privacy already
//! enforces (`pub(crate) struct ChatServiceImpl`); the scan for
//! `allow(unsafe_code)` and the per-crate `unsafe_code` declaration, which the
//! workspace lint table states once and `mise run check:core-graph` greps; and
//! the resolved-graph walk for `wasm-bindgen` and Tokio executors, which is
//! `cargo tree` in the same task.

use std::collections::BTreeSet;
use std::error::Error;
use std::path::Path;
use std::process::Command;
use std::{fs, io};

use serde_json::Value;

use crate::output::{Run, repository_root};
use crate::{handles, i18n};

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
                // `bindings/ffi` is the wasm core's root, so its outside edges
                // are named one by one: BoltFFI, and `async-task` for the
                // observation driver, which is `no_std`, pulls in no bindgen
                // and no executor, and replaces a hand-written waker.
                from != "bindings/ffi" || matches!(name, "boltffi" | "async-task")
            };
            let exempt =
                exception(from, to) && (to != "futures-executor" || dependency["kind"] == "dev");
            if !permitted && !exempt {
                violations.insert(format!("{from} -> {to}: forbidden dependency"));
            }
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

/// The observation helper a surface of this kind must use instead of
/// subscribing by hand. Named in the message, because "do not write this" is
/// only useful beside "write that".
fn helper(file: &str) -> Option<&'static str> {
    match Path::new(file).extension().and_then(|e| e.to_str())? {
        "swift" => Some("ArutBindings.Projection"),
        "kt" => Some("dev.arut.bindings.projection"),
        "cs" => Some("Arut.Bindings.Projection<T>"),
        "ts" | "tsx" => Some("observe from @arut/bindings-typescript"),
        _ => None,
    }
}

/// Every way each ecosystem writes "await the next element of this stream".
const SUBSCRIBE: [&str; 3] = ["for await", "await foreach", ".collect {"];

/// A surface renders; it does not run the subscribe-and-read loop. Every
/// ecosystem has one generic helper over a `*Changes()` stream in its binding
/// package (ADR 0021), and a loop in a surface is the same kind of defect a
/// hand-written FFI type is.
///
/// `surfaces/linux` is excluded: it is Rust, it consumes the watch directly
/// rather than an FFI stream, and its GLib observer is moving to a Rust-surface
/// binding module of its own.
fn surface_subscriptions(root: &Path, files: &[String], violations: &mut BTreeSet<String>) {
    for file in files {
        if !file.starts_with("surfaces/") || file.starts_with("surfaces/linux/") {
            continue;
        }
        let Some(helper) = helper(file) else {
            continue;
        };
        let Ok(source) = fs::read_to_string(root.join(file)) else {
            continue;
        };
        let lines: Vec<&str> = source.lines().collect();
        for (index, text) in lines.iter().enumerate() {
            let window = format!("{text}{}", lines.get(index + 1).unwrap_or(&""));
            let subscribes = SUBSCRIBE.iter().any(|form| text.contains(form));
            let stream = window.contains("Changes(") || window.contains("_changes(");
            if subscribes && stream {
                violations.insert(format!(
                    "{file}:{} -> a revision stream is subscribed to in a surface; \
                     observe it through {helper}",
                    index + 1
                ));
            }
        }
    }
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
    let root = repository_root();
    let mut violations = BTreeSet::new();
    let files = tracked_files(&root)?;
    crates(&mut violations)?;
    typescript_imports(&root, &files, &mut violations);
    surface_subscriptions(&root, &files, &mut violations);
    windows_uids(&root, &mut violations)?;
    for path in i18n::run(&root, Run::Check)?.stale {
        violations.insert(format!(
            "{}: does not match product/i18n/locales; run `mise run generate`",
            path.display()
        ));
    }
    for path in handles::run(&root, Run::Check)?.stale {
        violations.insert(format!(
            "{}: does not match {}; run `mise run generate`",
            path.display(),
            handles::DECLARATION
        ));
    }
    for violation in &violations {
        eprintln!("{violation}");
    }
    if !violations.is_empty() {
        return Err(format!("{} check(s) failed", violations.len()).into());
    }
    println!(
        "Layer boundaries, the isolated bindings/ffi graph, TypeScript package boundaries, \
         surface revision observation, generated localization resources and FFI handles, \
         and Windows x:Uid references passed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{allowed, crosses_package, exception, helper, surface_subscriptions};

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
    fn a_surface_may_not_run_the_subscribe_and_read_loop_itself() {
        let root = std::env::temp_dir().join(format!("arut-dev-check-{}", std::process::id()));
        let file = "surfaces/example/View.swift";
        std::fs::create_dir_all(root.join("surfaces/example")).unwrap();
        std::fs::write(
            root.join(file),
            "for await _ in handle.chatChanges() {\n  state = handle.state()\n}\n",
        )
        .unwrap();
        let mut violations = std::collections::BTreeSet::new();
        surface_subscriptions(&root, &[file.to_owned()], &mut violations);
        let reported = violations.iter().next().expect("one violation");
        assert!(reported.contains("View.swift:1"), "{reported}");
        assert!(reported.contains("ArutBindings.Projection"), "{reported}");

        std::fs::write(root.join(file), "state = projection.value\n").unwrap();
        let mut clean = std::collections::BTreeSet::new();
        surface_subscriptions(&root, &[file.to_owned()], &mut clean);
        assert!(clean.is_empty());
        assert_eq!(
            helper("surfaces/x/A.kt"),
            Some("dev.arut.bindings.projection")
        );
        assert_eq!(helper("surfaces/linux/src/app/mod.rs"), None);
        let _ = std::fs::remove_dir_all(&root);
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
