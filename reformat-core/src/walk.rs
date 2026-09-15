//! Shared directory-traversal filtering.
//!
//! Every transformer needs the same two decisions: is this *file* one we are
//! allowed to touch, and is this *directory* one we should descend into. Both
//! are centralised here so that a new transformer cannot accidentally omit
//! them -- an omission that previously let the file renamer walk into `.git`
//! and destroy repositories.

use std::path::{Component, Path};
use walkdir::{DirEntry, WalkDir};

/// Directory names that are never traversed: version-control metadata,
/// dependency trees, and build output.
pub const DEFAULT_SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "build",
    "dist",
    "vendor",
    "__pycache__",
    "venv",
    ".venv",
];

/// Returns true if `name` -- a single path component, not a whole path -- is
/// a hidden entry or a skipped directory name.
pub fn is_excluded_component<S: AsRef<str>>(name: &str, skip_dirs: &[S]) -> bool {
    name.starts_with('.') || skip_dirs.iter().any(|d| d.as_ref() == name)
}

/// Returns true if any *named* component of `path` is hidden or a skipped
/// directory.
///
/// Use this for a path considered in isolation. For files reached by walking,
/// prefer pruning with [`include_entry`] and testing only the file's own name:
/// checking every component would also reject files whose *ancestors* happen
/// to be hidden, which silently skipped anything under a directory such as
/// `~/.config` or a `.tmp`-prefixed scratch directory.
///
/// Only [`Component::Normal`] components are considered. `.` and `..` are
/// navigation, not directory names, so `./src/main.rs` is not treated as
/// hidden -- without this, the natural `reformat clean .` was a silent no-op.
///
/// The check applies to the whole path as given, so an explicitly named hidden
/// file (`reformat clean .hidden.txt`) is still skipped.
pub fn is_excluded<S: AsRef<str>>(path: &Path, skip_dirs: &[S]) -> bool {
    path.components().any(|c| match c {
        Component::Normal(os) => os
            .to_str()
            .is_some_and(|s| is_excluded_component(s, skip_dirs)),
        _ => false,
    })
}

/// Returns true if any component of `path` is `.git`. Such paths are never
/// modified, whatever the caller selected.
pub fn in_git_dir(path: &Path) -> bool {
    path.components()
        .any(|c| matches!(c, Component::Normal(name) if name == ".git"))
}

/// Renders `path` with `/` separators on every platform. Recorded paths are
/// written into source files as references, where `\` is wrong.
pub fn to_slash(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Predicate for [`walkdir::IntoIter::filter_entry`], pruning excluded
/// subtrees *before* descending into them rather than filtering their files
/// out one by one afterwards.
///
/// The walk root (depth 0) is always accepted: the user named it explicitly,
/// and pruning it would make every walk empty. Per-file exclusion is still
/// enforced by [`is_excluded`].
pub fn include_entry<S: AsRef<str>>(entry: &DirEntry, skip_dirs: &[S]) -> bool {
    if entry.depth() == 0 {
        // The user named this directory explicitly, so being hidden is not
        // disqualifying -- a project may well live under `~/.config`, and
        // pruning the root would make every such walk silently empty. A root
        // that *is* a known metadata or build directory is still refused:
        // pointing the renamer at `.git` should not destroy the repository.
        return match entry.file_name().to_str() {
            Some(name) => !skip_dirs.iter().any(|d| d.as_ref() == name),
            None => false,
        };
    }
    match entry.file_name().to_str() {
        Some(name) => !is_excluded_component(name, skip_dirs),
        None => false, // non-UTF-8 names cannot be matched against patterns
    }
}

/// Iterator over the files under `root`, pruning hidden and build directories.
///
/// When `recursive` is false only the immediate children of `root` are
/// yielded. Unreadable entries are skipped rather than aborting the walk.
pub fn walk_files(root: &Path, recursive: bool) -> impl Iterator<Item = DirEntry> {
    let walker = if recursive {
        WalkDir::new(root)
    } else {
        WalkDir::new(root).max_depth(1)
    };

    walker
        .into_iter()
        .filter_entry(|e| include_entry(e, DEFAULT_SKIP_DIRS))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
}

/// As [`walk_files`], but also yields symbolic links when `include_symlinks`
/// is set. Returns `(path, is_symlink)` pairs.
pub fn walk_files_and_symlinks(
    root: &Path,
    recursive: bool,
    include_symlinks: bool,
) -> impl Iterator<Item = (std::path::PathBuf, bool)> {
    let walker = if recursive {
        WalkDir::new(root)
    } else {
        WalkDir::new(root).max_depth(1)
    };

    walker
        .into_iter()
        .filter_entry(|e| include_entry(e, DEFAULT_SKIP_DIRS))
        .filter_map(|e| e.ok())
        .filter_map(move |e| {
            let ft = e.file_type();
            let is_symlink = ft.is_symlink();
            if ft.is_file() || (include_symlinks && is_symlink) {
                Some((e.path().to_path_buf(), is_symlink))
            } else {
                None
            }
        })
}

/// Iterator over the directories under `root` (excluding `root` itself),
/// pruning hidden and build directories.
///
/// The full list is materialised by the caller before any work begins, so
/// directories created during processing are not themselves processed.
pub fn walk_dirs(root: &Path, recursive: bool) -> impl Iterator<Item = std::path::PathBuf> {
    let walker = if recursive {
        WalkDir::new(root)
    } else {
        WalkDir::new(root).max_depth(1)
    };

    walker
        .into_iter()
        .filter_entry(|e| include_entry(e, DEFAULT_SKIP_DIRS))
        .filter_map(|e| e.ok())
        .filter(|e| e.depth() > 0 && e.file_type().is_dir())
        .map(|e| e.path().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_slash_uses_forward_slashes() {
        assert_eq!(to_slash(&Path::new("wbs").join("a.tmpl")), "wbs/a.tmpl");
        assert_eq!(to_slash(Path::new("a.tmpl")), "a.tmpl");
    }

    #[test]
    fn test_hidden_component_is_excluded() {
        assert!(is_excluded(Path::new("/a/.git/config"), DEFAULT_SKIP_DIRS));
        assert!(is_excluded(Path::new(".hidden.txt"), DEFAULT_SKIP_DIRS));
        assert!(is_excluded(Path::new("a/.cache/b.txt"), DEFAULT_SKIP_DIRS));
    }

    #[test]
    fn test_build_directories_are_excluded() {
        for dir in DEFAULT_SKIP_DIRS {
            let p = format!("project/{}/file.txt", dir);
            assert!(
                is_excluded(Path::new(&p), DEFAULT_SKIP_DIRS),
                "{} should be excluded",
                dir
            );
        }
    }

    #[test]
    fn test_dot_components_are_not_hidden() {
        // Regression: `reformat clean .` used to be a silent no-op because the
        // `.` component was read as a hidden directory name.
        assert!(!is_excluded(Path::new("./src/main.rs"), DEFAULT_SKIP_DIRS));
        assert!(!is_excluded(Path::new("."), DEFAULT_SKIP_DIRS));
        assert!(!is_excluded(
            Path::new("../sibling/a.txt"),
            DEFAULT_SKIP_DIRS
        ));
        assert!(!is_excluded(Path::new("./a.txt"), DEFAULT_SKIP_DIRS));
    }

    #[test]
    fn test_ordinary_paths_are_included() {
        assert!(!is_excluded(
            Path::new("/home/u/proj/src/a.rs"),
            DEFAULT_SKIP_DIRS
        ));
        assert!(!is_excluded(Path::new("a.txt"), DEFAULT_SKIP_DIRS));
        // A file merely *named* like a skip dir is fine; only components match.
        assert!(!is_excluded(Path::new("src/target.rs"), DEFAULT_SKIP_DIRS));
    }

    #[test]
    fn test_custom_skip_list() {
        let skip = ["fixtures".to_string()];
        assert!(is_excluded(Path::new("t/fixtures/a.txt"), &skip));
        assert!(!is_excluded(Path::new("t/cases/a.txt"), &skip));
    }

    #[test]
    fn test_hidden_root_is_walked_but_metadata_root_is_not() {
        let tmp = tempfile::Builder::new()
            .prefix(".hidden-root")
            .tempdir()
            .unwrap();
        std::fs::write(tmp.path().join("a.rs"), "x").unwrap();
        let found: Vec<_> = walk_files(tmp.path(), true).collect();
        assert_eq!(
            found.len(),
            1,
            "a hidden root the user named explicitly should still be walked"
        );

        let repo = tempfile::Builder::new().prefix("repo").tempdir().unwrap();
        let git = repo.path().join(".git");
        std::fs::create_dir_all(&git).unwrap();
        std::fs::write(git.join("HEAD"), "ref").unwrap();
        assert_eq!(
            walk_files(&git, true).count(),
            0,
            "pointing directly at .git must still be refused"
        );
    }

    #[test]
    fn test_walk_prunes_excluded_subtrees() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let tmp = _tmp.path().to_path_buf();
        std::fs::create_dir_all(tmp.join(".git")).unwrap();
        std::fs::create_dir_all(tmp.join("node_modules")).unwrap();
        std::fs::create_dir_all(tmp.join("src")).unwrap();
        std::fs::write(tmp.join(".git/HEAD"), "ref").unwrap();
        std::fs::write(tmp.join("node_modules/x.js"), "x").unwrap();
        std::fs::write(tmp.join("src/a.rs"), "a").unwrap();
        std::fs::write(tmp.join("top.rs"), "t").unwrap();

        let found: Vec<String> = walk_files(&tmp, true)
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();

        assert!(found.contains(&"a.rs".to_string()));
        assert!(found.contains(&"top.rs".to_string()));
        assert!(!found.contains(&"HEAD".to_string()), "descended into .git");
        assert!(
            !found.contains(&"x.js".to_string()),
            "descended into node_modules"
        );

        let shallow: Vec<String> = walk_files(&tmp, false)
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(shallow.contains(&"top.rs".to_string()));
        assert!(
            !shallow.contains(&"a.rs".to_string()),
            "non-recursive walk descended"
        );
    }
}
