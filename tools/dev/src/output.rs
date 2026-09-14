//! Where a generated file goes, and whether this run writes it or reports it.
//!
//! Both generators -- the localization resources and the FFI handle blocks --
//! check their output in and have `arut-dev check` regenerate and diff, so both
//! need the same two modes and the same "write only when the bytes differ"
//! rule.

use std::path::{Path, PathBuf};
use std::{fs, io};

/// Whether a run writes its output or only reports what would change.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Run {
    Write,
    Check,
}

/// What one run produced: every output path, and the ones a check found out of
/// date.
pub(crate) struct Outcome {
    pub written: Vec<PathBuf>,
    pub stale: Vec<PathBuf>,
}

/// The repository this binary was compiled inside, so the commands need no
/// argument and no working directory.
pub(crate) fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("tools/dev sits two levels below the repository root")
        .to_path_buf()
}

/// Write only when the bytes differ, so a no-op run leaves every mtime alone;
/// under [`Run::Check`], record the difference instead of writing it.
pub(crate) fn write(
    path: PathBuf,
    contents: &str,
    written: &mut Vec<PathBuf>,
    stale: &mut Vec<PathBuf>,
    run: Run,
) -> Result<(), String> {
    let unchanged = fs::read_to_string(&path).is_ok_and(|existing| existing == contents);
    if !unchanged {
        if run == Run::Check {
            stale.push(path);
            return Ok(());
        }
        let parent = path.parent().expect("every output has a directory");
        fs::create_dir_all(parent).map_err(|error| failed("create", parent, &error))?;
        fs::write(&path, contents).map_err(|error| failed("write", &path, &error))?;
    }
    written.push(path);
    Ok(())
}

fn failed(action: &str, path: &Path, error: &io::Error) -> String {
    format!("could not {action} {}: {error}", path.display())
}
