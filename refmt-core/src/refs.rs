//! Reference scanning and fixing for broken file references
//!
//! This module provides functionality to scan codebases for references to
//! moved/renamed files and generate fixes for those references.

use crate::changes::ChangeRecord;
use regex::Regex;
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
        }
    }
}

/// Reference scanner for finding broken references after file moves
pub struct ReferenceScanner {
    options: ScanOptions,
    /// Map of old filename -> new path
    file_moves: HashMap<String, String>,
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
        
        ReferenceScanner { options, file_moves }
    }

    /// Creates a scanner from a mapping of old -> new paths
    pub fn new(file_moves: HashMap<String, String>, options: ScanOptions) -> Self {
        ReferenceScanner { options, file_moves }
    }

    /// Checks if a path should be excluded from scanning
    fn should_exclude(&self, path: &Path) -> bool {
        for component in path.components() {
            if let Some(name) = component.as_os_str().to_str() {
                for pattern in &self.options.exclude_patterns {
                    if name == pattern || name.starts_with('.') {
                        return true;
                    }
                }
            }
        }
        false
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

    /// Scans a file for references to moved files
    fn scan_file(&self, path: &Path) -> crate::Result<Vec<ReferenceFix>> {
        let mut fixes = Vec::new();
        let content = fs::read_to_string(path)?;
        
        for (line_num, line) in content.lines().enumerate() {
            for (old_ref, new_ref) in &self.file_moves {
                // Look for the old reference in various contexts
                // This handles: "filename", 'filename', `filename`, /path/filename, etc.
                if let Some(col) = self.find_reference(line, old_ref) {
                    fixes.push(ReferenceFix {
                        file: path.to_string_lossy().to_string(),
                        line: line_num + 1,
                        column: col + 1,
                        context: line.trim().to_string(),
                        old_reference: old_ref.clone(),
                        new_reference: new_ref.clone(),
                    });
                }
            }
        }
        
        Ok(fixes)
    }

    /// Finds a reference in a line, returning the column if found
    fn find_reference(&self, line: &str, reference: &str) -> Option<usize> {
        // Escape special regex characters in the reference
        let escaped = regex::escape(reference);
        
        // Build pattern to match the reference in various contexts
        // This matches: quoted strings, paths, template includes, etc.
        let pattern = format!(
            r#"(?:["'`]|/|\\|^|\s)({})(?:["'`]|/|\\|$|\s|[,;:\)>\]])"#,
            escaped
        );
        
        if let Ok(re) = Regex::new(&pattern) {
            if let Some(m) = re.find(line) {
                // Find the actual start of the reference within the match
                if let Some(pos) = line[m.start()..].find(reference) {
                    return Some(m.start() + pos);
                }
            }
        }
        
        // Fallback: simple contains check for cases the regex misses
        if line.contains(reference) {
            return line.find(reference);
        }
        
        None
    }

    /// Scans directories for broken references
    pub fn scan(&self, directories: &[PathBuf]) -> crate::Result<FixRecord> {
        let mut fix_record = FixRecord::new("changes.json", directories);
        
        for dir in directories {
            if !dir.exists() {
                continue;
            }
            
            let walker = if self.options.recursive {
                WalkDir::new(dir).into_iter()
            } else {
                WalkDir::new(dir).max_depth(1).into_iter()
            };
            
            for entry in walker.filter_map(|e| e.ok()) {
                let path = entry.path();
                
                if self.should_exclude(path) {
                    continue;
                }
                
                if !path.is_file() || !self.should_scan_file(path) {
                    continue;
                }
                
                match self.scan_file(path) {
                    Ok(fixes) => fix_record.fixes.extend(fixes),
                    Err(e) => {
                        // Skip files we can't read (binary, permissions, etc.)
                        log::debug!("Skipping {}: {}", path.display(), e);
                    }
                }
            }
        }
        
        // Deduplicate fixes (same file/line might have multiple matches)
        fix_record.fixes.sort_by(|a, b| {
            (&a.file, a.line, a.column).cmp(&(&b.file, b.line, b.column))
        });
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
            "refmt_refs_{}_{}_{}",
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
        let mut moves = HashMap::new();
        moves.insert("old.tmpl".to_string(), "new/old.tmpl".to_string());
        
        let scanner = ReferenceScanner::new(moves, ScanOptions::default());
        
        assert!(scanner.find_reference(r#"include "old.tmpl""#, "old.tmpl").is_some());
        assert!(scanner.find_reference(r#"include 'old.tmpl'"#, "old.tmpl").is_some());
        assert!(scanner.find_reference(r#"template: old.tmpl"#, "old.tmpl").is_some());
    }

    #[test]
    fn test_scan_file() {
        let test_dir = create_test_dir("scan");
        
        // Create a file with references
        let test_file = test_dir.join("handler.go");
        fs::write(&test_file, r#"
package main

func render() {
    t := template.ParseFiles("wbs_create.tmpl")
    t2 := template.ParseFiles("wbs_delete.tmpl")
}
"#).unwrap();
        
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
        fs::write(test_dir.join("main.go"), r#"
include "old_file.tmpl"
"#).unwrap();
        
        fs::write(test_dir.join("config.yaml"), r#"
template: old_file.tmpl
"#).unwrap();
        
        let mut moves = HashMap::new();
        moves.insert("old_file.tmpl".to_string(), "templates/file.tmpl".to_string());
        
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
        let scanner = ReferenceScanner::new(HashMap::new(), ScanOptions::default());
        
        assert!(scanner.should_exclude(Path::new("/project/.git/config")));
        assert!(scanner.should_exclude(Path::new("/project/node_modules/pkg/index.js")));
        assert!(scanner.should_exclude(Path::new("/project/target/debug/main")));
        assert!(!scanner.should_exclude(Path::new("/project/src/main.rs")));
    }
}
