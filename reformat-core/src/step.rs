//! Content transformations as composable steps, and the runner that applies
//! them to files.
//!
//! A [`ContentStep`] maps text to text and never touches the filesystem.
//! [`run_content_steps`] reads each file once, applies every step that accepts
//! it in order, and writes the result once. Callers choose which files to
//! offer, so file discovery stays out of the transformers.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A file offered to the runner.
#[derive(Debug, Clone)]
pub struct FileTarget {
    /// Path of the file.
    pub path: PathBuf,
    /// The path the user named, from which this file was reached.
    pub root: PathBuf,
    /// 0 for a file named directly, 1 for a direct child of `root`, and so on.
    pub depth: usize,
}

impl FileTarget {
    /// A target for a file named directly.
    pub fn file(path: &Path) -> Self {
        FileTarget {
            path: path.to_path_buf(),
            root: path.parent().unwrap_or(Path::new(".")).to_path_buf(),
            depth: 0,
        }
    }
}

/// A text transformation that can be composed with others.
pub trait ContentStep {
    /// Step name as used in presets, e.g. `"clean"`.
    fn name(&self) -> &'static str;

    /// Whether this step applies to `file`. Checked before the file is read.
    fn accepts(&self, file: &FileTarget) -> bool;

    /// Returns the new text and a step-specific change count, or `None` if
    /// the text is unchanged.
    fn transform(&self, text: &str, file: &FileTarget) -> Option<(String, usize)>;

    /// As [`ContentStep::transform`], for content that is not valid UTF-8.
    /// Steps that cannot work on raw bytes return `None`.
    fn transform_bytes(&self, _bytes: &[u8], _file: &FileTarget) -> Option<(Vec<u8>, usize)> {
        None
    }

    /// Whether [`ContentStep::transform_bytes`] is implemented.
    fn supports_bytes(&self) -> bool {
        false
    }

    /// One log line describing `units` changes to a file, e.g.
    /// `"Cleaned 2 lines in"`. The runner appends the path.
    fn describe(&self, units: usize, dry_run: bool) -> String;
}

/// Totals for one step across a run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepTotals {
    /// Step name.
    pub name: &'static str,
    /// Files this step changed.
    pub files: usize,
    /// Step-specific change count (lines, replacements, endings).
    pub units: usize,
}

/// Outcome of [`run_content_steps`].
#[derive(Debug, Clone, Default)]
pub struct RunReport {
    /// Per-step totals, in step order.
    pub steps: Vec<StepTotals>,
    /// Files changed by at least one step.
    pub files_changed: usize,
    /// One message per file that could not be read or written.
    pub errors: Vec<String>,
}

/// Returns true if `path` has one of `extensions`.
///
/// Matching ignores case and a leading dot, so `py`, `.py` and `.PY` are
/// equivalent. An empty list matches every file.
pub fn matches_extension<S: AsRef<str>>(path: &Path, extensions: &[S]) -> bool {
    if extensions.is_empty() {
        return true;
    }
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    extensions
        .iter()
        .any(|e| e.as_ref().trim_start_matches('.').eq_ignore_ascii_case(ext))
}

/// True if a non-recursive step may process `file`.
pub fn within_depth(file: &FileTarget, recursive: bool) -> bool {
    recursive || file.depth <= 1
}

/// Shared acceptance test for the extension-filtered transformers.
///
/// Hidden names are not checked here: which files to offer is the caller's
/// decision, so `reformat --hidden` can reach `.pre-commit-config.yaml`.
pub(crate) fn accepts_by_extension<S: AsRef<str>>(
    file: &FileTarget,
    extensions: &[S],
    recursive: bool,
) -> bool {
    within_depth(file, recursive) && matches_extension(&file.path, extensions)
}

/// True if the per-transformer file methods skip `path`: a hidden name or a
/// build directory name, as their directory walks do.
fn skipped_by_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_none_or(|n| crate::walk::is_excluded_component(n, crate::walk::DEFAULT_SKIP_DIRS))
}

/// Receives a changed file with its original and new contents.
pub type OnChange<'a> = dyn FnMut(&FileTarget, &[u8], &[u8]) + 'a;

/// Applies `steps` to each file in `files`.
///
/// Each file is read once and written once. When `dry_run` is set nothing is
/// written. `on_change` receives the original and new bytes of every changed
/// file. A file that cannot be read or written is recorded in
/// [`RunReport::errors`] and the run continues.
pub fn run_content_steps<I>(
    steps: &[&dyn ContentStep],
    files: I,
    dry_run: bool,
    on_change: &mut OnChange,
) -> RunReport
where
    I: IntoIterator<Item = FileTarget>,
{
    let mut report = RunReport {
        steps: steps
            .iter()
            .map(|s| StepTotals {
                name: s.name(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };

    for file in files {
        if crate::walk::in_git_dir(&file.path) {
            log::debug!("Refusing to modify {}", file.path.display());
            continue;
        }
        let active: Vec<usize> = (0..steps.len())
            .filter(|&i| steps[i].accepts(&file))
            .collect();
        if active.is_empty() {
            continue;
        }
        match apply_steps(steps, &active, &file, dry_run) {
            Ok(Some((original, updated, units))) => {
                report.files_changed += 1;
                for (i, n) in units.into_iter().enumerate() {
                    if n > 0 {
                        report.steps[i].files += 1;
                        report.steps[i].units += n;
                    }
                }
                on_change(&file, &original, &updated);
            }
            Ok(None) => {}
            Err(e) => {
                let message = format!("{}: {}", file.path.display(), e);
                log::error!("{}", message);
                report.errors.push(message);
            }
        }
    }

    report
}

type Applied = Option<(Vec<u8>, Vec<u8>, Vec<usize>)>;

/// Applies the steps in `steps` that accept `file` to `input`, in order.
///
/// Nothing is read or written: `file` only decides acceptance and names the
/// input in log messages, so this serves content that is not on disk, such as
/// stdin. Returns the result and each step's change count. Binary input (a
/// NUL byte) is returned unchanged. `dry_run` only sets the tense of the log
/// messages.
pub fn apply_to_bytes(
    steps: &[&dyn ContentStep],
    file: &FileTarget,
    input: &[u8],
    dry_run: bool,
) -> (Vec<u8>, Vec<usize>) {
    let active: Vec<usize> = (0..steps.len())
        .filter(|&i| steps[i].accepts(file))
        .collect();
    match transform_active(steps, &active, file, input, dry_run) {
        Some((output, units)) => (output, units),
        None => (input.to_vec(), vec![0; steps.len()]),
    }
}

/// Applies the `active` steps to `input`. `None` if binary or unchanged.
fn transform_active(
    steps: &[&dyn ContentStep],
    active: &[usize],
    file: &FileTarget,
    input: &[u8],
    dry_run: bool,
) -> Option<(Vec<u8>, Vec<usize>)> {
    if input.contains(&0) {
        log::debug!("Skipping binary file: {}", file.path.display());
        return None;
    }

    let mut current = input.to_vec();
    let mut units = vec![0; steps.len()];
    for &i in active {
        let step = steps[i];
        let result = match std::str::from_utf8(&current) {
            Ok(text) => step.transform(text, file).map(|(s, n)| (s.into_bytes(), n)),
            Err(_) if step.supports_bytes() => step.transform_bytes(&current, file),
            Err(_) => {
                log::warn!(
                    "Skipping file with non-UTF-8 contents for '{}': {}",
                    step.name(),
                    file.path.display()
                );
                None
            }
        };
        if let Some((next, n)) = result {
            if next != current {
                log::info!("{} '{}'", step.describe(n, dry_run), file.path.display());
                units[i] = n;
                current = next;
            }
        }
    }

    (current != input).then_some((current, units))
}

/// Reads `file`, applies the `active` steps, and writes the result.
fn apply_steps(
    steps: &[&dyn ContentStep],
    active: &[usize],
    file: &FileTarget,
    dry_run: bool,
) -> crate::Result<Applied> {
    let original = fs::read(&file.path)?;
    let Some((updated, units)) = transform_active(steps, active, file, &original, dry_run) else {
        return Ok(None);
    };
    if !dry_run {
        write_atomic(&file.path, &updated)?;
    }
    Ok(Some((original, updated, units)))
}

/// Applies one step to one file, returning its change count.
///
/// Used by the per-transformer file methods. I/O errors are returned.
pub(crate) fn apply_one(
    step: &dyn ContentStep,
    file: &FileTarget,
    dry_run: bool,
) -> crate::Result<usize> {
    if skipped_by_name(&file.path) || !step.accepts(file) {
        return Ok(0);
    }
    Ok(apply_steps(&[step], &[0], file, dry_run)?
        .map(|(_, _, units)| units[0])
        .unwrap_or(0))
}

/// Runs one step over `path`, a file or a directory walked with
/// [`crate::walk::walk_files`]. Returns `(files_changed, units)`, or the
/// first error after the walk completes.
pub(crate) fn process_path(
    step: &dyn ContentStep,
    path: &Path,
    recursive: bool,
    dry_run: bool,
) -> crate::Result<(usize, usize)> {
    let files: Vec<FileTarget> = if path.is_file() {
        if skipped_by_name(path) {
            Vec::new()
        } else {
            vec![FileTarget::file(path)]
        }
    } else if path.is_dir() {
        crate::walk::walk_files(path, recursive)
            .map(|e| FileTarget {
                path: e.path().to_path_buf(),
                root: path.to_path_buf(),
                depth: e.depth(),
            })
            .collect()
    } else {
        Vec::new()
    };

    let report = run_content_steps(&[step], files, dry_run, &mut |_, _, _| {});
    if let Some(first) = report.errors.first() {
        anyhow::bail!("{}", first);
    }
    Ok((report.steps[0].files, report.steps[0].units))
}

/// Replaces the contents of `path` without a window in which it is truncated.
///
/// Writes to a temporary file in the same directory, copies the original
/// permissions, then renames over the original. Symbolic links are resolved
/// first so the link itself survives. On Unix, a file with several hard
/// links is written in place, since a rename would detach it from the others.
pub fn write_atomic(path: &Path, contents: &[u8]) -> crate::Result<()> {
    let target = fs::canonicalize(path)?;
    let metadata = fs::metadata(&target)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() > 1 {
            fs::write(&target, contents)?;
            return Ok(());
        }
    }

    let dir = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent directory for '{}'", target.display()))?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{}.reformat-{}.tmp", name, std::process::id()));

    let result = (|| -> crate::Result<()> {
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        out.write_all(contents)?;
        out.sync_all()?;
        drop(out);
        fs::set_permissions(&tmp, metadata.permissions())?;
        fs::rename(&tmp, &target)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Upper;
    impl ContentStep for Upper {
        fn name(&self) -> &'static str {
            "upper"
        }
        fn accepts(&self, file: &FileTarget) -> bool {
            matches_extension(&file.path, &[".txt"])
        }
        fn transform(&self, text: &str, _: &FileTarget) -> Option<(String, usize)> {
            let up = text.to_uppercase();
            (up != text).then_some((up, 1))
        }
        fn describe(&self, _: usize, dry_run: bool) -> String {
            if dry_run { "Would upper" } else { "Uppered" }.to_string()
        }
    }

    struct Exclaim;
    impl ContentStep for Exclaim {
        fn name(&self) -> &'static str {
            "exclaim"
        }
        fn accepts(&self, _: &FileTarget) -> bool {
            true
        }
        fn transform(&self, text: &str, _: &FileTarget) -> Option<(String, usize)> {
            if text.ends_with('!') {
                None
            } else {
                Some((format!("{}!", text), 1))
            }
        }
        fn describe(&self, _: usize, _: bool) -> String {
            "Exclaimed".to_string()
        }
    }

    #[test]
    fn test_matches_extension_normalises() {
        let p = Path::new("a/b.PY");
        assert!(matches_extension(p, &["py"]));
        assert!(matches_extension(p, &[".py"]));
        assert!(matches_extension(p, &[".Py"]));
        assert!(!matches_extension(p, &[".rs"]));
        assert!(matches_extension(p, &[] as &[&str]));
        assert!(!matches_extension(Path::new("Makefile"), &[".py"]));
    }

    #[test]
    fn test_steps_compose_and_write_once() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.txt");
        fs::write(&path, "hi").unwrap();

        let mut seen = Vec::new();
        let report = run_content_steps(
            &[&Upper, &Exclaim],
            [FileTarget::file(&path)],
            false,
            &mut |_, before, after| seen.push((before.to_vec(), after.to_vec())),
        );

        assert_eq!(fs::read_to_string(&path).unwrap(), "HI!");
        assert_eq!(report.files_changed, 1);
        assert_eq!(report.steps[0].files, 1);
        assert_eq!(report.steps[1].files, 1);
        assert_eq!(seen, vec![(b"hi".to_vec(), b"HI!".to_vec())]);
    }

    #[test]
    fn test_dry_run_sees_earlier_steps_but_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.txt");
        fs::write(&path, "hi").unwrap();

        let mut after = Vec::new();
        run_content_steps(
            &[&Upper, &Exclaim],
            [FileTarget::file(&path)],
            true,
            &mut |_, _, a| after = a.to_vec(),
        );

        assert_eq!(fs::read_to_string(&path).unwrap(), "hi");
        assert_eq!(after, b"HI!", "the second step must see the first's output");
    }

    #[test]
    fn test_apply_to_bytes_touches_no_file() {
        let target = FileTarget::file(Path::new("does/not/exist.txt"));
        let (out, units) = apply_to_bytes(&[&Upper, &Exclaim], &target, b"hi", false);
        assert_eq!(out, b"HI!");
        assert_eq!(units, [1, 1]);

        let other = FileTarget::file(Path::new("x.md"));
        let (out, units) = apply_to_bytes(&[&Upper], &other, b"hi", false);
        assert_eq!((out, units), (b"hi".to_vec(), vec![0]), "not accepted");

        let (out, _) = apply_to_bytes(&[&Upper], &target, b"a\0b", false);
        assert_eq!(out, b"a\0b", "binary input passes through");
    }

    #[test]
    fn test_unaccepted_file_is_not_read() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing.md");
        let report = run_content_steps(
            &[&Upper],
            [FileTarget::file(&missing)],
            false,
            &mut |_, _, _| {},
        );
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_errors_are_collected_and_the_run_continues() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing.txt");
        let good = tmp.path().join("good.txt");
        fs::write(&good, "ok").unwrap();

        let report = run_content_steps(
            &[&Upper],
            [FileTarget::file(&missing), FileTarget::file(&good)],
            false,
            &mut |_, _, _| {},
        );

        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].contains("missing.txt"));
        assert_eq!(fs::read_to_string(&good).unwrap(), "OK");
    }

    #[test]
    fn test_binary_and_non_utf8_files_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin.txt");
        let latin1 = tmp.path().join("latin1.txt");
        fs::write(&bin, [b'a', 0, b'b']).unwrap();
        fs::write(&latin1, [b'c', 0xE9]).unwrap();

        let report = run_content_steps(
            &[&Upper],
            [FileTarget::file(&bin), FileTarget::file(&latin1)],
            false,
            &mut |_, _, _| {},
        );

        assert_eq!(report.files_changed, 0);
        assert!(report.errors.is_empty());
        assert_eq!(fs::read(&latin1).unwrap(), [b'c', 0xE9]);
    }

    #[test]
    fn test_runner_never_modifies_git_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let git = tmp.path().join(".git");
        fs::create_dir(&git).unwrap();
        let config = git.join("config.txt");
        fs::write(&config, "x").unwrap();

        run_content_steps(
            &[&Upper],
            [FileTarget::file(&config)],
            false,
            &mut |_, _, _| {},
        );

        assert_eq!(fs::read_to_string(&config).unwrap(), "x");
    }

    #[test]
    fn test_hidden_names_are_the_callers_choice() {
        let tmp = tempfile::tempdir().unwrap();
        let hidden = tmp.path().join(".notes.txt");
        fs::write(&hidden, "x").unwrap();

        // Offered explicitly to the runner, a hidden file is processed...
        run_content_steps(
            &[&Upper],
            [FileTarget::file(&hidden)],
            false,
            &mut |_, _, _| {},
        );
        assert_eq!(fs::read_to_string(&hidden).unwrap(), "X");

        // ...while the per-transformer path methods still skip it.
        fs::write(&hidden, "x").unwrap();
        assert_eq!(process_path(&Upper, &hidden, true, false).unwrap(), (0, 0));
        assert_eq!(
            apply_one(&Upper, &FileTarget::file(&hidden), false).unwrap(),
            0
        );
        assert_eq!(fs::read_to_string(&hidden).unwrap(), "x");
    }

    #[test]
    fn test_non_recursive_depth() {
        let t = |depth| FileTarget {
            path: PathBuf::from("x"),
            root: PathBuf::from("."),
            depth,
        };
        assert!(within_depth(&t(0), false));
        assert!(within_depth(&t(1), false));
        assert!(!within_depth(&t(2), false));
        assert!(within_depth(&t(2), true));
    }

    #[cfg(unix)]
    #[test]
    fn test_write_atomic_preserves_permissions_and_symlinks() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real.sh");
        let link = tmp.path().join("link.sh");
        fs::write(&real, "old").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o751)).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        write_atomic(&link, b"new").unwrap();

        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        let mode = fs::metadata(&real).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o751);
        let leftovers = fs::read_dir(tmp.path()).unwrap().count();
        assert_eq!(leftovers, 2, "temporary file left behind");
    }

    #[cfg(unix)]
    #[test]
    fn test_write_atomic_keeps_hard_links_together() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        fs::write(&a, "old").unwrap();
        fs::hard_link(&a, &b).unwrap();

        write_atomic(&a, b"new").unwrap();

        assert_eq!(fs::read_to_string(&b).unwrap(), "new");
    }
}
