//! Repository development commands.
//!
//! `generate` writes the localization resources each platform's own API reads,
//! from the one Fluent source (ADR 0022). `check` proves the repository still
//! matches ARCHITECTURE.md: layer edges, the isolated `bindings/ffi` graph,
//! TypeScript package boundaries, the generated resources, and the Windows
//! `x:Uid` references. Mise calls both, and CI calls the same mise tasks.

mod check;
mod i18n;

use std::error::Error;
use std::process::ExitCode;

fn main() -> ExitCode {
    let command = std::env::args().nth(1);
    let result: Result<(), Box<dyn Error>> = match command.as_deref() {
        Some("generate") => i18n::generate().map_err(Into::into),
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
