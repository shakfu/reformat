//! Combined processing for multiple transformations in a single pass

use std::path::{Path, PathBuf};

use crate::step::FileTarget;
use crate::{
    CaseTransform, ContentStep, EmojiOptions, EmojiTransformer, FileRenamer, RenameOptions,
    WhitespaceCleaner, WhitespaceOptions,
};

/// Options for combined processing
#[derive(Debug, Clone)]
pub struct CombinedOptions {
    /// Process directories recursively
    pub recursive: bool,
    /// Dry run mode (don't modify files)
    pub dry_run: bool,
    /// Also rename files to lowercase. Off by default: lowercasing
    /// `Cargo.toml`, `Makefile` or `Button.tsx` breaks builds and imports.
    pub lowercase_filenames: bool,
}

impl Default for CombinedOptions {
    fn default() -> Self {
        CombinedOptions {
            recursive: true,
            dry_run: false,
            lowercase_filenames: false,
        }
    }
}

/// Statistics from combined processing
#[derive(Debug, Default)]
pub struct CombinedStats {
    /// Number of files renamed
    pub files_renamed: usize,
    /// Number of files with emoji transformations
    pub files_emoji_transformed: usize,
    /// Number of emoji changes
    pub emoji_changes: usize,
    /// Number of files with whitespace cleaned
    pub files_whitespace_cleaned: usize,
    /// Number of lines with whitespace cleaned
    pub whitespace_lines_cleaned: usize,
}

/// Combined processor that applies multiple transformations in a single pass
pub struct CombinedProcessor {
    options: CombinedOptions,
    renamer: Option<FileRenamer>,
    emojis: EmojiTransformer,
    whitespace: WhitespaceCleaner,
}

impl CombinedProcessor {
    /// Creates a new combined processor with the given options
    pub fn new(options: CombinedOptions) -> Self {
        let renamer = options.lowercase_filenames.then(|| {
            FileRenamer::new(RenameOptions {
                case_transform: CaseTransform::Lowercase,
                recursive: options.recursive,
                dry_run: options.dry_run,
                ..Default::default()
            })
        });

        let emojis = EmojiTransformer::new(EmojiOptions {
            recursive: options.recursive,
            dry_run: options.dry_run,
            ..Default::default()
        });

        let whitespace = WhitespaceCleaner::new(WhitespaceOptions {
            recursive: options.recursive,
            dry_run: options.dry_run,
            ..Default::default()
        });

        CombinedProcessor {
            options,
            renamer,
            emojis,
            whitespace,
        }
    }

    /// Creates a processor with default options
    pub fn with_defaults() -> Self {
        CombinedProcessor::new(CombinedOptions::default())
    }

    /// The content steps this processor applies, in order.
    pub fn content_steps(&self) -> [&dyn ContentStep; 2] {
        [&self.emojis, &self.whitespace]
    }

    /// Processes a directory or file with all transformations
    pub fn process(&self, path: &Path) -> crate::Result<CombinedStats> {
        let mut stats = CombinedStats::default();

        if path.is_file() {
            let skipped = path.file_name().and_then(|n| n.to_str()).is_none_or(|n| {
                crate::walk::is_excluded_component(n, crate::walk::DEFAULT_SKIP_DIRS)
            });
            if !skipped {
                self.process_single_file(path, &mut stats)?;
            }
        } else if path.is_dir() {
            // Collect every file up front: renaming while iterating would
            // invalidate the walk.
            let mut files: Vec<PathBuf> = crate::walk::walk_files(path, self.options.recursive)
                .map(|e| e.path().to_path_buf())
                .collect();

            // Deepest first, then alphabetically, for a reproducible order.
            files.sort_by(|a, b| {
                b.components()
                    .count()
                    .cmp(&a.components().count())
                    .then_with(|| a.cmp(b))
            });

            for file_path in files {
                self.process_single_file(&file_path, &mut stats)?;
            }
        }

        Ok(stats)
    }

    /// Processes a single file with all transformations
    fn process_single_file(&self, path: &Path, stats: &mut CombinedStats) -> crate::Result<()> {
        let mut current_path = path.to_path_buf();

        if let Some(ref renamer) = self.renamer {
            if renamer.rename_file(path, false)? {
                stats.files_renamed += 1;
                if !self.options.dry_run {
                    let lowercase = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?
                        .to_lowercase();
                    current_path = path.with_file_name(lowercase);
                }
            }
        }

        let target = FileTarget::file(&current_path);
        let report = crate::step::run_content_steps(
            &self.content_steps(),
            [target],
            self.options.dry_run,
            &mut |_, _, _| {},
        );
        if let Some(first) = report.errors.first() {
            anyhow::bail!("{}", first);
        }
        let [emojis, whitespace] = &report.steps[..] else {
            unreachable!("two content steps");
        };
        stats.files_emoji_transformed += emojis.files;
        stats.emoji_changes += emojis.units;
        stats.files_whitespace_cleaned += whitespace.files;
        stats.whitespace_lines_cleaned += whitespace.units;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_combined_processing() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        // Create a file with uppercase name, emojis, and trailing whitespace
        let test_file = test_dir.join("TestFile.txt");
        fs::write(&test_file, "Line 1   \nTask done ✅\nLine 3\t\n").unwrap();

        let processor = CombinedProcessor::new(CombinedOptions {
            lowercase_filenames: true,
            ..Default::default()
        });
        let stats = processor.process(&test_file).unwrap();

        // File should be renamed
        assert_eq!(stats.files_renamed, 1);
        let renamed_file = test_dir.join("testfile.txt");
        assert!(renamed_file.exists());

        // Emojis should be transformed
        assert_eq!(stats.files_emoji_transformed, 1);
        let content = fs::read_to_string(&renamed_file).unwrap();
        assert!(content.contains("[x]"));
        assert!(!content.contains("✅"));

        // Whitespace should be cleaned
        assert_eq!(stats.files_whitespace_cleaned, 1);
        assert!(!content.contains("   \n"));
        assert!(!content.contains("\t\n"));
    }

    #[test]
    fn test_combined_dry_run() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("TestFile.txt");
        let original_content = "Line 1   \nTask ✅\n";
        fs::write(&test_file, original_content).unwrap();

        let options = CombinedOptions {
            dry_run: true,

            ..Default::default()
        };

        let processor = CombinedProcessor::new(options);
        let _stats = processor.process(&test_file).unwrap();

        // File should remain unchanged in dry run
        assert!(test_file.exists());
        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, original_content);
    }

    #[test]
    fn test_combined_recursive() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let sub_dir = test_dir.join("subdir");
        fs::create_dir_all(&sub_dir).unwrap();

        let file1 = test_dir.join("File1.txt");
        let file2 = sub_dir.join("File2.md");

        fs::write(&file1, "Text   \n✅ Done\n").unwrap();
        fs::write(&file2, "More text\t\n☐ Todo\n").unwrap();

        let processor = CombinedProcessor::new(CombinedOptions {
            lowercase_filenames: true,
            ..Default::default()
        });
        let stats = processor.process(&test_dir).unwrap();

        // Both files should be processed
        assert_eq!(stats.files_renamed, 2);
        assert_eq!(stats.files_emoji_transformed, 2);
        assert_eq!(stats.files_whitespace_cleaned, 2);

        // Check renamed files exist
        assert!(test_dir.join("file1.txt").exists());
        assert!(sub_dir.join("file2.md").exists());
    }

    #[test]
    fn test_combined_non_recursive() {
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

        fs::write(&file1, "Text   \n").unwrap();
        fs::write(&file2, "More   \n").unwrap();

        let options = CombinedOptions {
            recursive: false,
            lowercase_filenames: true,

            ..Default::default()
        };

        let processor = CombinedProcessor::new(options);
        let stats = processor.process(&test_dir).unwrap();

        // Only top-level file should be processed
        assert_eq!(stats.files_renamed, 1);
        assert!(test_dir.join("file1.txt").exists());

        // Check that subdirectory file was NOT renamed (should still be File2.txt)
        // On case-insensitive filesystems, both paths refer to the same file, so check actual filename
        let entries: Vec<_> = fs::read_dir(&sub_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let actual_name = entries[0].as_ref().unwrap().file_name();
        assert_eq!(
            actual_name.to_str().unwrap(),
            "File2.txt",
            "Subdirectory file should not be renamed"
        );
    }

    /// Lowercasing build manifests and source files broke builds, so the
    /// default processor leaves names alone.
    #[test]
    fn test_default_does_not_rename() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]\n").unwrap();
        fs::write(tmp.path().join("README.md"), "Title  \n").unwrap();

        let stats = CombinedProcessor::with_defaults()
            .process(tmp.path())
            .unwrap();

        assert_eq!(stats.files_renamed, 0);
        let mut names: Vec<String> = fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["Cargo.toml", "README.md"]);
        assert_eq!(stats.files_whitespace_cleaned, 1);
        assert_eq!(
            fs::read_to_string(tmp.path().join("README.md")).unwrap(),
            "Title\n"
        );
    }
}
