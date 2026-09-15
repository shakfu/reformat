//! Reference scanning and fixing for broken file references
//!
//! This module provides functionality to scan codebases for references to
//! moved/renamed files and generate fixes for those references.

use crate::changes::{to_slash, ChangeRecord};
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
    /// Byte offset of the reference within the file. This is the authoritative
    /// position used when applying the fix; `line` and `column` are derived
    /// from it for display. Optional so that records written by earlier
    /// versions still load -- those fall back to line/column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
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
        let mut record: FixRecord = serde_json::from_str(&json)?;
        // Windows builds up to 0.2.0 wrote `\` separators into new_reference.
        for fix in &mut record.fixes {
            fix.new_reference = to_slash(&fix.new_reference);
        }
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

/// Byte offsets at which each line of `content` starts.
fn line_starts(content: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(content.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

/// Characters that can form part of a filename. A match flanked by one of
/// these is part of a longer name, not a reference to the moved file.
///
/// `/` is deliberately absent: a match preceded by a path separator (as in
/// `tmpl/user_list.tmpl`) is a genuine reference, and rewriting just the
/// trailing filename yields the correct new path.
fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '.'
}

/// True if `content[start..end]` stands alone as a path reference rather than
/// being embedded in a longer filename.
fn is_standalone_reference(content: &str, start: usize, end: usize) -> bool {
    let before_ok = content[..start]
        .chars()
        .next_back()
        .is_none_or(|c| !is_name_char(c));
    let after_ok = content[end..]
        .chars()
        .next()
        .is_none_or(|c| !is_name_char(c));
    before_ok && after_ok
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
    pub fn from_change_record(record: &ChangeRecord, options: ScanOptions) -> crate::Result<Self> {
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

    /// Creates a scanner from a mapping of old -> new paths.
    ///
    /// Returns an error rather than panicking if the automaton cannot be
    /// built: this is a public library constructor, and a pathological set of
    /// move patterns should surface as an error to the caller.
    pub fn new(file_moves: HashMap<String, String>, options: ScanOptions) -> crate::Result<Self> {
        // Sorted so that scan output and fix ordering are reproducible; a
        // HashMap's iteration order varies between runs.
        let mut patterns: Vec<String> = file_moves.keys().cloned().collect();
        patterns.sort();

        // Aho-Corasick gives O(n) multi-pattern matching over each file.
        let automaton = AhoCorasick::new(&patterns)
            .map_err(|e| anyhow::anyhow!("failed to build reference matcher: {}", e))?;

        Ok(ReferenceScanner {
            options,
            file_moves,
            automaton,
            patterns,
        })
    }

    /// Checks if a directory entry should be excluded from scanning.
    /// Used with filter_entry to prune entire subtrees before descending.
    ///
    /// Delegates to `crate::walk::include_entry`, which always accepts the
    /// walk root: pruning it would make `--scope .` scan nothing at all,
    /// since the root's file name is literally `.`.
    fn should_include_entry(
        entry: &walkdir::DirEntry,
        exclude_patterns: &[String],
        verbose: bool,
    ) -> bool {
        let included = crate::walk::include_entry(entry, exclude_patterns);
        if !included && verbose && entry.file_type().is_dir() {
            log::info!("  [skip] {}", entry.path().display());
        }
        included
    }

    /// Checks if a file should be scanned based on extension
    fn should_scan_file(&self, path: &Path) -> bool {
        crate::step::matches_extension(path, &self.options.extensions)
    }

    /// Scans a file for references to moved files using Aho-Corasick for O(n) matching
    fn scan_file(&self, path: &Path) -> crate::Result<Vec<ReferenceFix>> {
        let content = fs::read_to_string(path)?;

        if self.patterns.is_empty() {
            return Ok(Vec::new());
        }

        // Build line index for efficient line/column lookup
        let line_starts = line_starts(&content);

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

            // Aho-Corasick matches bare substrings, so `user_list.tmpl` also
            // matches inside `super_user_list.tmpl` and `user_list.tmpl.bak`,
            // which are different files entirely. Require the match to stand
            // alone as a path reference.
            if !is_standalone_reference(&content, mat.start(), mat.end()) {
                continue;
            }

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
                offset: Some(byte_pos),
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
                    log::info!("[scan] Directory does not exist: {}", dir.display());
                }
                continue;
            }

            if verbose {
                log::info!("[scan] Starting scan of: {}", dir.display());
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
                    log::info!("[scan] Entering directory: {}", path.display());
                    continue;
                }

                if !path.is_file() {
                    continue;
                }

                if !self.should_scan_file(path) {
                    if verbose {
                        log::info!("  [skip] {} (extension not in scan list)", path.display());
                    }
                    continue;
                }

                if verbose {
                    log::info!("  [file] {}", path.display());
                }
                files_scanned += 1;

                match self.scan_file(path) {
                    Ok(fixes) => {
                        if verbose && !fixes.is_empty() {
                            log::info!("    -> Found {} reference(s)", fixes.len());
                        }
                        fix_record.fixes.extend(fixes);
                    }
                    Err(e) => {
                        if verbose {
                            log::info!("    -> Error: {}", e);
                        }
                        log::debug!("Skipping {}: {}", path.display(), e);
                    }
                }
            }
        }

        if verbose {
            log::info!(
                "[scan] Complete. Scanned {} files, found {} references.",
                files_scanned,
                fix_record.fixes.len()
            );
        }

        // Sort for stable output, then drop only exact duplicates. Two
        // references on the same line at different columns are distinct
        // occurrences that each need their own edit -- deduplicating on
        // (file, line, old_reference) alone silently dropped all but one.
        fix_record
            .fixes
            .sort_by(|a, b| (&a.file, a.line, a.column).cmp(&(&b.file, b.line, b.column)));
        fix_record.fixes.dedup_by(|a, b| {
            a.file == b.file
                && a.line == b.line
                && a.column == b.column
                && a.old_reference == b.old_reference
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

        // Deterministic order so output and error reporting are reproducible.
        let mut by_file: Vec<(&str, Vec<&ReferenceFix>)> = fixes_by_file.into_iter().collect();
        by_file.sort_by(|a, b| a.0.cmp(b.0));

        for (file_path, fixes) in by_file {
            match Self::apply_fixes_to_file(Path::new(file_path), &fixes) {
                Ok(outcome) => {
                    if outcome.modified {
                        result.files_modified += 1;
                    }
                    result.references_fixed += outcome.replaced;
                    result.references_skipped += outcome.skipped;
                }
                Err(e) => {
                    result.errors.push(format!("{}: {}", file_path, e));
                }
            }
        }

        Ok(result)
    }

    /// Resolves a recorded fix to a byte offset in the current file contents,
    /// returning `None` if the text there is not what was recorded.
    ///
    /// This verification is what makes applying a record safe: a stale record,
    /// a file edited since the scan, or a record that has already been applied
    /// all fail the check and are skipped rather than corrupting the file.
    fn resolve_span(content: &str, line_starts: &[usize], fix: &ReferenceFix) -> Option<usize> {
        let start = match fix.offset {
            Some(offset) => offset,
            None => {
                // Records written before offsets were stored: derive the
                // position from the 1-indexed line and column.
                let line_start = *line_starts.get(fix.line.checked_sub(1)?)?;
                line_start + fix.column.checked_sub(1)?
            }
        };
        let end = start.checked_add(fix.old_reference.len())?;

        if end <= content.len()
            && content.is_char_boundary(start)
            && content.is_char_boundary(end)
            && content[start..end] == fix.old_reference
        {
            Some(start)
        } else {
            None
        }
    }

    /// Applies fixes to a single file by byte offset.
    ///
    /// Each recorded occurrence is edited in place. A global
    /// `String::replace` was used here previously, which rewrote every
    /// occurrence of the old filename anywhere in the file -- including inside
    /// longer, unrelated filenames -- and compounded on a second run, since a
    /// move such as `a.txt` -> `x/a.txt` re-introduces `a.txt` as a substring.
    fn apply_fixes_to_file(path: &Path, fixes: &[&ReferenceFix]) -> crate::Result<FileOutcome> {
        let content = fs::read_to_string(path)?;
        let line_starts = line_starts(&content);

        let mut spans: Vec<(usize, usize, &str)> = Vec::with_capacity(fixes.len());
        let mut skipped = 0;

        for fix in fixes {
            match Self::resolve_span(&content, &line_starts, fix) {
                Some(start) => spans.push((
                    start,
                    start + fix.old_reference.len(),
                    fix.new_reference.as_str(),
                )),
                None => skipped += 1,
            }
        }

        // Edit from the end of the file backwards so that earlier offsets stay
        // valid as the content shifts.
        spans.sort_by_key(|&(start, _, _)| std::cmp::Reverse(start));

        let mut new_content = content.clone();
        let mut replaced = 0;
        let mut next_start = content.len();

        for (start, end, replacement) in spans {
            if end > next_start {
                // Overlaps an edit already applied; leave it alone.
                skipped += 1;
                continue;
            }
            new_content.replace_range(start..end, replacement);
            next_start = start;
            replaced += 1;
        }

        let modified = new_content != content;
        if modified {
            crate::step::write_atomic(path, new_content.as_bytes())?;
        }

        Ok(FileOutcome {
            replaced,
            skipped,
            modified,
        })
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

/// Per-file outcome of applying fixes
struct FileOutcome {
    /// Occurrences actually rewritten
    replaced: usize,
    /// Recorded fixes that no longer matched and were skipped
    skipped: usize,
    /// Whether the file was written
    modified: bool,
}

/// Result of applying fixes
#[derive(Debug, Default)]
pub struct ApplyResult {
    /// Number of files modified
    pub files_modified: usize,
    /// Number of references fixed
    pub references_fixed: usize,
    /// Number of recorded fixes that no longer matched the file and were
    /// skipped (a stale record, or one that has already been applied)
    pub references_skipped: usize,
    /// Errors encountered
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a scanner for a single move.
    fn scanner_for(old: &str, new: &str) -> ReferenceScanner {
        let mut moves = HashMap::new();
        moves.insert(old.to_string(), new.to_string());
        ReferenceScanner::new(moves, ScanOptions::default()).unwrap()
    }

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        p
    }

    /// A filename that merely *contains* a moved filename is a different file
    /// and must not be reported or rewritten.
    #[test]
    fn test_substring_matches_are_not_reported() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        write(
            &dir,
            "app.go",
            concat!(
                "render(\"user_list.tmpl\")\n",
                "// see also super_user_list.tmpl elsewhere\n",
                "load(\"user_list.tmpl.bak\")\n",
                "use(\"tmpl/user_list.tmpl\")\n",
            ),
        );

        let scanner = scanner_for("user_list.tmpl", "user/list.tmpl");
        let record = scanner.scan(std::slice::from_ref(&dir)).unwrap();

        // Only the standalone reference and the path-qualified one qualify.
        assert_eq!(
            record.len(),
            2,
            "expected 2 real references, got: {:#?}",
            record.fixes
        );
        for fix in &record.fixes {
            assert!(
                !fix.context.contains("super_user_list"),
                "matched inside a longer filename: {}",
                fix.context
            );
            assert!(
                !fix.context.contains(".tmpl.bak"),
                "matched a different file with a longer extension: {}",
                fix.context
            );
        }
    }

    /// Applying fixes must rewrite only the recorded occurrences, leaving
    /// near-miss text in the same file untouched.
    #[test]
    fn test_apply_only_touches_recorded_occurrences() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = write(
            &dir,
            "app.go",
            concat!(
                "render(\"user_list.tmpl\")\n",
                "// see also super_user_list.tmpl elsewhere\n",
            ),
        );

        let scanner = scanner_for("user_list.tmpl", "user/list.tmpl");
        let record = scanner.scan(std::slice::from_ref(&dir)).unwrap();
        let applied = ReferenceFixer::apply_fixes(&record).unwrap();

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(
            content,
            concat!(
                "render(\"user/list.tmpl\")\n",
                "// see also super_user_list.tmpl elsewhere\n",
            ),
            "the unrelated super_user_list.tmpl reference was rewritten"
        );
        assert_eq!(applied.files_modified, 1);
        assert_eq!(applied.references_fixed, 1);
    }

    /// Every occurrence on a line must be fixed, not just the first.
    #[test]
    fn test_multiple_occurrences_on_one_line_are_all_fixed() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = write(
            &dir,
            "app.go",
            "a(\"x.tmpl\"); b(\"x.tmpl\"); c(\"x.tmpl\")\n",
        );

        let scanner = scanner_for("x.tmpl", "g/x.tmpl");
        let record = scanner.scan(std::slice::from_ref(&dir)).unwrap();
        assert_eq!(record.len(), 3, "all three occurrences should be recorded");

        let applied = ReferenceFixer::apply_fixes(&record).unwrap();
        assert_eq!(
            applied.references_fixed, 3,
            "count must be occurrences, not fixes attempted"
        );
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "a(\"g/x.tmpl\"); b(\"g/x.tmpl\"); c(\"g/x.tmpl\")\n"
        );
    }

    /// Applying the same record twice must not compound the replacement.
    /// `a.txt` -> `x/a.txt` re-introduces `a.txt` as a substring; a global
    /// replace would turn the second run into `x/x/a.txt`.
    #[test]
    fn test_apply_is_idempotent() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = write(&dir, "app.go", "open(\"a.txt\")\n");

        let scanner = scanner_for("a.txt", "x/a.txt");
        let record = scanner.scan(std::slice::from_ref(&dir)).unwrap();

        let first = ReferenceFixer::apply_fixes(&record).unwrap();
        assert_eq!(first.references_fixed, 1);
        assert_eq!(fs::read_to_string(&file).unwrap(), "open(\"x/a.txt\")\n");

        // Re-applying the same (now stale) record must change nothing.
        let second = ReferenceFixer::apply_fixes(&record).unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "open(\"x/a.txt\")\n",
            "re-applying compounded the replacement"
        );
        assert_eq!(second.references_fixed, 0);
        assert_eq!(second.files_modified, 0, "no file was actually modified");
        assert_eq!(
            second.references_skipped, 1,
            "the stale fix should be reported as skipped"
        );
    }

    /// A file whose fixes all fail to apply must not be counted as modified.
    #[test]
    fn test_unmodified_files_are_not_counted() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = write(&dir, "app.go", "open(\"a.txt\")\n");
        let scanner = scanner_for("a.txt", "x/a.txt");
        let record = scanner.scan(std::slice::from_ref(&dir)).unwrap();

        // Edit the file out from under the record.
        fs::write(&file, "open(\"something_else.txt\")\n").unwrap();

        let applied = ReferenceFixer::apply_fixes(&record).unwrap();
        assert_eq!(
            applied.files_modified, 0,
            "a file that was not written must not be counted"
        );
        assert_eq!(applied.references_fixed, 0);
        assert_eq!(applied.references_skipped, 1);
    }

    /// Returns an owned temporary directory. The caller must keep the
    /// `TempDir` alive: dropping it removes the directory.
    fn create_test_dir(name: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(&format!("reformat-{}-", name))
            .tempdir()
            .unwrap()
    }

    #[test]
    fn test_find_reference_quoted() {
        let _tmp = create_test_dir("quoted");
        let test_dir = _tmp.path().to_path_buf();

        let mut moves = HashMap::new();
        moves.insert("old.tmpl".to_string(), "new/old.tmpl".to_string());

        let scanner = ReferenceScanner::new(moves, ScanOptions::default()).unwrap();

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
    }

    #[test]
    fn test_scan_file() {
        let _tmp = create_test_dir("scan");
        let test_dir = _tmp.path().to_path_buf();

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

        let scanner = ReferenceScanner::new(moves, ScanOptions::default()).unwrap();
        let fixes = scanner.scan_file(&test_file).unwrap();

        assert_eq!(fixes.len(), 2);
        assert_eq!(fixes[0].old_reference, "wbs_create.tmpl");
        assert_eq!(fixes[0].new_reference, "wbs/create.tmpl");
    }

    #[test]
    fn test_scan_directories() {
        let _tmp = create_test_dir("scandir");
        let test_dir = _tmp.path().to_path_buf();

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

        let scanner = ReferenceScanner::new(moves, ScanOptions::default()).unwrap();
        let fix_record = scanner.scan(std::slice::from_ref(&test_dir)).unwrap();

        assert_eq!(fix_record.len(), 2);
    }

    #[test]
    fn test_apply_fixes() {
        let _tmp = create_test_dir("apply");
        let test_dir = _tmp.path().to_path_buf();

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
                offset: None,
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
                offset: None,
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
    fn test_read_normalizes_windows_separators() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("fixes.json");
        let mut record = FixRecord::new("changes.json", &[]);
        record.fixes.push(ReferenceFix {
            file: "main.go".to_string(),
            line: 1,
            column: 7,
            offset: Some(6),
            context: r#"load("wbs_a.tmpl")"#.to_string(),
            old_reference: "wbs_a.tmpl".to_string(),
            new_reference: "wbs\\a.tmpl".to_string(),
        });
        record.write_to_file(&path).unwrap();

        let loaded = FixRecord::read_from_file(&path).unwrap();
        let expected = if cfg!(windows) {
            "wbs/a.tmpl"
        } else {
            "wbs\\a.tmpl"
        };
        assert_eq!(loaded.fixes[0].new_reference, expected);
    }

    #[test]
    fn test_exclude_patterns() {
        let _tmp = create_test_dir("exclude");
        let test_dir = _tmp.path().to_path_buf();

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

        let scanner = ReferenceScanner::new(moves, ScanOptions::default()).unwrap();
        let fix_record = scanner.scan(std::slice::from_ref(&test_dir)).unwrap();

        // Only src/main.rs should be scanned - node_modules and .git should be excluded
        assert_eq!(fix_record.len(), 1);
        assert!(fix_record.fixes[0].file.contains("src"));
    }
}
