//! Enforces ARCHITECTURE.md on declared edges and isolated production graphs.
//! Includes optional, target-specific, build, and development dependency edges.
//! Cargo tree isolates feature resolution per core root, avoiding workspace-wide
//! Tokio feature unification. Parsed source guards unsafe lints and prevents
//! runtimes and product code from naming feature service implementations.
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
fn exception(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        // ADR 0022: compile-time Fluent validation emits only string keys, no product types.
        ("features/chat", "product/i18n/macros") |
        // IPC is Connect over a Unix connector; sharing framing avoids a second protocol implementation.
        ("transports/ipc", "transports/connect-http") |
        // The FFI factory root supplies memory ports; observation stays in the runtime.
        // futures-executor is restricted below to test-only polling.
        ("bindings/ffi", "runtimes/host-polled" | "futures-executor")
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
    layer: &'a str,
    file: &'a Path,
    violations: &'a mut BTreeSet<String>,
}
fn forbidden_implementation(layer: &str, identifier: &str) -> bool {
    (matches!(layer, "runtimes" | "product") && identifier.contains("ServiceImpl"))
        || (layer == "runtimes" && identifier.contains("Authority"))
}
impl Attributes<'_> {
    fn implementation(&mut self, identifier: &str) {
        if forbidden_implementation(self.layer, identifier) {
            self.violations.insert(format!(
                "{} -> {identifier}: feature composition belongs in features/",
                self.file.display()
            ));
        }
    }
}
impl<'ast> Visit<'ast> for Attributes<'_> {
    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        self.implementation(&ident.to_string());
    }
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        // Macro bodies are opaque to syn's visitor; inspect their tokens too.
        for identifier in mac
            .tokens
            .to_string()
            .split(|c: char| !c.is_alphanumeric() && c != '_')
        {
            self.implementation(identifier);
        }
        syn::visit::visit_macro(self, mac);
    }

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
fn sources(dir: &Path, layer: &str, violations: &mut BTreeSet<String>) -> Result<()> {
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
            sources(&path, layer, violations)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let parsed = syn::parse_file(&fs::read_to_string(&path)?)?;
            Attributes {
                layer,
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
            .ok_or("non-UTF8 crate path")?
            .replace('\\', "/");
        let from = from.as_str();
        for dep in package["dependencies"]
            .as_array()
            .ok_or("missing dependencies")?
        {
            let name = dep["name"].as_str().ok_or("missing dependency name")?;
            if core(from)
                && (name == "wasm-bindgen"
                    || (name == "tokio"
                        && dep["features"].as_array().is_some_and(|f| {
                            f.iter().any(|f| f == "rt" || f == "rt-multi-thread")
                        })))
            {
                violations.insert(format!(
                    "{from} -> {name}: forbidden declared core dependency"
                ));
            }
            let local = dep["path"].as_str().map(Path::new);
            let to = local
                .and_then(|p| p.strip_prefix(root).ok())
                .and_then(|p| p.to_str())
                .unwrap_or(name)
                .replace('\\', "/");
            let to = to.as_str();
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
        sources(dir, from.split('/').next().unwrap_or(from), &mut violations)?;
        if core(from) {
            let name = package["name"].as_str().ok_or("missing package name")?;
            let graph = cargo(&[
                "tree",
                "-p",
                name,
                "--all-features",
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
pub(crate) fn run() -> Result<()> {
    let violations = check()?;
    for violation in &violations {
        eprintln!("{violation}");
    }
    if !violations.is_empty() {
        std::process::exit(1);
    }
    println!(
        "Layer boundaries, feature composition ownership, isolated core graphs, and unsafe-code lints passed"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source_violations(layer: &str, source: &str) -> BTreeSet<String> {
        let mut violations = BTreeSet::new();
        Attributes {
            layer,
            file: Path::new("fixture.rs"),
            violations: &mut violations,
        }
        .visit_file(&syn::parse_file(source).unwrap());
        violations
    }

    #[test]
    fn implementation_names_cannot_hide_in_aliases_paths_or_macros() {
        for source in [
            "use feature::ChatServiceImpl as Hidden;",
            "type Hidden = feature::ComposerServiceImpl;",
            "macro_rules! construct { () => { feature::ChatServiceImpl }; }",
        ] {
            for layer in ["runtimes", "product"] {
                assert!(
                    !source_violations(layer, source).is_empty(),
                    "{layer}: {source}"
                );
            }
            assert!(source_violations("features", source).is_empty());
        }
        assert!(
            !source_violations("runtimes", "use feature::ComposerAuthority as Hidden;").is_empty()
        );
    }

    #[test]
    fn runtime_ports_and_composed_features_remain_allowed() {
        let source = "use feature::{compose, ChatRuntime, ChatFeature, ChatClients, ports::{IdSource, Persist, Drafts, Clock}};";
        assert!(source_violations("runtimes", source).is_empty());
        assert!(source_violations("product", "use feature::ChatClients;").is_empty());
        assert!(
            source_violations(
                "runtimes",
                "// ComposerAuthority is private.\nfn driver() {} "
            )
            .is_empty()
        );
    }

    #[test]
    fn boundaries_and_exceptions_are_narrow() {
        for (from, to) in [
            ("features/new", "product/i18n"),
            ("product/session", "runtimes/local"),
            ("substrates/watch", "features/chat"),
            ("substrates/storage", "runtimes/local"),
            ("substrates/storage", "transports/connect-http"),
            ("substrates/watch", "surfaces/linux"),
            ("substrates/authority", "bindings/ffi"),
            ("substrates/watch", "tools/dev"),
            ("transports/ipc", "product/session"),
            ("bindings/ffi", "protocols/rpc"),
            ("surfaces/new", "transports/ipc"),
            ("runtimes/local", "surfaces/new"),
            ("runtimes/local", "tools/dev"),
            ("runtimes/local", "unknown/new"),
        ] {
            assert!(!allowed(from, to) && !exception(from, to), "{from} -> {to}");
        }
        assert!(allowed("features/new", "protocols/rpc"));
        assert!(allowed("tools/new", "surfaces/new"));
        assert!(exception("transports/ipc", "transports/connect-http"));
        assert!(!exception("transports/new", "transports/connect-http"));
    }
}
