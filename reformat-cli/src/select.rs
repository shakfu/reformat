//! File discovery: which files a command is offered.
//!
//! Directories are walked with the `ignore` crate, so `.gitignore`, `.ignore`
//! and git exclude files are honoured. `.git` is never entered, whatever the
//! flags say.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use ignore::overrides::{Override, OverrideBuilder};
use ignore::WalkBuilder;
use reformat_core::walk::DEFAULT_SKIP_DIRS;
use reformat_core::FileTarget;

/// How files are selected. Mirrors the `--include`, `--exclude`, `--hidden`
/// and `--no-ignore` flags.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub hidden: bool,
    pub no_ignore: bool,
}

/// A selected file.
#[derive(Debug, Clone)]
pub struct Found {
    pub target: FileTarget,
    pub is_symlink: bool,
}

impl Selection {
    fn overrides(&self, root: &Path) -> anyhow::Result<Override> {
        let mut builder = OverrideBuilder::new(root);
        for glob in &self.include {
            builder.add(glob)?;
        }
        for glob in &self.exclude {
            builder.add(&format!("!{}", glob))?;
        }
        Ok(builder.build()?)
    }
}

fn is_hidden_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

/// Returns the files under `paths`, each at most once, in a stable order.
///
/// Every path must exist; this is checked before anything is returned, so a
/// typo in the last argument cannot leave the earlier ones half-processed.
pub fn discover(
    paths: &[PathBuf],
    selection: &Selection,
    recursive: bool,
    include_symlinks: bool,
) -> anyhow::Result<Vec<Found>> {
    for path in paths {
        if fs::symlink_metadata(path).is_err() {
            anyhow::bail!("path '{}' does not exist", path.display());
        }
    }

    let mut seen = HashSet::new();
    let mut found = Vec::new();

    for root in paths {
        if reformat_core::walk::in_git_dir(root) {
            log::warn!("Skipping '{}': .git is never processed", root.display());
            continue;
        }

        if root.is_dir() {
            walk_dir(root, selection, recursive, include_symlinks, &mut |f| {
                if seen.insert(std::path::absolute(&f.target.path).unwrap_or_default()) {
                    found.push(f);
                }
            })?;
            continue;
        }

        let is_symlink = fs::symlink_metadata(root)?.file_type().is_symlink();
        if !(root.is_file() || include_symlinks && is_symlink) {
            continue;
        }
        if !named_file_selected(root, selection)? {
            continue;
        }
        let absolute = std::path::absolute(root)?;
        if seen.insert(absolute) {
            found.push(Found {
                target: FileTarget::file(root),
                is_symlink,
            });
        }
    }

    Ok(found)
}

/// Whether a file named directly passes `--hidden`, `--include` and
/// `--exclude`. The file need not exist, so this also serves a stdin filename.
/// `.gitignore` does not apply to a file named directly.
pub fn named_file_selected(path: &Path, selection: &Selection) -> anyhow::Result<bool> {
    if reformat_core::walk::in_git_dir(path) {
        return Ok(false);
    }
    if is_hidden_name(path) && !selection.hidden {
        log::debug!("Skipping hidden file: {}", path.display());
        return Ok(false);
    }
    let cwd = std::env::current_dir()?;
    let absolute = std::path::absolute(path)?;
    let overrides = selection.overrides(&cwd)?;
    // `matched` tests only the path itself. A walk prunes excluded
    // directories; a named file must check its parents instead.
    let excluded_parent = absolute
        .ancestors()
        .skip(1)
        .take_while(|dir| dir.starts_with(&cwd) && *dir != cwd)
        .any(|dir| overrides.matched(dir, true).is_ignore());
    if excluded_parent || overrides.matched(&absolute, false).is_ignore() {
        log::debug!("Skipping excluded file: {}", path.display());
        return Ok(false);
    }
    Ok(true)
}

fn walk_dir(
    root: &Path,
    selection: &Selection,
    recursive: bool,
    include_symlinks: bool,
    emit: &mut dyn FnMut(Found),
) -> anyhow::Result<()> {
    // The walker never filters its root, so refuse skipped roots here.
    let root_name = root.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !selection.no_ignore && DEFAULT_SKIP_DIRS.contains(&root_name) {
        log::warn!(
            "Skipping '{}': build and vendor directories need --no-ignore",
            root.display()
        );
        return Ok(());
    }

    let honour_ignores = !selection.no_ignore;
    let walker = WalkBuilder::new(root)
        .hidden(!selection.hidden)
        .parents(honour_ignores)
        .ignore(honour_ignores)
        .git_ignore(honour_ignores)
        .git_global(honour_ignores)
        .git_exclude(honour_ignores)
        .overrides(selection.overrides(root)?)
        .max_depth(if recursive { None } else { Some(1) })
        .sort_by_file_name(|a, b| a.cmp(b))
        .filter_entry(move |entry| match entry.file_name().to_str() {
            Some(".git") => false,
            Some(name) => !honour_ignores || !DEFAULT_SKIP_DIRS.contains(&name),
            None => false,
        })
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                log::warn!("{}", e);
                continue;
            }
        };
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        let is_symlink = file_type.is_symlink();
        if !(file_type.is_file() || include_symlinks && is_symlink) {
            continue;
        }
        emit(Found {
            target: FileTarget {
                path: entry.path().to_path_buf(),
                root: root.to_path_buf(),
                depth: entry.depth(),
            },
            is_symlink,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(found: &[Found], root: &Path) -> Vec<String> {
        let mut v: Vec<String> = found
            .iter()
            .map(|f| {
                f.target
                    .path
                    .strip_prefix(root)
                    .unwrap_or(&f.target.path)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        v.sort();
        v
    }

    fn tree(files: &[&str]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        for f in files {
            let p = tmp.path().join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, "x").unwrap();
        }
        tmp
    }

    #[test]
    fn test_default_selection_skips_hidden_git_and_build_dirs() {
        let tmp = tree(&[
            "a.rs",
            "sub/b.rs",
            ".github/ci.yml",
            ".git/HEAD",
            "node_modules/x.js",
        ]);
        let found = discover(
            &[tmp.path().to_path_buf()],
            &Selection::default(),
            true,
            false,
        )
        .unwrap();
        assert_eq!(names(&found, tmp.path()), ["a.rs", "sub/b.rs"]);
    }

    #[test]
    fn test_hidden_and_no_ignore_never_reach_git() {
        let tmp = tree(&["a.rs", ".github/ci.yml", ".git/HEAD", "node_modules/x.js"]);
        let selection = Selection {
            hidden: true,
            no_ignore: true,
            ..Default::default()
        };
        let found = discover(&[tmp.path().to_path_buf()], &selection, true, false).unwrap();
        assert_eq!(
            names(&found, tmp.path()),
            [".github/ci.yml", "a.rs", "node_modules/x.js"]
        );
    }

    #[test]
    fn test_include_and_exclude_globs() {
        let tmp = tree(&["Makefile", "a.rs", "vendor/c.rs", "docs/d.md"]);
        let selection = Selection {
            include: vec!["Makefile".into(), "*.rs".into()],
            exclude: vec!["vendor/".into()],
            ..Default::default()
        };
        let found = discover(&[tmp.path().to_path_buf()], &selection, true, false).unwrap();
        assert_eq!(names(&found, tmp.path()), ["Makefile", "a.rs"]);
    }

    #[test]
    fn test_gitignore_is_honoured_inside_a_repository() {
        let tmp = tree(&["keep.rs", "generated.rs", ".gitignore"]);
        fs::create_dir(tmp.path().join(".git")).unwrap();
        fs::write(tmp.path().join(".gitignore"), "generated.rs\n").unwrap();

        let root = [tmp.path().to_path_buf()];
        let found = discover(&root, &Selection::default(), true, false).unwrap();
        assert_eq!(names(&found, tmp.path()), ["keep.rs"]);

        let no_ignore = Selection {
            no_ignore: true,
            ..Default::default()
        };
        let found = discover(&root, &no_ignore, true, false).unwrap();
        assert_eq!(names(&found, tmp.path()), ["generated.rs", "keep.rs"]);
    }

    #[test]
    fn test_non_recursive_and_duplicates() {
        let tmp = tree(&["a.rs", "sub/b.rs"]);
        let paths = [tmp.path().to_path_buf(), tmp.path().join("a.rs")];
        let found = discover(&paths, &Selection::default(), false, false).unwrap();
        assert_eq!(names(&found, tmp.path()), ["a.rs"]);
    }

    #[test]
    fn test_missing_path_fails_before_anything_is_returned() {
        let tmp = tree(&["a.rs"]);
        let paths = [tmp.path().join("a.rs"), tmp.path().join("nope.rs")];
        let err = discover(&paths, &Selection::default(), true, false).unwrap_err();
        assert!(err.to_string().contains("nope.rs"));
    }

    #[test]
    fn test_named_file_inside_excluded_directory() {
        let selection = Selection {
            exclude: vec!["vendor".into()],
            ..Default::default()
        };
        assert!(!named_file_selected(Path::new("vendor/lib/a.py"), &selection).unwrap());
        assert!(named_file_selected(Path::new("src/a.py"), &selection).unwrap());
    }

    #[test]
    fn test_explicit_files_obey_hidden_git_and_excludes() {
        let tmp = tree(&[".env", ".git/config", "a.rs"]);
        let paths = [
            tmp.path().join(".env"),
            tmp.path().join(".git/config"),
            tmp.path().join("a.rs"),
        ];
        let selection = Selection {
            exclude: vec!["a.rs".into()],
            ..Default::default()
        };
        let found = discover(&paths, &selection, true, false).unwrap();
        assert!(found.is_empty(), "got {:?}", names(&found, tmp.path()));
    }
}
