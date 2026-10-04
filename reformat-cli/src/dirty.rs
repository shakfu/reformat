//! Refusing to rewrite files that git could not restore.
//!
//! `rename_files`, `group`, `convert` and `replace` change a tree in ways that
//! need review. Run over uncommitted work, their changes cannot be separated
//! from the user's or undone with git. `cargo fix` refuses for the same reason.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Upper bound on the pathspec bytes passed to one git call. Windows limits
/// a command line to 32767 characters.
const MAX_PATHSPEC_BYTES: usize = 24_000;

/// `git status --porcelain` lines for changes under `paths`, including
/// untracked files, and ignored files where the run can reach them: a file
/// named directly, or any file when `walks_ignored`. Paths outside a work
/// tree, or with no `git` on PATH, are not checked. Any other git failure is
/// an error, since the tree could not be checked.
///
/// Paths are batched per repository and ignore mode, so a long file list
/// costs a few git calls, not one full status scan per path.
fn uncommitted(paths: &[PathBuf], walks_ignored: bool) -> anyhow::Result<Vec<String>> {
    let mut batches: BTreeMap<(PathBuf, bool), Vec<PathBuf>> = BTreeMap::new();
    for path in paths {
        let Ok(absolute) = std::path::absolute(path) else {
            continue;
        };
        let is_dir = absolute.is_dir();
        let dir = if is_dir {
            absolute.as_path()
        } else {
            absolute.parent().unwrap_or(&absolute)
        };
        // Outside any `.git` ancestor, git may still find a repository
        // through GIT_DIR, so the directory itself is asked.
        let root = dir
            .ancestors()
            .find(|d| d.join(".git").symlink_metadata().is_ok())
            .unwrap_or(dir)
            .to_path_buf();
        let with_ignored = walks_ignored || !is_dir;
        batches
            .entry((root, with_ignored))
            .or_default()
            .push(absolute);
    }

    let mut dirty = Vec::new();
    for ((root, with_ignored), specs) in &batches {
        let mut start = 0;
        while start < specs.len() {
            let mut end = start + 1;
            let mut bytes = specs[start].as_os_str().len();
            while end < specs.len() && bytes + specs[end].as_os_str().len() < MAX_PATHSPEC_BYTES {
                bytes += specs[end].as_os_str().len();
                end += 1;
            }
            dirty.extend(status(root, *with_ignored, &specs[start..end])?);
            start = end;
        }
    }
    dirty.sort();
    dirty.dedup();
    Ok(dirty)
}

/// One `git status` call in `root` for `specs`.
fn status(root: &Path, with_ignored: bool, specs: &[PathBuf]) -> anyhow::Result<Vec<String>> {
    let ignored = if with_ignored {
        "--ignored=traditional"
    } else {
        "--ignored=no"
    };
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        // `a[1].txt` names one file; as a glob it also matches `a1.txt`.
        .arg("--literal-pathspecs")
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
        .args(specs)
        // English messages, so "not a git repository" can be recognised.
        .env("LC_ALL", "C")
        .output();
    let output = match output {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => anyhow::bail!("cannot run git to check '{}': {}", root.display(), e),
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not a git repository") {
            return Ok(Vec::new());
        }
        anyhow::bail!(
            "cannot check '{}' for uncommitted changes: {}\n\
             Fix the git error, or pass --allow-dirty.",
            root.display(),
            stderr.trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(String::from)
        .collect())
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
