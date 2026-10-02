//! Refusing to rewrite files that git could not restore.
//!
//! `rename_files`, `group`, `convert` and `replace` change a tree in ways that
//! need review. Run over uncommitted work, their changes cannot be separated
//! from the user's or undone with git. `cargo fix` refuses for the same reason.

use std::path::{Path, PathBuf};
use std::process::Command;

/// `git status --porcelain` lines for changes under `paths`, including
/// untracked files, and ignored files where the run can reach them: a file
/// named directly, or any file when `walks_ignored`. Paths outside a work
/// tree, or with no `git` on PATH, are not checked. Any other git failure is
/// an error, since the tree could not be checked.
fn uncommitted(paths: &[PathBuf], walks_ignored: bool) -> anyhow::Result<Vec<String>> {
    let mut dirty = Vec::new();
    for path in paths {
        let is_dir = path.is_dir();
        let dir = if is_dir {
            path.as_path()
        } else {
            match path.parent() {
                Some(parent) if !parent.as_os_str().is_empty() => parent,
                _ => Path::new("."),
            }
        };
        let Ok(absolute) = std::path::absolute(path) else {
            continue;
        };
        let ignored = if walks_ignored || !is_dir {
            "--ignored=traditional"
        } else {
            "--ignored=no"
        };
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            // Explicit, so a `status.showUntrackedFiles = no` config cannot hide them.
            .args([
                "status",
                "--porcelain",
                "--untracked-files=normal",
                // A submodule's `ignore` setting must not hide its changes.
                "--ignore-submodules=none",
                ignored,
                "--",
            ])
            .arg(&absolute)
            // English messages, so "not a git repository" can be recognised.
            .env("LC_ALL", "C")
            .output();
        let output = match output {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => anyhow::bail!("cannot run git to check '{}': {}", path.display(), e),
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not a git repository") {
                continue;
            }
            anyhow::bail!(
                "cannot check '{}' for uncommitted changes: {}\n\
                 Fix the git error, or pass --allow-dirty.",
                path.display(),
                stderr.trim()
            );
        }
        dirty.extend(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(String::from),
        );
    }
    dirty.sort();
    dirty.dedup();
    Ok(dirty)
}

/// Fails if any of `paths` has uncommitted changes, unless `allow_dirty`.
///
/// `walks_ignored` is set when the run reaches gitignored files inside a
/// directory, as `--no-ignore` does. Git cannot restore those either.
pub fn ensure_clean(
    command: &str,
    paths: &[PathBuf],
    walks_ignored: bool,
    allow_dirty: bool,
) -> anyhow::Result<()> {
    if allow_dirty {
        return Ok(());
    }
    let dirty = uncommitted(paths, walks_ignored)?;
    if dirty.is_empty() {
        return Ok(());
    }

    const SHOWN: usize = 10;
    let mut listing: Vec<String> = dirty
        .iter()
        .take(SHOWN)
        .map(|l| format!("  {}", l))
        .collect();
    if dirty.len() > SHOWN {
        listing.push(format!("  ... and {} more", dirty.len() - SHOWN));
    }
    anyhow::bail!(
        "{} would modify files git cannot restore (uncommitted, untracked or ignored):\n{}\n\
         Commit or stash them first, preview with --diff or --dry-run, \
         or pass --allow-dirty.",
        command,
        listing.join("\n")
    )
}
