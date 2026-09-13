//! Enforces ARCHITECTURE.md on declared edges and isolated production graphs.
//! Includes optional, target-specific, build, and development dependency edges.
//! Cargo tree isolates feature resolution per core root, avoiding workspace-wide
//! Tokio feature unification. Compiler lints and parsed attributes guard unsafe.
use quote::ToTokens;
use serde_json::Value;
use std::{collections::BTreeSet, error::Error, fs, path::Path, process::Command};
use syn::visit::Visit;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn cargo(args: &[&str]) -> Result<String> {
    let output = Command::new("cargo").args(args).output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn allowed(from: &str, to: &str) -> bool {
    let layer = to.split('/').next().unwrap_or(to);
    match from.split('/').next().unwrap_or(from) {
        "features" => matches!(layer, "substrates" | "protocols"),
        "product" => matches!(layer, "product" | "features" | "substrates" | "protocols"),
        "substrates" => !matches!(layer, "features" | "product"),
        "transports" => to == "protocols/rpc",
        "bindings" if from == "bindings/ffi" => {
            matches!(layer, "product" | "features" | "substrates")
        }
        "runtimes" => layer != "surfaces",
        "surfaces" => matches!(layer, "runtimes" | "product" | "bindings"),
        "protocols" => layer == "protocols",
        "backend" => matches!(layer, "protocols" | "substrates" | "transports"),
        "tools" => true,
        _ => false,
    }
}

fn exception(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        // ADR 0022: compile-time Fluent validation emits only string keys, no product types.
        ("features/chat", "product/i18n/macros") |
        // IPC is Connect over a Unix connector; sharing framing avoids a second protocol implementation.
        ("transports/ipc", "transports/connect-http") |
        // FFI observation polls watch invalidations and bridges callbacks, not product work.
        ("bindings/ffi", "futures-util" | "futures-executor")
    )
}

fn core(path: &str) -> bool {
    ["features/", "product/", "substrates/", "bindings/ffi"]
        .iter()
        .any(|p| path.starts_with(p))
}

fn denied(value: &toml::Value) -> bool {
    matches!(
        value.as_str().or_else(|| value.get("level")?.as_str()),
        Some("deny" | "forbid")
    )
}

struct Attributes<'a> {
    file: &'a Path,
    violations: &'a mut BTreeSet<String>,
}
impl<'ast> Visit<'ast> for Attributes<'_> {
    fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
        if !["allow", "warn", "expect", "cfg_attr"]
            .iter()
            .any(|p| attr.path().is_ident(p))
        {
            return;
        }
        let tokens = attr.meta.to_token_stream().to_string();
        if tokens.contains("unsafe_code")
            && ["allow", "warn", "expect"]
                .iter()
                .any(|s| tokens.contains(s))
        {
            self.violations.insert(format!(
                "{} -> {tokens}: unsafe_code must remain denied",
                self.file.display()
            ));
        }
    }
}
fn sources(dir: &Path, violations: &mut BTreeSet<String>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() && path.join("Cargo.toml").exists() {
            continue;
        }
        if path.is_dir()
            && !matches!(
                path.file_name().and_then(|n| n.to_str()),
                Some("target" | "node_modules" | ".git")
            )
        {
            sources(&path, violations)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let parsed = syn::parse_file(&fs::read_to_string(&path)?)?;
            Attributes {
                file: &path,
                violations,
            }
            .visit_file(&parsed);
        }
    }
    Ok(())
}

fn check() -> Result<BTreeSet<String>> {
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
    let mut violations = BTreeSet::new();
    for package in packages.iter().filter(|p| members.contains(&p["id"])) {
        let manifest = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("missing manifest")?,
        );
        let dir = manifest.parent().ok_or("missing crate directory")?;
        let from = dir
            .strip_prefix(root)?
            .to_str()
            .ok_or("non-UTF8 crate path")?;
        for dep in package["dependencies"]
            .as_array()
            .ok_or("missing dependencies")?
        {
            let name = dep["name"].as_str().ok_or("missing dependency name")?;
            let local = dep["path"].as_str().map(Path::new);
            let to = local
                .and_then(|p| p.strip_prefix(root).ok())
                .and_then(|p| p.to_str())
                .unwrap_or(name);
            let permitted = if local.is_some() {
                allowed(from, to)
            } else {
                from != "bindings/ffi" || name == "boltffi"
            };
            let exempt = exception(from, to) && (to != "futures-executor" || dep["kind"] == "dev");
            if !permitted && (!exempt || std::env::args().any(|a| a == "--strict")) {
                violations.insert(format!("{from} -> {to}: forbidden dependency"));
            }
        }
        let config: toml::Value = toml::from_str(&fs::read_to_string(manifest)?)?;
        let lint = if config
            .get("lints")
            .and_then(|v| v.get("workspace"))
            .and_then(toml::Value::as_bool)
            == Some(true)
        {
            workspace.get("workspace").and_then(|v| v.get("lints"))
        } else {
            config.get("lints")
        };
        if !lint
            .and_then(|v| v.get("rust"))
            .and_then(|v| v.get("unsafe_code"))
            .is_some_and(denied)
        {
            violations.insert(format!("{from} -> unsafe_code: missing deny/forbid lint"));
        }
        sources(dir, &mut violations)?;
        if core(from) {
            let name = package["name"].as_str().ok_or("missing package name")?;
            let graph = cargo(&[
                "tree",
                "-p",
                name,
                "--edges",
                "normal,build",
                "--target",
                "all",
                "--prefix",
                "none",
                "--format",
                "{p} {f}",
            ])?;
            for line in graph.lines() {
                let mut words = line.split_whitespace();
                let dependency = words.next().unwrap_or("");
                let _version = words.next();
                let features = words.next().unwrap_or("");
                if dependency == "wasm-bindgen"
                    || (dependency == "tokio" && features.split(',').any(|f| f == "rt"))
                {
                    violations.insert(format!(
                        "{from} -> {line}: forbidden isolated production dependency"
                    ));
                }
            }
        }
    }
    Ok(violations)
}
fn main() -> Result<()> {
    let violations = check()?;
    for violation in &violations {
        eprintln!("{violation}");
    }
    if !violations.is_empty() {
        std::process::exit(1);
    }
    println!("Layer boundaries, isolated core graphs, and unsafe-code lints passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_and_exceptions_are_narrow() {
        for (from, to) in [
            ("features/new", "product/i18n"),
            ("product/session", "runtimes/local"),
            ("substrates/watch", "features/chat"),
            ("transports/ipc", "product/session"),
            ("bindings/ffi", "protocols/rpc"),
            ("surfaces/new", "transports/ipc"),
            ("runtimes/local", "surfaces/new"),
        ] {
            assert!(!allowed(from, to) && !exception(from, to), "{from} -> {to}");
        }
        assert!(allowed("features/new", "protocols/rpc"));
        assert!(allowed("tools/new", "surfaces/new"));
        assert!(exception("transports/ipc", "transports/connect-http"));
        assert!(!exception("transports/new", "transports/connect-http"));
    }
}
