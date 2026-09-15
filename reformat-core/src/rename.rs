//! File renaming transformer

use std::fs;
use std::path::{Path, PathBuf};

/// Case transformation options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CaseTransform {
    /// Convert to lowercase
    Lowercase,
    /// Convert to UPPERCASE
    Uppercase,
    /// Capitalize first letter only
    Capitalize,
    /// No case transformation
    None,
}

/// Space replacement options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpaceReplace {
    /// Replace spaces with underscores
    Underscore,
    /// Replace spaces with hyphens
    Hyphen,
    /// No space replacement
    None,
}

/// Timestamp format options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimestampFormat {
    /// YYYYMMDD format (e.g., 20250915)
    Long,
    /// YYMMDD format (e.g., 250915)
    Short,
    /// No timestamp
    None,
}

/// Options for file renaming
#[derive(Debug, Clone)]
pub struct RenameOptions {
    /// Case transformation to apply
    pub case_transform: CaseTransform,
    /// Space replacement to apply
    pub space_replace: SpaceReplace,
    /// Prefix to add
    pub add_prefix: Option<String>,
    /// Prefix to remove
    pub remove_prefix: Option<String>,
    /// Suffix to add (before extension)
    pub add_suffix: Option<String>,
    /// Suffix to remove (before extension)
    pub remove_suffix: Option<String>,
    /// Replace prefix (old, new)
    pub replace_prefix: Option<(String, String)>,
    /// Replace suffix (old, new)
    pub replace_suffix: Option<(String, String)>,
    /// Timestamp format for prefix (based on file creation time)
    pub timestamp_format: TimestampFormat,
    /// Process directories recursively
    pub recursive: bool,
    /// Dry run mode (don't rename files)
    pub dry_run: bool,
    /// Include symbolic links in processing
    pub include_symlinks: bool,
}

impl Default for RenameOptions {
    fn default() -> Self {
        RenameOptions {
            case_transform: CaseTransform::None,
            space_replace: SpaceReplace::None,
            add_prefix: None,
            remove_prefix: None,
            add_suffix: None,
            remove_suffix: None,
            replace_prefix: None,
            replace_suffix: None,
            timestamp_format: TimestampFormat::None,
            recursive: true,
            dry_run: false,
            include_symlinks: false,
        }
    }
}

/// Outcome of a rename run.
///
/// A collision used to abort the entire run through `?`, leaving a large tree
/// half-renamed with no record of what had moved. Failures are now collected
/// so the run completes and reports what it could not do.
#[derive(Debug, Clone, Default)]
pub struct RenameStats {
    /// Files successfully renamed
    pub renamed: usize,
    /// Files left alone because renaming them would have failed
    pub skipped: usize,
    /// One message per skipped file
    pub errors: Vec<String>,
}

/// File renamer for transforming file names
pub struct FileRenamer {
    options: RenameOptions,
}

impl FileRenamer {
    /// Creates a new file renamer with the given options
    pub fn new(options: RenameOptions) -> Self {
        FileRenamer { options }
    }

    /// Creates a renamer with default options
    pub fn with_defaults() -> Self {
        FileRenamer {
            options: RenameOptions::default(),
        }
    }

    /// Checks if a path should be processed
    fn should_process(&self, path: &Path, is_symlink: bool, skip_hidden: bool) -> bool {
        // Skip symlinks unless include_symlinks is enabled
        if is_symlink && !self.options.include_symlinks {
            return false;
        }

        // For symlinks, check if it's a symlink (not a directory symlink)
        // For regular files, check is_file()
        if !is_symlink && !path.is_file() {
            return false;
        }

        // Renaming `HEAD` or `config` inside `.git` destroys the repository,
        // and renames are not journalled, so this is refused unconditionally.
        if crate::walk::in_git_dir(path) {
            return false;
        }

        // Hidden and build-directory names, unless the caller selected them.
        if skip_hidden
            && path.file_name().and_then(|n| n.to_str()).is_none_or(|n| {
                crate::walk::is_excluded_component(n, crate::walk::DEFAULT_SKIP_DIRS)
            })
        {
            return false;
        }

        true
    }

    /// Detects the separator style used in a filename
    /// Returns '-' for hyphenated or space-separated, '_' for underscored, or '-' as default
    fn detect_separator(name: &str) -> char {
        let hyphen_count = name.chars().filter(|&c| c == '-').count();
        let underscore_count = name.chars().filter(|&c| c == '_').count();
        let space_count = name.chars().filter(|&c| c == ' ').count();

        // If spaces are present, default to hyphen (unless overridden by user transformations)
        if space_count > 0 {
            return '-';
        }

        // If hyphens are more common, use hyphen
        if hyphen_count > underscore_count {
            '-'
        } else if underscore_count > hyphen_count {
            '_'
        } else {
            // When equal or no separators, default to hyphen
            '-'
        }
    }

    /// Formats a timestamp based on file creation time.
    ///
    /// Falls back to the modification time when the filesystem does not record
    /// a creation time, which is the common case on Linux. The date is the
    /// local one, matching what the user sees in their file manager.
    fn format_timestamp(&self, path: &Path, separator: char) -> Option<String> {
        use chrono::{DateTime, Local};

        if self.options.timestamp_format == TimestampFormat::None {
            return None;
        }

        let metadata = fs::metadata(path).ok()?;
        let created = metadata.created().or_else(|_| metadata.modified()).ok()?;
        let datetime: DateTime<Local> = created.into();

        match self.options.timestamp_format {
            TimestampFormat::Long => Some(format!("{}{}", datetime.format("%Y%m%d"), separator)),
            TimestampFormat::Short => Some(format!("{}{}", datetime.format("%y%m%d"), separator)),
            TimestampFormat::None => None,
        }
    }

    /// Applies all transformations to a filename
    fn transform_name(
        &self,
        name: &str,
        extension: Option<&str>,
        timestamp: Option<String>,
    ) -> String {
        let mut result = name.to_string();

        // 1. Remove prefix
        if let Some(prefix) = &self.options.remove_prefix {
            if result.starts_with(prefix) {
                result = result[prefix.len()..].to_string();
            }
        }

        // 2. Remove suffix (before extension)
        if let Some(suffix) = &self.options.remove_suffix {
            if result.ends_with(suffix) {
                result = result[..result.len() - suffix.len()].to_string();
            }
        }

        // 3. Replace prefix
        if let Some((old_prefix, new_prefix)) = &self.options.replace_prefix {
            if result.starts_with(old_prefix) {
                result = format!("{}{}", new_prefix, &result[old_prefix.len()..]);
            }
        }

        // 4. Replace suffix
        if let Some((old_suffix, new_suffix)) = &self.options.replace_suffix {
            if result.ends_with(old_suffix) {
                result = format!(
                    "{}{}",
                    &result[..result.len() - old_suffix.len()],
                    new_suffix
                );
            }
        }

        // 5. Separator replacement (replace spaces, hyphens, underscores with desired separator)
        match self.options.space_replace {
            SpaceReplace::Underscore => {
                // Replace all separators (spaces, hyphens) with underscores
                result = result.replace([' ', '-'], "_");
            }
            SpaceReplace::Hyphen => {
                // Replace all separators (spaces, underscores) with hyphens
                result = result.replace([' ', '_'], "-");
            }
            SpaceReplace::None => {}
        }

        // 6. Case transformation
        match self.options.case_transform {
            CaseTransform::Lowercase => {
                result = result.to_lowercase();
            }
            CaseTransform::Uppercase => {
                result = result.to_uppercase();
            }
            CaseTransform::Capitalize => {
                if !result.is_empty() {
                    let mut chars = result.chars();
                    if let Some(first) = chars.next() {
                        result = first.to_uppercase().collect::<String>()
                            + &chars.as_str().to_lowercase();
                    }
                }
            }
            CaseTransform::None => {}
        }

        // 7. Add timestamp prefix (if specified)
        if let Some(ts) = timestamp {
            result = format!("{}{}", ts, result);
        }

        // 8. Add prefix
        if let Some(prefix) = &self.options.add_prefix {
            result = format!("{}{}", prefix, result);
        }

        // 9. Add suffix (before extension)
        if let Some(suffix) = &self.options.add_suffix {
            result = format!("{}{}", result, suffix);
        }

        // 10. Add extension back
        if let Some(ext) = extension {
            result = format!("{}.{}", result, ext);
        }

        result
    }

    /// Renames a single file or symlink
    ///
    /// Hidden names are skipped. Paths inside `.git` are always refused.
    pub fn rename_file(&self, path: &Path, is_symlink: bool) -> crate::Result<bool> {
        self.rename_one(path, is_symlink, true)
    }

    fn rename_one(&self, path: &Path, is_symlink: bool, skip_hidden: bool) -> crate::Result<bool> {
        if !self.should_process(path, is_symlink, skip_hidden) {
            return Ok(false);
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

        // Split filename and extension
        let (name, extension) = if let Some(pos) = file_name.rfind('.') {
            let name = &file_name[..pos];
            let ext = &file_name[pos + 1..];
            (name, Some(ext))
        } else {
            (file_name, None)
        };

        // Detect separator style from the filename
        let separator = Self::detect_separator(name);

        // Get timestamp if needed (with detected separator)
        let timestamp = self.format_timestamp(path, separator);

        let new_name = self.transform_name(name, extension, timestamp);

        // If name didn't change, nothing to do
        if new_name == file_name {
            return Ok(false);
        }

        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("No parent directory"))?;
        let new_path = parent.join(&new_name);

        // Check if target already exists (but allow case-only renames on case-insensitive filesystems)
        if new_path.exists() {
            // Check if this is the same file (case-insensitive filesystems)
            // Use canonicalize to resolve to the actual path
            let same_file = match (path.canonicalize(), new_path.canonicalize()) {
                (Ok(p1), Ok(p2)) => p1 == p2,
                _ => false,
            };

            if !same_file {
                return Err(anyhow::anyhow!(
                    "Target file already exists: '{}'",
                    new_path.display()
                ));
            }
        }

        if self.options.dry_run {
            log::info!(
                "Would rename '{}' -> '{}'",
                path.display(),
                new_path.display()
            );
        } else {
            fs::rename(path, &new_path)?;
            log::info!("Renamed '{}' -> '{}'", path.display(), new_path.display());
        }

        Ok(true)
    }

    /// Processes a directory or file, returning the number renamed.
    ///
    /// Prefer [`FileRenamer::process_with_stats`] when you need to know
    /// whether anything was skipped.
    pub fn process(&self, path: &Path) -> crate::Result<usize> {
        Ok(self.process_with_stats(path)?.renamed)
    }

    /// Processes a directory or file, collecting per-file failures instead of
    /// aborting on the first one.
    pub fn process_with_stats(&self, path: &Path) -> crate::Result<RenameStats> {
        let mut stats = RenameStats::default();

        // Check if path itself is a symlink
        let path_is_symlink = path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);

        if path.is_file() || path_is_symlink {
            self.record(self.rename_file(path, path_is_symlink), path, &mut stats);
        } else if path.is_dir() {
            let files = crate::walk::walk_files_and_symlinks(
                path,
                self.options.recursive,
                self.options.include_symlinks,
            )
            .collect();
            return Ok(self.rename_paths(files));
        }

        Ok(stats)
    }

    /// Renames each `(path, is_symlink)` entry, collecting per-file failures.
    ///
    /// The caller selects the files, so hidden names are renamed if offered.
    /// Paths inside `.git` are still refused.
    ///
    /// Entries are processed deepest first, then alphabetically, so the order
    /// is reproducible.
    pub fn rename_paths(&self, mut files: Vec<(PathBuf, bool)>) -> RenameStats {
        let mut stats = RenameStats::default();
        files.sort_by(|a, b| {
            b.0.components()
                .count()
                .cmp(&a.0.components().count())
                .then_with(|| a.0.cmp(&b.0))
        });
        for (file_path, is_symlink) in files {
            self.record(
                self.rename_one(&file_path, is_symlink, false),
                &file_path,
                &mut stats,
            );
        }
        stats
    }

    /// Folds one file's result into the run statistics, logging any failure.
    fn record(&self, result: crate::Result<bool>, path: &Path, stats: &mut RenameStats) {
        match result {
            Ok(true) => stats.renamed += 1,
            Ok(false) => {}
            Err(e) => {
                let message = format!("{}: {}", path.display(), e);
                log::warn!("Skipping {}", message);
                stats.errors.push(message);
                stats.skipped += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// One collision must not abort the run: the other files still get
    /// renamed, and the failure is reported rather than swallowed.
    #[test]
    fn test_collision_does_not_abort_the_run() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        // The collision comes from space replacement, not from case: a
        // case-only clash ("b.txt" vs "B.txt") cannot even be set up on the
        // case-insensitive filesystems of macOS and Windows.
        //
        // "a b.txt" cannot become "a_b.txt" because that name is taken.
        fs::write(dir.join("a_b.txt"), "x").unwrap();
        fs::write(dir.join("a b.txt"), "y").unwrap();
        fs::write(dir.join("c d.txt"), "z").unwrap();
        fs::write(dir.join("e f.txt"), "w").unwrap();

        let renamer = FileRenamer::new(RenameOptions {
            space_replace: SpaceReplace::Underscore,
            ..Default::default()
        });
        let stats = renamer.process_with_stats(&dir).unwrap();

        assert_eq!(
            stats.renamed, 2,
            "'c d.txt' and 'e f.txt' should still be renamed"
        );
        assert_eq!(
            stats.skipped, 1,
            "the collision should be counted, not fatal"
        );
        assert_eq!(stats.errors.len(), 1);
        assert!(dir.join("c_d.txt").exists());
        assert!(dir.join("e_f.txt").exists());
        // The blocked file is left where it was, with its content intact.
        assert_eq!(fs::read_to_string(dir.join("a b.txt")).unwrap(), "y");
        assert_eq!(fs::read_to_string(dir.join("a_b.txt")).unwrap(), "x");
    }

    #[test]
    fn test_lowercase_transform() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("TestFile.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            case_transform: CaseTransform::Lowercase,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        let new_file = test_dir.join("testfile.txt");
        assert!(new_file.exists());
        assert_eq!(fs::read_to_string(&new_file).unwrap(), "content");
    }

    #[test]
    fn test_uppercase_transform() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("testfile.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            case_transform: CaseTransform::Uppercase,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        let new_file = test_dir.join("TESTFILE.txt");
        assert!(new_file.exists());
        assert_eq!(fs::read_to_string(&new_file).unwrap(), "content");
    }

    #[test]
    fn test_capitalize_transform() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("testFile.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            case_transform: CaseTransform::Capitalize,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        let new_file = test_dir.join("Testfile.txt");
        assert!(new_file.exists());
        assert_eq!(fs::read_to_string(&new_file).unwrap(), "content");
    }

    #[test]
    fn test_separators_to_underscore() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        // Test space to underscore
        let test_file1 = test_dir.join("test file.txt");
        fs::write(&test_file1, "content").unwrap();

        // Test hyphen to underscore
        let test_file2 = test_dir.join("test-file2.txt");
        fs::write(&test_file2, "content").unwrap();

        // Test mixed separators to underscore
        let test_file3 = test_dir.join("test-file 3.txt");
        fs::write(&test_file3, "content").unwrap();

        let opts = RenameOptions {
            space_replace: SpaceReplace::Underscore,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_dir).unwrap();

        assert_eq!(count, 3);
        assert!(test_dir.join("test_file.txt").exists());
        assert!(test_dir.join("test_file2.txt").exists());
        assert!(test_dir.join("test_file_3.txt").exists());
    }

    #[test]
    fn test_separators_to_hyphen() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        // Test space to hyphen
        let test_file1 = test_dir.join("test file.txt");
        fs::write(&test_file1, "content").unwrap();

        // Test underscore to hyphen
        let test_file2 = test_dir.join("test_file2.txt");
        fs::write(&test_file2, "content").unwrap();

        // Test mixed separators to hyphen
        let test_file3 = test_dir.join("test_file 3.txt");
        fs::write(&test_file3, "content").unwrap();

        let opts = RenameOptions {
            space_replace: SpaceReplace::Hyphen,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_dir).unwrap();

        assert_eq!(count, 3);
        assert!(test_dir.join("test-file.txt").exists());
        assert!(test_dir.join("test-file2.txt").exists());
        assert!(test_dir.join("test-file-3.txt").exists());
    }

    #[test]
    fn test_add_prefix() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("file.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            add_prefix: Some("new_".to_string()),

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        assert!(test_dir.join("new_file.txt").exists());
        assert!(!test_file.exists());
    }

    #[test]
    fn test_remove_prefix() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("old_file.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            remove_prefix: Some("old_".to_string()),

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        assert!(test_dir.join("file.txt").exists());
        assert!(!test_file.exists());
    }

    #[test]
    fn test_add_suffix() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("file.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            add_suffix: Some("_backup".to_string()),

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        assert!(test_dir.join("file_backup.txt").exists());
        assert!(!test_file.exists());
    }

    #[test]
    fn test_remove_suffix() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("file_old.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            remove_suffix: Some("_old".to_string()),

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        assert!(test_dir.join("file.txt").exists());
        assert!(!test_file.exists());
    }

    #[test]
    fn test_combined_transforms() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("old_Test File.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            remove_prefix: Some("old_".to_string()),

            space_replace: SpaceReplace::Underscore,

            case_transform: CaseTransform::Lowercase,

            add_suffix: Some("_new".to_string()),

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        assert!(test_dir.join("test_file_new.txt").exists());
        assert!(!test_file.exists());
    }

    #[test]
    fn test_dry_run_mode() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("TestFile.txt");
        let original_content = "content";
        fs::write(&test_file, original_content).unwrap();

        let opts = RenameOptions {
            case_transform: CaseTransform::Lowercase,

            dry_run: true,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        // File should still exist and be unchanged in dry run
        assert!(test_file.exists());
        assert_eq!(fs::read_to_string(&test_file).unwrap(), original_content);
    }

    #[test]
    fn test_skip_hidden_files() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let hidden_file = test_dir.join(".hidden.txt");
        fs::write(&hidden_file, "content").unwrap();

        let opts = RenameOptions {
            case_transform: CaseTransform::Uppercase,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&hidden_file).unwrap();

        // Hidden file should be skipped
        assert_eq!(count, 0);
        assert!(hidden_file.exists());
    }

    #[test]
    fn test_recursive_processing() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let sub_dir = test_dir.join("subdir");
        fs::create_dir_all(&sub_dir).unwrap();

        let file1 = test_dir.join("File1.txt");
        let file2 = sub_dir.join("File2.txt");

        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();

        let opts = RenameOptions {
            case_transform: CaseTransform::Lowercase,

            recursive: true,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_dir).unwrap();

        assert_eq!(count, 2);
        assert!(test_dir.join("file1.txt").exists());
        assert!(sub_dir.join("file2.txt").exists());
    }

    #[test]
    fn test_no_extension_file() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("TestFile");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            case_transform: CaseTransform::Lowercase,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        let new_file = test_dir.join("testfile");
        assert!(new_file.exists());
        assert_eq!(fs::read_to_string(&new_file).unwrap(), "content");
    }

    #[test]
    fn test_timestamp_long_format() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        let _ = fs::remove_dir_all(&test_dir); // Clean up first
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("document.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            timestamp_format: TimestampFormat::Long,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);

        // Check that a file with timestamp prefix exists
        // The name should be like: YYYYMMDD-document.txt (hyphen as default)
        let entries: Vec<_> = fs::read_dir(&test_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);

        let renamed_file = entries[0].as_ref().unwrap().path();
        let file_name = renamed_file.file_name().unwrap().to_str().unwrap();

        // Verify format: should start with 8 digits followed by hyphen (default separator)
        assert!(
            file_name.len() >= 9,
            "Filename should have at least 9 characters (YYYYMMDD-)"
        );
        assert!(
            file_name.starts_with(|c: char| c.is_ascii_digit()),
            "Should start with digit"
        );
        assert_eq!(
            &file_name[8..9],
            "-",
            "Should have hyphen after date (default separator)"
        );
        assert!(
            file_name.ends_with("document.txt"),
            "Should end with original name"
        );
    }

    #[test]
    fn test_timestamp_short_format() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        let _ = fs::remove_dir_all(&test_dir); // Clean up first
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("notes.md");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            timestamp_format: TimestampFormat::Short,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);

        // Check that a file with timestamp prefix exists
        // The name should be like: YYMMDD-notes.md (hyphen as default)
        let entries: Vec<_> = fs::read_dir(&test_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);

        let renamed_file = entries[0].as_ref().unwrap().path();
        let file_name = renamed_file.file_name().unwrap().to_str().unwrap();

        // Verify format: should start with 6 digits followed by hyphen (default separator)
        assert!(
            file_name.len() >= 7,
            "Filename should have at least 7 characters (YYMMDD-)"
        );
        assert!(
            file_name.starts_with(|c: char| c.is_ascii_digit()),
            "Should start with digit"
        );
        assert_eq!(
            &file_name[6..7],
            "-",
            "Should have hyphen after date (default separator)"
        );
        assert!(
            file_name.ends_with("notes.md"),
            "Should end with original name"
        );
    }

    #[test]
    fn test_timestamp_with_other_transforms() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("My Document.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            timestamp_format: TimestampFormat::Long,

            space_replace: SpaceReplace::Underscore,

            case_transform: CaseTransform::Lowercase,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);

        // The file should be renamed with timestamp, spaces replaced, and lowercase
        let entries: Vec<_> = fs::read_dir(&test_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);

        let renamed_file = entries[0].as_ref().unwrap().path();
        let file_name = renamed_file.file_name().unwrap().to_str().unwrap();

        // Should have format: YYYYMMDD_my_document.txt
        assert!(file_name.starts_with(|c: char| c.is_ascii_digit()));
        assert!(file_name.contains("my_document.txt"));
        assert!(!file_name.contains(" "));
        assert!(!file_name.contains("My"));
    }

    #[test]
    fn test_timestamp_separator_detection_hyphen() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("my-document-file.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            timestamp_format: TimestampFormat::Long,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);

        let entries: Vec<_> = fs::read_dir(&test_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);

        let renamed_file = entries[0].as_ref().unwrap().path();
        let file_name = renamed_file.file_name().unwrap().to_str().unwrap();

        // Should use hyphen as separator: YYYYMMDD-my-document-file.txt
        assert!(file_name.starts_with(|c: char| c.is_ascii_digit()));
        assert_eq!(
            &file_name[8..9],
            "-",
            "Timestamp should use hyphen separator"
        );
        assert!(file_name.ends_with("my-document-file.txt"));
    }

    #[test]
    fn test_timestamp_separator_detection_underscore() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("my_document_file.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            timestamp_format: TimestampFormat::Short,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);

        let entries: Vec<_> = fs::read_dir(&test_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);

        let renamed_file = entries[0].as_ref().unwrap().path();
        let file_name = renamed_file.file_name().unwrap().to_str().unwrap();

        // Should use underscore as separator: YYMMDD_my_document_file.txt
        assert!(file_name.starts_with(|c: char| c.is_ascii_digit()));
        assert_eq!(
            &file_name[6..7],
            "_",
            "Timestamp should use underscore separator"
        );
        assert!(file_name.ends_with("my_document_file.txt"));
    }

    #[test]
    fn test_timestamp_separator_detection_mixed() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        let _ = fs::remove_dir_all(&test_dir); // Clean up first
        fs::create_dir_all(&test_dir).unwrap();

        // More hyphens than underscores (2 hyphens vs 1 underscore)
        let test_file1 = test_dir.join("my-document-file_v2.txt");
        fs::write(&test_file1, "content").unwrap();

        let opts = RenameOptions {
            timestamp_format: TimestampFormat::Long,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file1).unwrap();

        assert_eq!(count, 1);

        let entries: Vec<_> = fs::read_dir(&test_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);

        let renamed_file = entries[0].as_ref().unwrap().path();
        let file_name = renamed_file.file_name().unwrap().to_str().unwrap();

        // Should use hyphen (more hyphens than underscores)
        assert_eq!(
            &file_name[8..9],
            "-",
            "Should use hyphen for mixed with more hyphens"
        );
    }

    #[test]
    fn test_timestamp_separator_detection_no_separator() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("mydocument.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            timestamp_format: TimestampFormat::Long,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);

        let entries: Vec<_> = fs::read_dir(&test_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);

        let renamed_file = entries[0].as_ref().unwrap().path();
        let file_name = renamed_file.file_name().unwrap().to_str().unwrap();

        // Should default to hyphen when no separators
        assert_eq!(&file_name[8..9], "-", "Should default to hyphen");
        assert!(file_name.ends_with("mydocument.txt"));
    }

    #[test]
    fn test_timestamp_separator_detection_spaces() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("my document file.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            timestamp_format: TimestampFormat::Long,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);

        let entries: Vec<_> = fs::read_dir(&test_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);

        let renamed_file = entries[0].as_ref().unwrap().path();
        let file_name = renamed_file.file_name().unwrap().to_str().unwrap();

        // Should use hyphen for space-separated files
        assert_eq!(
            &file_name[8..9],
            "-",
            "Should use hyphen for space-separated files"
        );
        assert!(file_name.ends_with("my document file.txt"));
    }

    #[test]
    fn test_replace_prefix() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("old_file.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            replace_prefix: Some(("old_".to_string(), "new_".to_string())),

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        assert!(test_dir.join("new_file.txt").exists());
        assert!(!test_file.exists());
    }

    #[test]
    fn test_replace_prefix_no_match() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("other_file.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            replace_prefix: Some(("old_".to_string(), "new_".to_string())),

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        // File should not be renamed since prefix doesn't match
        assert_eq!(count, 0);
        assert!(test_file.exists());
    }

    #[test]
    fn test_replace_suffix() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("file_old.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            replace_suffix: Some(("_old".to_string(), "_new".to_string())),

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        assert!(test_dir.join("file_new.txt").exists());
        assert!(!test_file.exists());
    }

    #[test]
    fn test_replace_suffix_no_match() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("file_other.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            replace_suffix: Some(("_old".to_string(), "_new".to_string())),

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        // File should not be renamed since suffix doesn't match
        assert_eq!(count, 0);
        assert!(test_file.exists());
    }

    #[test]
    fn test_replace_prefix_and_suffix_combined() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("old_file_v1.txt");
        fs::write(&test_file, "content").unwrap();

        let opts = RenameOptions {
            replace_prefix: Some(("old_".to_string(), "new_".to_string())),

            replace_suffix: Some(("_v1".to_string(), "_v2".to_string())),

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_file).unwrap();

        assert_eq!(count, 1);
        assert!(test_dir.join("new_file_v2.txt").exists());
        assert!(!test_file.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_symlinks_skipped_by_default() {
        use std::os::unix::fs::symlink;

        // A unique directory per test: these run in parallel, and a shared

        // fixture path lets them clobber each other. TempDir also cleans up

        // when a test panics, which explicit teardown at the end does not.

        let _tmp = tempfile::tempdir().unwrap();

        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        // Create a regular file with uppercase name
        let target_file = test_dir.join("Target.txt");
        fs::write(&target_file, "content").unwrap();

        // Create a symlink to the file
        let symlink_file = test_dir.join("SymLink.txt");
        symlink(&target_file, &symlink_file).unwrap();

        let mut opts = RenameOptions {
            case_transform: CaseTransform::Lowercase,

            ..Default::default()
        };
        opts.include_symlinks = false; // default

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_dir).unwrap();

        // Only the target file should be renamed, symlink should be skipped
        assert_eq!(count, 1);
        assert!(test_dir.join("target.txt").exists());

        // Check that symlink still has uppercase name by listing directory
        let entries: Vec<_> = fs::read_dir(&test_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            entries.iter().any(|n| n == "SymLink.txt"),
            "Symlink should retain original uppercase name"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_symlinks_included_when_enabled() {
        use std::os::unix::fs::symlink;

        // A unique directory per test: these run in parallel, and a shared

        // fixture path lets them clobber each other. TempDir also cleans up

        // when a test panics, which explicit teardown at the end does not.

        let _tmp = tempfile::tempdir().unwrap();

        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        // Create a regular file (already lowercase)
        let target_file = test_dir.join("target.txt");
        fs::write(&target_file, "content").unwrap();

        // Create a symlink to the file with uppercase name
        let symlink_file = test_dir.join("SymLink.txt");
        symlink(&target_file, &symlink_file).unwrap();

        let opts = RenameOptions {
            case_transform: CaseTransform::Lowercase,

            include_symlinks: true,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_dir).unwrap();

        // Only symlink should be renamed (target is already lowercase)
        assert_eq!(count, 1);
        assert!(test_dir.join("target.txt").exists());

        // Check that symlink was renamed to lowercase by listing directory
        let entries: Vec<_> = fs::read_dir(&test_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            entries.iter().any(|n| n == "symlink.txt"),
            "Symlink should be renamed to lowercase"
        );
        assert!(
            !entries.iter().any(|n| n == "SymLink.txt"),
            "Original uppercase symlink name should be gone"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_with_uppercase_target() {
        use std::os::unix::fs::symlink;

        // A unique directory per test: these run in parallel, and a shared

        // fixture path lets them clobber each other. TempDir also cleans up

        // when a test panics, which explicit teardown at the end does not.

        let _tmp = tempfile::tempdir().unwrap();

        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        // Create a regular file with uppercase
        let target_file = test_dir.join("Target.txt");
        fs::write(&target_file, "content").unwrap();

        // Create a symlink to the file
        let symlink_file = test_dir.join("SymLink.txt");
        symlink(&target_file, &symlink_file).unwrap();

        let opts = RenameOptions {
            case_transform: CaseTransform::Lowercase,

            include_symlinks: true,

            ..Default::default()
        };

        let renamer = FileRenamer::new(opts);
        let count = renamer.process(&test_dir).unwrap();

        // Both should be renamed
        assert_eq!(count, 2);

        // Check files by listing directory (handles case-insensitive filesystems)
        let entries: Vec<_> = fs::read_dir(&test_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            entries.iter().any(|n| n == "target.txt"),
            "Target file should be renamed to lowercase"
        );
        assert!(
            entries.iter().any(|n| n == "symlink.txt"),
            "Symlink should be renamed to lowercase"
        );
    }

    /// `rename_paths` renames what the caller selected, hidden names included,
    /// but never a path inside `.git`.
    #[test]
    fn test_rename_paths_selection_and_git_refusal() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir(dir.join(".git")).unwrap();
        fs::write(dir.join(".git").join("HEAD"), "ref").unwrap();
        fs::write(dir.join(".Hidden.txt"), "x").unwrap();

        let renamer = FileRenamer::new(RenameOptions {
            case_transform: CaseTransform::Uppercase,
            ..Default::default()
        });

        assert!(!renamer
            .rename_file(&dir.join(".Hidden.txt"), false)
            .unwrap());

        let stats = renamer.rename_paths(vec![
            (dir.join(".git").join("HEAD"), false),
            (dir.join(".Hidden.txt"), false),
        ]);
        assert_eq!(stats.renamed, 1);
        let names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&".HIDDEN.txt".to_string()), "{:?}", names);
        let git: Vec<String> = fs::read_dir(dir.join(".git"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(git, ["HEAD"]);
    }
}
