//! File grouping transformer - organizes files by common prefix into subdirectories

use crate::changes::ChangeRecord;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Options for file grouping
#[derive(Debug, Clone, Serialize)]
pub struct GroupOptions {
    /// Separator character that divides prefix from the rest of the filename (default: '_')
    pub separator: char,
    /// Minimum number of files with same prefix to create a group (default: 2)
    pub min_count: usize,
    /// Remove the prefix from filenames after moving to subdirectory
    pub strip_prefix: bool,
    /// Use the suffix (part after last separator) as filename, rest as directory
    /// When true, splits at the LAST separator instead of the first
    pub from_suffix: bool,
    /// Process directories recursively
    pub recursive: bool,
    /// Dry run mode (don't move files or create directories)
    pub dry_run: bool,
}

impl Default for GroupOptions {
    fn default() -> Self {
        GroupOptions {
            separator: '_',
            min_count: 2,
            strip_prefix: false,
            from_suffix: false,
            recursive: false,
            dry_run: false,
        }
    }
}

/// Statistics from a grouping operation
#[derive(Debug, Clone, Default)]
pub struct GroupStats {
    /// Number of directories created
    pub dirs_created: usize,
    /// Number of files moved
    pub files_moved: usize,
    /// Number of files renamed (prefix stripped)
    pub files_renamed: usize,
}

/// Result of a grouping operation including change tracking
#[derive(Debug, Clone)]
pub struct GroupResult {
    /// Statistics from the operation
    pub stats: GroupStats,
    /// Record of all changes made (for reference fixing)
    pub changes: ChangeRecord,
}

/// File grouper for organizing files by prefix into subdirectories
pub struct FileGrouper {
    options: GroupOptions,
}

impl FileGrouper {
    /// Creates a new file grouper with the given options
    pub fn new(options: GroupOptions) -> Self {
        FileGrouper { options }
    }

    /// Creates a grouper with default options
    pub fn with_defaults() -> Self {
        FileGrouper {
            options: GroupOptions::default(),
        }
    }

    /// Extracts the prefix from a filename based on the separator
    /// When from_suffix is true, splits at the LAST separator (e.g., "a_b_c" -> "a_b")
    /// Otherwise splits at the FIRST separator (e.g., "a_b_c" -> "a")
    /// Returns None if no separator is found
    fn extract_prefix(&self, filename: &str) -> Option<String> {
        // Get the stem (filename without extension) for suffix-based splitting
        let (stem, _ext) = if self.options.from_suffix {
            // For from_suffix mode, we need to work with the stem to find the last separator
            // before the extension
            if let Some(dot_pos) = filename.rfind('.') {
                (&filename[..dot_pos], Some(&filename[dot_pos..]))
            } else {
                (filename, None)
            }
        } else {
            (filename, None)
        };

        let search_str = if self.options.from_suffix {
            stem
        } else {
            filename
        };

        let pos = if self.options.from_suffix {
            // Find the LAST occurrence of the separator in the stem
            search_str.rfind(self.options.separator)
        } else {
            // Find the FIRST occurrence of the separator
            search_str.find(self.options.separator)
        };

        if let Some(pos) = pos {
            let prefix = &search_str[..pos];
            // Only return prefix if it's not empty and there's something after the separator
            if !prefix.is_empty() && pos + 1 < search_str.len() {
                return Some(prefix.to_string());
            }
        }
        None
    }

    /// Strips the prefix and separator from a filename
    /// When from_suffix is true, this strips everything up to and including the last separator
    fn strip_prefix_from_name(&self, filename: &str, prefix: &str) -> String {
        let prefix_with_sep = format!("{}{}", prefix, self.options.separator);
        if filename.starts_with(&prefix_with_sep) {
            filename[prefix_with_sep.len()..].to_string()
        } else {
            filename.to_string()
        }
    }

    /// Analyzes a directory and returns a map of prefix -> list of files
    fn analyze_directory(&self, dir: &Path) -> crate::Result<HashMap<String, Vec<PathBuf>>> {
        let mut prefix_map: HashMap<String, Vec<PathBuf>> = HashMap::new();

        let entries = fs::read_dir(dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only process files, not directories
            if !path.is_file() {
                continue;
            }

            // Skip hidden files
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }

                // Extract prefix
                if let Some(prefix) = self.extract_prefix(name) {
                    prefix_map.entry(prefix).or_default().push(path);
                }
            }
        }

        Ok(prefix_map)
    }

    /// Processes a single directory (non-recursive part)
    fn process_directory_single(
        &self,
        dir: &Path,
        base_dir: &Path,
        changes: &mut ChangeRecord,
    ) -> crate::Result<GroupStats> {
        let mut stats = GroupStats::default();

        // Analyze directory for prefixes
        let prefix_map = self.analyze_directory(dir)?;

        // Process each prefix group that meets the minimum count
        for (prefix, files) in prefix_map {
            if files.len() < self.options.min_count {
                continue;
            }

            // Create subdirectory path
            let subdir = dir.join(&prefix);

            // Create directory if it doesn't exist
            if !subdir.exists() {
                if self.options.dry_run {
                    println!("Would create directory: {}", subdir.display());
                } else {
                    fs::create_dir(&subdir)?;
                    println!("Created directory: {}", subdir.display());
                }
                // Record the directory creation (relative to base_dir)
                let rel_path = subdir.strip_prefix(base_dir).unwrap_or(&subdir);
                changes.add_directory_created(&rel_path.to_string_lossy());
                stats.dirs_created += 1;
            }

            // Move each file to the subdirectory
            for file_path in files {
                let filename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

                // Determine the new filename (with or without prefix)
                let new_filename = if self.options.strip_prefix {
                    self.strip_prefix_from_name(filename, &prefix)
                } else {
                    filename.to_string()
                };

                let new_path = subdir.join(&new_filename);

                // Check if target already exists
                if new_path.exists() {
                    eprintln!(
                        "Warning: Target file already exists, skipping: {}",
                        new_path.display()
                    );
                    continue;
                }

                // Calculate relative paths for change tracking
                let old_rel = file_path.strip_prefix(base_dir).unwrap_or(&file_path);
                let new_rel = new_path.strip_prefix(base_dir).unwrap_or(&new_path);

                if self.options.dry_run {
                    if self.options.strip_prefix && new_filename != filename {
                        println!(
                            "Would move and rename '{}' -> '{}'",
                            file_path.display(),
                            new_path.display()
                        );
                        stats.files_renamed += 1;
                    } else {
                        println!(
                            "Would move '{}' -> '{}'",
                            file_path.display(),
                            new_path.display()
                        );
                    }
                } else {
                    fs::rename(&file_path, &new_path)?;
                    if self.options.strip_prefix && new_filename != filename {
                        println!(
                            "Moved and renamed '{}' -> '{}'",
                            file_path.display(),
                            new_path.display()
                        );
                        stats.files_renamed += 1;
                    } else {
                        println!(
                            "Moved '{}' -> '{}'",
                            file_path.display(),
                            new_path.display()
                        );
                    }
                }

                // Record the file move
                changes.add_file_moved(&old_rel.to_string_lossy(), &new_rel.to_string_lossy());
                stats.files_moved += 1;
            }
        }

        Ok(stats)
    }

    /// Processes a directory, grouping files by prefix into subdirectories
    /// Returns GroupStats for backward compatibility
    pub fn process(&self, path: &Path) -> crate::Result<GroupStats> {
        let result = self.process_with_changes(path)?;
        Ok(result.stats)
    }

    /// Processes a directory and returns full result with change tracking
    pub fn process_with_changes(&self, path: &Path) -> crate::Result<GroupResult> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let mut changes = ChangeRecord::new("group", &path).with_options(&self.options);
        let mut total_stats = GroupStats::default();

        if !path.is_dir() {
            return Err(anyhow::anyhow!(
                "Path is not a directory: {}",
                path.display()
            ));
        }

        // If recursive, collect subdirectories BEFORE processing
        // This prevents processing newly created group directories
        let subdirs_to_process: Vec<PathBuf> = if self.options.recursive {
            fs::read_dir(&path)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter(|e| {
                    // Skip hidden directories
                    e.file_name()
                        .to_str()
                        .map(|s| !s.starts_with('.'))
                        .unwrap_or(false)
                })
                .map(|e| e.path())
                .collect()
        } else {
            Vec::new()
        };

        // Process the target directory
        let stats = self.process_directory_single(&path, &path, &mut changes)?;
        total_stats.dirs_created += stats.dirs_created;
        total_stats.files_moved += stats.files_moved;
        total_stats.files_renamed += stats.files_renamed;

        // Process pre-existing subdirectories (not newly created ones)
        for subdir_path in subdirs_to_process {
            // Skip if the directory was removed or doesn't exist anymore
            if !subdir_path.is_dir() {
                continue;
            }
            let stats = self.process_directory_single(&subdir_path, &path, &mut changes)?;
            total_stats.dirs_created += stats.dirs_created;
            total_stats.files_moved += stats.files_moved;
            total_stats.files_renamed += stats.files_renamed;
        }

        Ok(GroupResult {
            stats: total_stats,
            changes,
        })
    }

    /// Preview what groups would be created without making changes
    pub fn preview(&self, path: &Path) -> crate::Result<HashMap<String, Vec<String>>> {
        if !path.is_dir() {
            return Err(anyhow::anyhow!(
                "Path is not a directory: {}",
                path.display()
            ));
        }

        let prefix_map = self.analyze_directory(path)?;

        // Filter by min_count and convert PathBuf to String
        let result: HashMap<String, Vec<String>> = prefix_map
            .into_iter()
            .filter(|(_, files)| files.len() >= self.options.min_count)
            .map(|(prefix, files)| {
                let filenames: Vec<String> = files
                    .iter()
                    .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
                    .collect();
                (prefix, filenames)
            })
            .collect();

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    // Counter to ensure unique test directories even when tests run in parallel
    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn create_test_dir(test_name: &str) -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let test_dir = std::env::temp_dir().join(format!(
            "reformat_group_{}_{}_{}",
            test_name,
            std::process::id(),
            counter
        ));
        // Clean up any existing directory first
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).unwrap();
        test_dir
    }

    #[test]
    fn test_extract_prefix() {
        let grouper = FileGrouper::with_defaults();

        assert_eq!(
            grouper.extract_prefix("wbs_create.tmpl"),
            Some("wbs".to_string())
        );
        assert_eq!(
            grouper.extract_prefix("work_package_list.tmpl"),
            Some("work".to_string())
        );
        assert_eq!(grouper.extract_prefix("noprefix.txt"), None);
        assert_eq!(grouper.extract_prefix("_leadingunderscore.txt"), None);
        assert_eq!(grouper.extract_prefix("trailing_"), None);
    }

    #[test]
    fn test_extract_prefix_custom_separator() {
        let mut options = GroupOptions::default();
        options.separator = '-';
        let grouper = FileGrouper::new(options);

        assert_eq!(
            grouper.extract_prefix("wbs-create.tmpl"),
            Some("wbs".to_string())
        );
        assert_eq!(grouper.extract_prefix("wbs_create.tmpl"), None);
    }

    #[test]
    fn test_strip_prefix_from_name() {
        let grouper = FileGrouper::with_defaults();

        assert_eq!(
            grouper.strip_prefix_from_name("wbs_create.tmpl", "wbs"),
            "create.tmpl"
        );
        assert_eq!(
            grouper.strip_prefix_from_name("work_package_list.tmpl", "work"),
            "package_list.tmpl"
        );
    }

    #[test]
    fn test_basic_grouping() {
        let test_dir = create_test_dir("basic");

        // Create test files
        fs::write(test_dir.join("wbs_create.tmpl"), "content").unwrap();
        fs::write(test_dir.join("wbs_delete.tmpl"), "content").unwrap();
        fs::write(test_dir.join("wbs_list.tmpl"), "content").unwrap();
        fs::write(test_dir.join("other_file.txt"), "content").unwrap();

        let mut options = GroupOptions::default();
        options.min_count = 2;

        let grouper = FileGrouper::new(options);
        let stats = grouper.process(&test_dir).unwrap();

        assert_eq!(stats.dirs_created, 1);
        assert_eq!(stats.files_moved, 3);
        assert!(test_dir.join("wbs").is_dir());
        assert!(test_dir.join("wbs").join("wbs_create.tmpl").exists());
        assert!(test_dir.join("wbs").join("wbs_delete.tmpl").exists());
        assert!(test_dir.join("wbs").join("wbs_list.tmpl").exists());
        // other_file.txt should not be moved (only 1 file with "other" prefix)
        assert!(test_dir.join("other_file.txt").exists());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_grouping_with_strip_prefix() {
        let test_dir = create_test_dir("strip");

        // Create test files
        fs::write(test_dir.join("wbs_create.tmpl"), "content").unwrap();
        fs::write(test_dir.join("wbs_delete.tmpl"), "content").unwrap();

        let mut options = GroupOptions::default();
        options.strip_prefix = true;

        let grouper = FileGrouper::new(options);
        let stats = grouper.process(&test_dir).unwrap();

        assert_eq!(stats.dirs_created, 1);
        assert_eq!(stats.files_moved, 2);
        assert_eq!(stats.files_renamed, 2);
        assert!(test_dir.join("wbs").join("create.tmpl").exists());
        assert!(test_dir.join("wbs").join("delete.tmpl").exists());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_dry_run_mode() {
        let test_dir = create_test_dir("dryrun");

        // Create test files
        fs::write(test_dir.join("abc_create.tmpl"), "content").unwrap();
        fs::write(test_dir.join("abc_delete.tmpl"), "content").unwrap();

        let mut options = GroupOptions::default();
        options.dry_run = true;

        let grouper = FileGrouper::new(options);
        let stats = grouper.process(&test_dir).unwrap();

        assert_eq!(stats.dirs_created, 1);
        assert_eq!(stats.files_moved, 2);
        // Directory should NOT exist in dry run
        assert!(!test_dir.join("abc").exists());
        // Files should still be in original location
        assert!(test_dir.join("abc_create.tmpl").exists());
        assert!(test_dir.join("abc_delete.tmpl").exists());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_min_count_threshold() {
        let test_dir = create_test_dir("mincount");

        // Create test files - only 2 with same prefix
        fs::write(test_dir.join("xyz_create.tmpl"), "content").unwrap();
        fs::write(test_dir.join("xyz_delete.tmpl"), "content").unwrap();

        let mut options = GroupOptions::default();
        options.min_count = 3; // Require at least 3 files

        let grouper = FileGrouper::new(options);
        let stats = grouper.process(&test_dir).unwrap();

        // Nothing should be grouped since min_count is 3
        assert_eq!(stats.dirs_created, 0);
        assert_eq!(stats.files_moved, 0);

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_multiple_prefixes() {
        let test_dir = create_test_dir("multiple");

        // Create test files with different prefixes
        fs::write(test_dir.join("aaa_create.tmpl"), "content").unwrap();
        fs::write(test_dir.join("aaa_delete.tmpl"), "content").unwrap();
        fs::write(test_dir.join("bbb_create.tmpl"), "content").unwrap();
        fs::write(test_dir.join("bbb_delete.tmpl"), "content").unwrap();

        let grouper = FileGrouper::with_defaults();
        let stats = grouper.process(&test_dir).unwrap();

        assert_eq!(stats.dirs_created, 2);
        assert_eq!(stats.files_moved, 4);
        assert!(test_dir.join("aaa").is_dir());
        assert!(test_dir.join("bbb").is_dir());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_preview() {
        let test_dir = create_test_dir("preview");

        // Create test files
        fs::write(test_dir.join("pre_create.tmpl"), "content").unwrap();
        fs::write(test_dir.join("pre_delete.tmpl"), "content").unwrap();
        fs::write(test_dir.join("other.txt"), "content").unwrap();

        let grouper = FileGrouper::with_defaults();
        let preview = grouper.preview(&test_dir).unwrap();

        assert_eq!(preview.len(), 1);
        assert!(preview.contains_key("pre"));
        assert_eq!(preview.get("pre").unwrap().len(), 2);

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_skip_hidden_files() {
        let test_dir = create_test_dir("hidden");

        // Create test files including hidden ones
        fs::write(test_dir.join("hid_create.tmpl"), "content").unwrap();
        fs::write(test_dir.join("hid_delete.tmpl"), "content").unwrap();
        fs::write(test_dir.join(".hid_hidden.tmpl"), "content").unwrap();

        let grouper = FileGrouper::with_defaults();
        let stats = grouper.process(&test_dir).unwrap();

        assert_eq!(stats.files_moved, 2);
        // Hidden file should still exist in original location
        assert!(test_dir.join(".hid_hidden.tmpl").exists());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_from_suffix_basic() {
        let test_dir = create_test_dir("from_suffix");

        // Create test files with multi-part prefix
        fs::write(test_dir.join("activity_relationships_list.tmpl"), "content").unwrap();
        fs::write(
            test_dir.join("activity_relationships_create.tmpl"),
            "content",
        )
        .unwrap();
        fs::write(
            test_dir.join("activity_relationships_delete.tmpl"),
            "content",
        )
        .unwrap();

        let mut options = GroupOptions::default();
        options.from_suffix = true;
        // from_suffix implies strip_prefix for proper behavior
        options.strip_prefix = true;

        let grouper = FileGrouper::new(options);
        let stats = grouper.process(&test_dir).unwrap();

        assert_eq!(stats.dirs_created, 1);
        assert_eq!(stats.files_moved, 3);
        assert_eq!(stats.files_renamed, 3);

        // Directory should be named after the full prefix (everything before last separator)
        assert!(test_dir.join("activity_relationships").is_dir());

        // Files should be renamed to just the suffix + extension
        assert!(test_dir
            .join("activity_relationships")
            .join("list.tmpl")
            .exists());
        assert!(test_dir
            .join("activity_relationships")
            .join("create.tmpl")
            .exists());
        assert!(test_dir
            .join("activity_relationships")
            .join("delete.tmpl")
            .exists());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_from_suffix_mixed_prefixes() {
        let test_dir = create_test_dir("from_suffix_mixed");

        // Create test files with different multi-part prefixes
        fs::write(test_dir.join("user_profile_edit.tmpl"), "content").unwrap();
        fs::write(test_dir.join("user_profile_view.tmpl"), "content").unwrap();
        fs::write(test_dir.join("project_settings_edit.tmpl"), "content").unwrap();
        fs::write(test_dir.join("project_settings_view.tmpl"), "content").unwrap();

        let mut options = GroupOptions::default();
        options.from_suffix = true;
        options.strip_prefix = true;

        let grouper = FileGrouper::new(options);
        let stats = grouper.process(&test_dir).unwrap();

        assert_eq!(stats.dirs_created, 2);
        assert_eq!(stats.files_moved, 4);

        // Check both directories were created
        assert!(test_dir.join("user_profile").is_dir());
        assert!(test_dir.join("project_settings").is_dir());

        // Check files are in the right places
        assert!(test_dir.join("user_profile").join("edit.tmpl").exists());
        assert!(test_dir.join("user_profile").join("view.tmpl").exists());
        assert!(test_dir.join("project_settings").join("edit.tmpl").exists());
        assert!(test_dir.join("project_settings").join("view.tmpl").exists());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_from_suffix_vs_default() {
        // Test that from_suffix produces different results than default
        let test_dir = create_test_dir("suffix_vs_default");

        // Create test files
        fs::write(test_dir.join("a_b_c.txt"), "content").unwrap();
        fs::write(test_dir.join("a_b_d.txt"), "content").unwrap();

        // With default behavior (split at first separator)
        let mut options = GroupOptions::default();
        options.strip_prefix = true;

        let grouper = FileGrouper::new(options);
        let stats = grouper.process(&test_dir).unwrap();

        assert_eq!(stats.dirs_created, 1);
        // Directory is "a", files are "b_c.txt" and "b_d.txt"
        assert!(test_dir.join("a").is_dir());
        assert!(test_dir.join("a").join("b_c.txt").exists());
        assert!(test_dir.join("a").join("b_d.txt").exists());

        let _ = fs::remove_dir_all(&test_dir);

        // Now with from_suffix (split at last separator)
        let test_dir2 = create_test_dir("suffix_vs_default2");
        fs::write(test_dir2.join("a_b_c.txt"), "content").unwrap();
        fs::write(test_dir2.join("a_b_d.txt"), "content").unwrap();

        let mut options2 = GroupOptions::default();
        options2.from_suffix = true;
        options2.strip_prefix = true;

        let grouper2 = FileGrouper::new(options2);
        let stats2 = grouper2.process(&test_dir2).unwrap();

        assert_eq!(stats2.dirs_created, 1);
        // Directory is "a_b", files are "c.txt" and "d.txt"
        assert!(test_dir2.join("a_b").is_dir());
        assert!(test_dir2.join("a_b").join("c.txt").exists());
        assert!(test_dir2.join("a_b").join("d.txt").exists());

        let _ = fs::remove_dir_all(&test_dir2);
    }

    #[test]
    fn test_extract_prefix_from_suffix() {
        let mut options = GroupOptions::default();
        options.from_suffix = true;
        let grouper = FileGrouper::new(options);

        // With from_suffix, should return everything before the LAST separator
        assert_eq!(
            grouper.extract_prefix("activity_relationships_list.tmpl"),
            Some("activity_relationships".to_string())
        );
        assert_eq!(grouper.extract_prefix("a_b_c.txt"), Some("a_b".to_string()));
        assert_eq!(
            grouper.extract_prefix("single_part.txt"),
            Some("single".to_string())
        );
        // No separator
        assert_eq!(grouper.extract_prefix("noseparator.txt"), None);
    }

    #[test]
    fn test_existing_directory() {
        let test_dir = create_test_dir("existing");

        // Create the target directory first
        fs::create_dir(test_dir.join("exist")).unwrap();

        // Create test files
        fs::write(test_dir.join("exist_create.tmpl"), "content").unwrap();
        fs::write(test_dir.join("exist_delete.tmpl"), "content").unwrap();

        let grouper = FileGrouper::with_defaults();
        let stats = grouper.process(&test_dir).unwrap();

        // Directory already existed, so dirs_created should be 0
        assert_eq!(stats.dirs_created, 0);
        assert_eq!(stats.files_moved, 2);
        assert!(test_dir.join("exist").join("exist_create.tmpl").exists());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_recursive_processing() {
        let test_dir = create_test_dir("recursive");

        // Create a subdirectory with files
        let sub_dir = test_dir.join("templates");
        fs::create_dir_all(&sub_dir).unwrap();

        // Create files in root with prefix that won't conflict
        fs::write(test_dir.join("top_file1.txt"), "content").unwrap();
        fs::write(test_dir.join("top_file2.txt"), "content").unwrap();

        // Create files in subdirectory
        fs::write(sub_dir.join("sub_create.tmpl"), "content").unwrap();
        fs::write(sub_dir.join("sub_delete.tmpl"), "content").unwrap();

        let mut options = GroupOptions::default();
        options.recursive = true;

        let grouper = FileGrouper::new(options);
        let stats = grouper.process(&test_dir).unwrap();

        // Should create groups in both directories
        assert_eq!(stats.dirs_created, 2); // "top" in root, "sub" in templates
        assert!(test_dir.join("top").is_dir());
        assert!(sub_dir.join("sub").is_dir());
        // Verify files are in the right places
        assert!(test_dir.join("top").join("top_file1.txt").exists());
        assert!(sub_dir.join("sub").join("sub_create.tmpl").exists());

        let _ = fs::remove_dir_all(&test_dir);
    }
}
