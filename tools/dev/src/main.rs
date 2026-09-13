//! Repository development commands for localization, layering, and binding facades.
//! Localization resources and accessors share their platform key naming.
//! Each subcommand supports `--check`; layers always checks without writing.
mod bindings;
mod i18n;
mod layers;

use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.first().map(String::as_str) {
        Some("i18n") => return i18n::run(&arguments[1..]),
        Some("bindings") => bindings::run(arguments.iter().any(|arg| arg == "--check")),
        Some("layers") => layers::run(),
        _ => Err("usage: arut-dev <i18n|layers|bindings> [--check]".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
