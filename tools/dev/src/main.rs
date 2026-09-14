//! Repository development commands.
//!
//! `generate` writes what two declarations imply: the localization resources
//! each platform's own API reads, from the one Fluent source (ADR 0022), and
//! the `#[export]` blocks BoltFFI scans, from `bindings/ffi/handles.toml`
//! (ADR 0021). `check` proves the repository still
//! matches ARCHITECTURE.md: layer edges, the isolated `bindings/ffi` graph,
//! TypeScript package boundaries, the generated resources, and the Windows
//! `x:Uid` references. Mise calls both, and CI calls the same mise tasks.

mod check;
mod handles;
mod i18n;
mod output;

use std::error::Error;
use std::process::ExitCode;

/// Write every generated file and name the ones that landed.
fn generate() -> Result<(), String> {
    let root = output::repository_root();
    let outcomes = [
        i18n::run(&root, output::Run::Write)?,
        handles::run(&root, output::Run::Write)?,
    ];
    for path in outcomes.into_iter().flat_map(|outcome| outcome.written) {
        println!("{}", path.display());
    }
    Ok(())
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1);
    let result: Result<(), Box<dyn Error>> = match command.as_deref() {
        Some("generate") => generate().map_err(Into::into),
        Some("check") => check::run(),
        _ => Err("usage: arut-dev <generate|check>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
