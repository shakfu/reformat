//! Reference scanning and fixing for broken file references
//!
//! This module provides functionality to scan codebases for references to
//! moved/renamed files and generate fixes for those references.

use crate::changes::ChangeRecord;
use aho_corasick::AhoCorasick;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A proposed fix for a broken reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceFix {
    /// File containing the reference
    pub file: String,
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: usize,
    /// The line content with context
    pub context: String,
    /// The old reference that needs to be fixed
    pub old_reference: String,
    /// The new reference to replace it with
    pub new_reference: String,
}

/// Collection of fixes to be applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixRecord {
    /// Source of the changes that caused these fixes
    pub generated_from: String,
    /// ISO 8601 timestamp of when the scan was performed
    pub timestamp: String,
    /// Directories that were scanned
    pub scan_directories: Vec<String>,
    /// List of proposed fixes
    pub fixes: Vec<ReferenceFix>,
}

impl FixRecord {
    /// Creates a new fix record
    pub fn new(generated_from: &str, scan_directories: &[PathBuf]) -> Self {
        let timestamp = chrono::Utc::now().to_rfc3339();
        FixRecord {
            generated_from: generated_from.to_string(),
            timestamp,
            scan_directories: scan_directories
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            fixes: Vec::new(),
        }
    }

    /// Returns true if there are no fixes
    pub fn is_empty(&self) -> bool {
        self.fixes.is_empty()
    }

    /// Returns the number of fixes
    pub fn len(&self) -> usize {
        self.fixes.len()
    }

    /// Writes the fix record to a JSON file
    pub fn write_to_file(&self, path: &Path) -> crate::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Reads a fix record from a JSON file
    pub fn read_from_file(path: &Path) -> crate::Result<Self> {
        let json = fs::read_to_string(path)?;
        let record: FixRecord = serde_json::from_str(&json)?;
        Ok(record)
    }
}

/// Options for reference scanning
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// File extensions to scan (empty means all text files)
    pub extensions: Vec<String>,
    /// Directories/patterns to exclude from scanning
    pub exclude_patterns: Vec<String>,
    /// Whether to scan recursively
    pub recursive: bool,
    /// Whether to print verbose output during scanning
    pub verbose: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            extensions: vec![
                ".go".to_string(),
                ".py".to_string(),
                ".js".to_string(),
                ".ts".to_string(),
                ".jsx".to_string(),
                ".tsx".to_string(),
                ".rs".to_string(),
                ".java".to_string(),
                ".c".to_string(),
                ".cpp".to_string(),
                ".h".to_string(),
                ".hpp".to_string(),
                ".html".to_string(),
                ".tmpl".to_string(),
                ".yaml".to_string(),
                ".yml".to_string(),
                ".json".to_string(),
                ".toml".to_string(),
                ".xml".to_string(),
                ".md".to_string(),
                ".txt".to_string(),
                ".cfg".to_string(),
                ".conf".to_string(),
                ".ini".to_string(),
            ],
            exclude_patterns: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                "vendor".to_string(),
                "__pycache__".to_string(),
                ".venv".to_string(),
                "dist".to_string(),
                "build".to_string(),
            ],
            recursive: true,
            verbose: false,
        }
    }
}

/// Reference scanner for finding broken references after file moves
pub struct ReferenceScanner {
    options: ScanOptions,
    /// Map of old filename -> new path
    file_moves: HashMap<String, String>,
    /// Aho-Corasick automaton for O(n) multi-pattern matching
    automaton: AhoCorasick,
    /// Ordered list of patterns (index matches automaton pattern indices)
    patterns: Vec<String>,
}

impl ReferenceScanner {
    /// Creates a new reference scanner from a change record
    pub fn from_change_record(record: &ChangeRecord, options: ScanOptions) -> Self {
        let mut file_moves = HashMap::new();

        for (from, to) in record.file_moves() {
            // Extract just the filename from the 'from' path
            let from_filename = Path::new(from)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(from);

            file_moves.insert(from_filename.to_string(), to.to_string());

            // Also add the full path as a key
            if from != from_filename {
                file_moves.insert(from.to_string(), to.to_string());
            }
        }

        Self::new(file_moves, options)
    }

    /// Creates a scanner from a mapping of old -> new paths
    pub fn new(file_moves: HashMap<String, String>, options: ScanOptions) -> Self {
        // Build the Aho-Corasick automaton for O(n) multi-pattern matching
        let patterns: Vec<String> = file_moves.keys().cloned().collect();
        let automaton =
            AhoCorasick::new(&patterns).expect("Failed to build Aho-Corasick automaton");

        ReferenceScanner {
            options,
            file_moves,
            automaton,
            patterns,
        }
    }

    /// Checks if a directory entry should be excluded from scanning
    /// Used with filter_entry to prune entire subtrees before descending
    fn should_include_entry(
        entry: &walkdir::DirEntry,
        exclude_patterns: &[String],
        verbose: bool,
    ) -> bool {
        let name = match entry.file_name().to_str() {
            Some(n) => n,
            None => return false, // Skip entries with invalid UTF-8 names
        };

        // Skip hidden files/directories
        if name.starts_with('.') {
            if verbose && entry.file_type().is_dir() {
                eprintln!("  [skip] {} (hidden)", entry.path().display());
            }
            return false;
        }

        // Skip excluded patterns
        if exclude_patterns.iter().any(|p| p == name) {
            if verbose && entry.file_type().is_dir() {
                eprintln!(
                    "  [skip] {} (excluded pattern: {})",
                    entry.path().display(),
                    name
                );
            }
            return false;
        }

        true
    }

    /// Checks if a file should be scanned based on extension
    fn should_scan_file(&self, path: &Path) -> bool {
        if self.options.extensions.is_empty() {
            return true;
        }

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_with_dot = format!(".{}", ext);
            self.options.extensions.iter().any(|e| e == &ext_with_dot)
        } else {
            false
        }
    }

    /// Scans a file for references to moved files using Aho-Corasick for O(n) matching
    fn scan_file(&self, path: &Path) -> crate::Result<Vec<ReferenceFix>> {
        let content = fs::read_to_string(path)?;

        if self.patterns.is_empty() {
            return Ok(Vec::new());
        }

        // Build line index for efficient line/column lookup
        let line_starts: Vec<usize> = std::iter::once(0)
            .chain(content.match_indices('\n').map(|(i, _)| i + 1))
            .collect();

        let mut fixes = Vec::new();
        let file_path_str = path.to_string_lossy().to_string();

        // Single pass through the file using Aho-Corasick
        for mat in self.automaton.find_iter(&content) {
            let pattern_idx = mat.pattern().as_usize();
            let old_ref = &self.patterns[pattern_idx];
            let new_ref = match self.file_moves.get(old_ref) {
                Some(r) => r,
                None => continue,
            };

            // Binary search to find line number
            let byte_pos = mat.start();
            let line_idx = line_starts.partition_point(|&start| start <= byte_pos) - 1;
            let line_start = line_starts[line_idx];
            let column = byte_pos - line_start;

            // Extract line content for context
            let line_end = line_starts
                .get(line_idx + 1)
                .map(|&s| s.saturating_sub(1))
                .unwrap_or(content.len());
            let line_content = &content[line_start..line_end];

            fixes.push(ReferenceFix {
                file: file_path_str.clone(),
                line: line_idx + 1,
                column: column + 1,
                context: line_content.trim().to_string(),
                old_reference: old_ref.clone(),
                new_reference: new_ref.clone(),
            });
        }

        Ok(fixes)
    }

    /// Scans directories for broken references
    pub fn scan(&self, directories: &[PathBuf]) -> crate::Result<FixRecord> {
        let mut fix_record = FixRecord::new("changes.json", directories);
        let verbose = self.options.verbose;
        let mut files_scanned = 0;

        for dir in directories {
            if !dir.exists() {
                if verbose {
                    eprintln!("[scan] Directory does not exist: {}", dir.display());
                }
                continue;
            }

            if verbose {
                eprintln!("[scan] Starting scan of: {}", dir.display());
            }

            let walker = if self.options.recursive {
                WalkDir::new(dir)
            } else {
                WalkDir::new(dir).max_depth(1)
            };

            // Use filter_entry to prune excluded directories BEFORE descending into them.
            // This prevents walking into node_modules, .git, target, etc. entirely,
            // rather than entering them and then skipping files one by one.
            let exclude_patterns = &self.options.exclude_patterns;
            let walker = walker
                .into_iter()
                .filter_entry(|e| Self::should_include_entry(e, exclude_patterns, verbose));

            for entry in walker.filter_map(|e| e.ok()) {
                let path = entry.path();

                // Print when entering a new directory
                if verbose && entry.file_type().is_dir() {
                    eprintln!("[scan] Entering directory: {}", path.display());
                    continue;
                }

                if !path.is_file() {
                    continue;
                }

                if !self.should_scan_file(path) {
                    if verbose {
                        eprintln!("  [skip] {} (extension not in scan list)", path.display());
                    }
                    continue;
                }

                if verbose {
                    eprintln!("  [file] {}", path.display());
                }
                files_scanned += 1;

                match self.scan_file(path) {
                    Ok(fixes) => {
                        if verbose && !fixes.is_empty() {
                            eprintln!("    -> Found {} reference(s)", fixes.len());
                        }
                        fix_record.fixes.extend(fixes);
                    }
                    Err(e) => {
                        if verbose {
                            eprintln!("    -> Error: {}", e);
                        }
                        log::debug!("Skipping {}: {}", path.display(), e);
                    }
                }
            }
        }

        if verbose {
            eprintln!(
                "[scan] Complete. Scanned {} files, found {} references.",
                files_scanned,
                fix_record.fixes.len()
            );
        }

        // Deduplicate fixes (same file/line might have multiple matches)
        fix_record
            .fixes
            .sort_by(|a, b| (&a.file, a.line, a.column).cmp(&(&b.file, b.line, b.column)));
        fix_record.fixes.dedup_by(|a, b| {
            a.file == b.file && a.line == b.line && a.old_reference == b.old_reference
        });

        Ok(fix_record)
    }
}

/// Applies fixes from a fix record
pub struct ReferenceFixer;

impl ReferenceFixer {
    /// Applies all fixes from a fix record
    pub fn apply_fixes(fix_record: &FixRecord) -> crate::Result<ApplyResult> {
        let mut result = ApplyResult::default();

        // Group fixes by file
        let mut fixes_by_file: HashMap<&str, Vec<&ReferenceFix>> = HashMap::new();
        for fix in &fix_record.fixes {
            fixes_by_file.entry(&fix.file).or_default().push(fix);
        }

        for (file_path, fixes) in fixes_by_file {
            match Self::apply_fixes_to_file(Path::new(file_path), &fixes) {
                Ok(count) => {
                    result.files_modified += 1;
                    result.references_fixed += count;
                }
                Err(e) => {
                    result.errors.push(format!("{}: {}", file_path, e));
                }
            }
        }

        Ok(result)
    }

    /// Applies fixes to a single file
    fn apply_fixes_to_file(path: &Path, fixes: &[&ReferenceFix]) -> crate::Result<usize> {
        let content = fs::read_to_string(path)?;
        let mut new_content = content.clone();
        let mut fixed_count = 0;

        // Apply fixes (we need to be careful about overlapping replacements)
        for fix in fixes {
            let old = &fix.old_reference;
            let new = &fix.new_reference;

            if new_content.contains(old) {
                new_content = new_content.replace(old, new);
                fixed_count += 1;
            }
        }

        if new_content != content {
            fs::write(path, new_content)?;
        }

        Ok(fixed_count)
    }

    /// Performs a dry run, returning what would be changed
    pub fn dry_run(fix_record: &FixRecord) -> Vec<String> {
        fix_record
            .fixes
            .iter()
            .map(|fix| {
                format!(
                    "{}:{}: '{}' -> '{}'",
                    fix.file, fix.line, fix.old_reference, fix.new_reference
                )
            })
            .collect()
    }
}

/// Result of applying fixes
#[derive(Debug, Default)]
pub struct ApplyResult {
    /// Number of files modified
    pub files_modified: usize,
    /// Number of references fixed
    pub references_fixed: usize,
    /// Errors encountered
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn create_test_dir(name: &str) -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let test_dir = std::env::temp_dir().join(format!(
            "reformat_refs_{}_{}_{}",
            name,
            std::process::id(),
            counter
        ));
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).unwrap();
        test_dir
    }

    #[test]
    fn test_find_reference_quoted() {
        let test_dir = create_test_dir("quoted");

        let mut moves = HashMap::new();
        moves.insert("old.tmpl".to_string(), "new/old.tmpl".to_string());

        let scanner = ReferenceScanner::new(moves, ScanOptions::default());

        // Test with double quotes
        let file1 = test_dir.join("test1.go");
        fs::write(&file1, r#"include "old.tmpl""#).unwrap();
        let fixes = scanner.scan_file(&file1).unwrap();
        assert_eq!(fixes.len(), 1);

        // Test with single quotes
        let file2 = test_dir.join("test2.go");
        fs::write(&file2, r#"include 'old.tmpl'"#).unwrap();
        let fixes = scanner.scan_file(&file2).unwrap();
        assert_eq!(fixes.len(), 1);

        // Test with colon prefix
        let file3 = test_dir.join("test3.yaml");
        fs::write(&file3, "template: old.tmpl").unwrap();
        let fixes = scanner.scan_file(&file3).unwrap();
        assert_eq!(fixes.len(), 1);

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_scan_file() {
        let test_dir = create_test_dir("scan");

        // Create a file with references
        let test_file = test_dir.join("handler.go");
        fs::write(
            &test_file,
            r#"
package main

func render() {
    t := template.ParseFiles("wbs_create.tmpl")
    t2 := template.ParseFiles("wbs_delete.tmpl")
}
"#,
        )
        .unwrap();

        let mut moves = HashMap::new();
        moves.insert("wbs_create.tmpl".to_string(), "wbs/create.tmpl".to_string());
        moves.insert("wbs_delete.tmpl".to_string(), "wbs/delete.tmpl".to_string());

        let scanner = ReferenceScanner::new(moves, ScanOptions::default());
        let fixes = scanner.scan_file(&test_file).unwrap();

        assert_eq!(fixes.len(), 2);
        assert_eq!(fixes[0].old_reference, "wbs_create.tmpl");
        assert_eq!(fixes[0].new_reference, "wbs/create.tmpl");

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_scan_directories() {
        let test_dir = create_test_dir("scandir");

        // Create files with references
        fs::write(
            test_dir.join("main.go"),
            r#"
include "old_file.tmpl"
"#,
        )
        .unwrap();

        fs::write(
            test_dir.join("config.yaml"),
            r#"
template: old_file.tmpl
"#,
        )
        .unwrap();

        let mut moves = HashMap::new();
        moves.insert(
            "old_file.tmpl".to_string(),
            "templates/file.tmpl".to_string(),
        );

        let scanner = ReferenceScanner::new(moves, ScanOptions::default());
        let fix_record = scanner.scan(&[test_dir.clone()]).unwrap();

        assert_eq!(fix_record.len(), 2);

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_apply_fixes() {
        let test_dir = create_test_dir("apply");

        let test_file = test_dir.join("test.go");
        fs::write(&test_file, r#"include "old.tmpl""#).unwrap();

        let fix_record = FixRecord {
            generated_from: "test".to_string(),
            timestamp: "2026-01-15T00:00:00Z".to_string(),
            scan_directories: vec![test_dir.to_string_lossy().to_string()],
            fixes: vec![ReferenceFix {
                file: test_file.to_string_lossy().to_string(),
                line: 1,
                column: 10,
                context: r#"include "old.tmpl""#.to_string(),
                old_reference: "old.tmpl".to_string(),
                new_reference: "new/old.tmpl".to_string(),
            }],
        };

        let result = ReferenceFixer::apply_fixes(&fix_record).unwrap();
        assert_eq!(result.files_modified, 1);
        assert_eq!(result.references_fixed, 1);

        let content = fs::read_to_string(&test_file).unwrap();
        assert!(content.contains("new/old.tmpl"));
        assert!(!content.contains(r#""old.tmpl""#));

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_fix_record_serialization() {
        let fix_record = FixRecord {
            generated_from: "changes.json".to_string(),
            timestamp: "2026-01-15T00:00:00Z".to_string(),
            scan_directories: vec!["/tmp/src".to_string()],
            fixes: vec![ReferenceFix {
                file: "/tmp/src/main.go".to_string(),
                line: 10,
                column: 15,
                context: r#"include "old.tmpl""#.to_string(),
                old_reference: "old.tmpl".to_string(),
                new_reference: "new/old.tmpl".to_string(),
            }],
        };

        let json = serde_json::to_string_pretty(&fix_record).unwrap();
        assert!(json.contains("\"generated_from\": \"changes.json\""));
        assert!(json.contains("\"old_reference\": \"old.tmpl\""));

        let parsed: FixRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.fixes.len(), 1);
    }

    #[test]
    fn test_exclude_patterns() {
        let test_dir = create_test_dir("exclude");

        // Create a directory structure with excluded directories
        let node_modules = test_dir.join("node_modules");
        let git_dir = test_dir.join(".git");
        let src_dir = test_dir.join("src");
        fs::create_dir_all(&node_modules).unwrap();
        fs::create_dir_all(&git_dir).unwrap();
        fs::create_dir_all(&src_dir).unwrap();

        // Create files in each directory that reference "old.tmpl"
        fs::write(node_modules.join("index.js"), "require('old.tmpl')").unwrap();
        fs::write(git_dir.join("config"), "path = old.tmpl").unwrap();
        fs::write(src_dir.join("main.rs"), r#"include!("old.tmpl")"#).unwrap();

        let mut moves = HashMap::new();
        moves.insert("old.tmpl".to_string(), "new/old.tmpl".to_string());

        let scanner = ReferenceScanner::new(moves, ScanOptions::default());
        let fix_record = scanner.scan(&[test_dir.clone()]).unwrap();

        // Only src/main.rs should be scanned - node_modules and .git should be excluded
        assert_eq!(fix_record.len(), 1);
        assert!(fix_record.fixes[0].file.contains("src"));

        let _ = fs::remove_dir_all(&test_dir);
    }
}
