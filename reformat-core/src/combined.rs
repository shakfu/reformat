//! Combined processing for multiple transformations in a single pass

use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::{
    CaseTransform, EmojiOptions, EmojiTransformer, FileRenamer, RenameOptions, WhitespaceCleaner,
    WhitespaceOptions,
};

/// Options for combined processing
#[derive(Debug, Clone)]
pub struct CombinedOptions {
    /// Process directories recursively
    pub recursive: bool,
    /// Dry run mode (don't modify files)
    pub dry_run: bool,
}

impl Default for CombinedOptions {
    fn default() -> Self {
        CombinedOptions {
            recursive: true,
            dry_run: false,
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
    rename_options: RenameOptions,
    emoji_options: EmojiOptions,
    whitespace_options: WhitespaceOptions,
}

impl CombinedProcessor {
    /// Creates a new combined processor with the given options
    pub fn new(options: CombinedOptions) -> Self {
        let rename_options = RenameOptions {
            case_transform: CaseTransform::Lowercase,
            recursive: options.recursive,
            dry_run: options.dry_run,
            ..Default::default()
        };

        let emoji_options = EmojiOptions {
            recursive: options.recursive,
            dry_run: options.dry_run,
            ..Default::default()
        };

        let whitespace_options = WhitespaceOptions {
            recursive: options.recursive,
            dry_run: options.dry_run,
            ..Default::default()
        };

        CombinedProcessor {
            options,
            rename_options,
            emoji_options,
            whitespace_options,
        }
    }

    /// Creates a processor with default options
    pub fn with_defaults() -> Self {
        CombinedProcessor::new(CombinedOptions::default())
    }

    /// Processes a directory or file with all transformations
    pub fn process(&self, path: &Path) -> crate::Result<CombinedStats> {
        let mut stats = CombinedStats::default();

        if path.is_file() {
            self.process_single_file(path, &mut stats)?;
        } else if path.is_dir() {
            if self.options.recursive {
                // Collect all files first to avoid iterator invalidation during renames
                let mut files: Vec<PathBuf> = WalkDir::new(path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .map(|e| e.path().to_path_buf())
                    .collect();

                // Sort by depth (deepest first) to avoid parent directory rename issues
                files.sort_by_key(|b| std::cmp::Reverse(b.components().count()));

                for file_path in files {
                    self.process_single_file(&file_path, &mut stats)?;
                }
            } else {
                let mut files: Vec<PathBuf> = fs::read_dir(path)?
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.is_file())
                    .collect();

                // Sort for consistent processing
                files.sort();

                for file_path in files {
                    self.process_single_file(&file_path, &mut stats)?;
                }
            }
        }

        Ok(stats)
    }

    /// Processes a single file with all transformations
    fn process_single_file(&self, path: &Path, stats: &mut CombinedStats) -> crate::Result<()> {
        // Step 1: Rename file (lowercase)
        // Combined processor only handles regular files, not symlinks
        let renamer = FileRenamer::new(self.rename_options.clone());
        let renamed = renamer.rename_file(path, false)?;
        if renamed {
            stats.files_renamed += 1;
        }

        // Determine the current path (may have been renamed)
        let current_path = if renamed && !self.options.dry_run {
            // Calculate the new path after renaming
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

            let lowercase_name = file_name.to_lowercase();
            let parent = path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("No parent directory"))?;
            parent.join(lowercase_name)
        } else {
            path.to_path_buf()
        };

        // Step 2: Transform emojis
        let emoji_transformer = EmojiTransformer::new(self.emoji_options.clone());
        let emoji_changes = emoji_transformer.transform_file(&current_path)?;
        if emoji_changes > 0 {
            stats.files_emoji_transformed += 1;
            stats.emoji_changes += emoji_changes;
        }

        // Step 3: Clean whitespace
        let whitespace_cleaner = WhitespaceCleaner::new(self.whitespace_options.clone());
        let lines_cleaned = whitespace_cleaner.clean_file(&current_path)?;
        if lines_cleaned > 0 {
            stats.files_whitespace_cleaned += 1;
            stats.whitespace_lines_cleaned += lines_cleaned;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_combined_processing() {
        let test_dir = std::env::temp_dir().join("reformat_combined_test");
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).unwrap();

        // Create a file with uppercase name, emojis, and trailing whitespace
        let test_file = test_dir.join("TestFile.txt");
        fs::write(&test_file, "Line 1   \nTask done ✅\nLine 3\t\n").unwrap();

        let processor = CombinedProcessor::with_defaults();
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

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn test_combined_dry_run() {
        let test_dir = std::env::temp_dir().join("reformat_combined_dry");
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("TestFile.txt");
        let original_content = "Line 1   \nTask ✅\n";
        fs::write(&test_file, original_content).unwrap();

        let mut options = CombinedOptions::default();
        options.dry_run = true;

        let processor = CombinedProcessor::new(options);
        let _stats = processor.process(&test_file).unwrap();

        // File should remain unchanged in dry run
        assert!(test_file.exists());
        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, original_content);

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn test_combined_recursive() {
        let test_dir = std::env::temp_dir().join("reformat_combined_recursive");
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).unwrap();

        let sub_dir = test_dir.join("subdir");
        fs::create_dir_all(&sub_dir).unwrap();

        let file1 = test_dir.join("File1.txt");
        let file2 = sub_dir.join("File2.md");

        fs::write(&file1, "Text   \n✅ Done\n").unwrap();
        fs::write(&file2, "More text\t\n☐ Todo\n").unwrap();

        let processor = CombinedProcessor::with_defaults();
        let stats = processor.process(&test_dir).unwrap();

        // Both files should be processed
        assert_eq!(stats.files_renamed, 2);
        assert_eq!(stats.files_emoji_transformed, 2);
        assert_eq!(stats.files_whitespace_cleaned, 2);

        // Check renamed files exist
        assert!(test_dir.join("file1.txt").exists());
        assert!(sub_dir.join("file2.md").exists());

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn test_combined_non_recursive() {
        let test_dir = std::env::temp_dir().join("reformat_combined_nonrec");
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).unwrap();

        let sub_dir = test_dir.join("subdir");
        fs::create_dir_all(&sub_dir).unwrap();

        let file1 = test_dir.join("File1.txt");
        let file2 = sub_dir.join("File2.txt");

        fs::write(&file1, "Text   \n").unwrap();
        fs::write(&file2, "More   \n").unwrap();

        let mut options = CombinedOptions::default();
        options.recursive = false;

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

        fs::remove_dir_all(&test_dir).unwrap();
    }
}
