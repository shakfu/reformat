//! Refusing to rewrite files that git could not restore.
//!
//! `rename_files`, `group`, `convert` and `replace` change a tree in ways that
//! need review. Run over uncommitted work, their changes cannot be separated
//! from the user's or undone with git. `cargo fix` refuses for the same reason.

use std::path::{Path, PathBuf};
use std::process::Command;

/// `git status --porcelain` lines for changes under `paths`, including
/// untracked files. Paths outside a work tree, or with no `git` on PATH, are
/// not checked.
fn uncommitted(paths: &[PathBuf]) -> Vec<String> {
    let mut dirty = Vec::new();
    for path in paths {
        let dir = if path.is_dir() {
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
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            // Explicit, so a `status.showUntrackedFiles = no` config cannot hide them.
            .args(["status", "--porcelain", "--untracked-files=normal", "--"])
            .arg(&absolute)
            .output();
        if let Ok(output) = output {
            if output.status.success() {
                dirty.extend(
                    String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .map(String::from),
                );
            }
        }
    }
    dirty.sort();
    dirty.dedup();
    dirty
}

/// Fails if any of `paths` has uncommitted changes, unless `allow_dirty`.
pub fn ensure_clean(command: &str, paths: &[PathBuf], allow_dirty: bool) -> anyhow::Result<()> {
    if allow_dirty {
        return Ok(());
    }
    let dirty = uncommitted(paths);
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
        "{} would modify files with uncommitted changes:\n{}\n\
         Commit or stash them first, preview with --diff or --dry-run, \
         or pass --allow-dirty.",
        command,
        listing.join("\n")
    )
}
