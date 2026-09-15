//! Whitespace cleaning transformer

use std::path::Path;

use crate::step::{ContentStep, FileTarget};

/// Options for whitespace cleaning
#[derive(Debug, Clone)]
pub struct WhitespaceOptions {
    /// Remove trailing whitespace from lines
    pub remove_trailing: bool,
    /// Append a line terminator to a non-empty file that lacks one
    pub insert_final_newline: bool,
    /// Remove whitespace-only lines at the end of the file
    pub trim_trailing_blank_lines: bool,
    /// File extensions to process; empty matches every file
    pub file_extensions: Vec<String>,
    /// Process directories recursively
    pub recursive: bool,
    /// Dry run mode (don't modify files)
    pub dry_run: bool,
}

impl Default for WhitespaceOptions {
    fn default() -> Self {
        WhitespaceOptions {
            remove_trailing: true,
            insert_final_newline: false,
            trim_trailing_blank_lines: false,
            file_extensions: vec![
                ".py", ".pyx", ".pxd", ".pxi", ".c", ".h", ".cpp", ".hpp", ".rs", ".go", ".java",
                ".js", ".ts", ".jsx", ".tsx", ".md", ".qmd", ".txt",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            recursive: true,
            dry_run: false,
        }
    }
}

/// Whitespace cleaner for removing trailing whitespace from files
pub struct WhitespaceCleaner {
    options: WhitespaceOptions,
}

impl WhitespaceCleaner {
    /// Creates a new whitespace cleaner with the given options
    pub fn new(options: WhitespaceOptions) -> Self {
        WhitespaceCleaner { options }
    }

    /// Creates a cleaner with default options
    pub fn with_defaults() -> Self {
        WhitespaceCleaner {
            options: WhitespaceOptions::default(),
        }
    }

    /// Removes trailing whitespace from a single file
    pub fn clean_file(&self, path: &Path) -> crate::Result<usize> {
        if !path.is_file() {
            return Ok(0);
        }
        crate::step::apply_one(self, &FileTarget::file(path), self.options.dry_run)
    }

    /// Processes a directory or file
    pub fn process(&self, path: &Path) -> crate::Result<(usize, usize)> {
        crate::step::process_path(self, path, self.options.recursive, self.options.dry_run)
    }
}

impl ContentStep for WhitespaceCleaner {
    fn name(&self) -> &'static str {
        "clean"
    }

    fn accepts(&self, file: &FileTarget) -> bool {
        crate::step::accepts_by_extension(
            file,
            &self.options.file_extensions,
            self.options.recursive,
        )
    }

    fn transform(&self, text: &str, _file: &FileTarget) -> Option<(String, usize)> {
        let mut changed = 0;

        // Each line keeps its own terminator, so a CRLF file stays CRLF.
        let mut lines: Vec<(&str, &str)> = Vec::new();
        for (body, terminator) in crate::lines::split_lines(text) {
            let cleaned = if self.options.remove_trailing {
                body.trim_end()
            } else {
                body
            };
            if cleaned != body {
                changed += 1;
            }
            lines.push((cleaned, terminator));
        }

        if self.options.trim_trailing_blank_lines {
            while lines.last().is_some_and(|(body, _)| body.trim().is_empty()) {
                lines.pop();
                changed += 1;
            }
        }

        if self.options.insert_final_newline {
            if let Some(last) = lines.last_mut() {
                if last.1.is_empty() {
                    last.1 = crate::lines::first_terminator(text);
                    changed += 1;
                }
            }
        }

        let output: String = lines.iter().flat_map(|(b, t)| [*b, *t]).collect();
        (output != text).then_some((output, changed))
    }

    fn describe(&self, units: usize, dry_run: bool) -> String {
        if dry_run {
            format!("Would clean {} lines in", units)
        } else {
            format!("Cleaned {} lines in", units)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A binary or non-UTF-8 file carrying a processed extension must be
    /// skipped without aborting the walk over its siblings.
    #[test]
    fn test_binary_and_non_utf8_files_are_skipped() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        fs::write(dir.join("a_binary.txt"), [0x00, 0x01, 0x02]).unwrap();
        fs::write(dir.join("b_latin1.txt"), [b'x', b' ', b' ', 0xE9, b'\n']).unwrap();
        fs::write(dir.join("c_ok.txt"), "text  \n").unwrap();

        let cleaner = WhitespaceCleaner::with_defaults();
        let (files, _) = cleaner.process(&dir).unwrap();

        assert_eq!(files, 1, "the walk should continue past unreadable files");
        assert_eq!(fs::read_to_string(dir.join("c_ok.txt")).unwrap(), "text\n");
        assert_eq!(
            fs::read(dir.join("a_binary.txt")).unwrap(),
            [0x00, 0x01, 0x02]
        );
    }
    use std::fs;

    #[test]
    fn test_remove_trailing_whitespace() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.txt");
        fs::write(&test_file, "line1   \nline2\t\nline3\n").unwrap();

        let cleaner = WhitespaceCleaner::with_defaults();
        let (files, lines) = cleaner.process(&test_file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(lines, 2); // line1 and line2 had trailing whitespace

        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "line1\nline2\nline3\n");
    }

    /// Trailing whitespace must be stripped without rewriting the file's line
    /// terminators. Cleaning a CRLF file used to silently convert it to LF,
    /// turning a whitespace tidy-up into a whole-file diff.
    #[test]
    fn test_preserve_line_endings() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        // (input, expected output)
        let cases = [
            ("lf", "line1  \nline2\n", "line1\nline2\n"),
            ("crlf", "line1  \r\nline2\r\n", "line1\r\nline2\r\n"),
            ("cr", "line1  \rline2\r", "line1\rline2\r"),
            (
                "mixed",
                "line1  \r\nline2  \nline3  \r",
                "line1\r\nline2\nline3\r",
            ),
            // Trailing whitespace on the final line, with no terminator at all.
            ("no_final_newline", "line1  \nline2  ", "line1\nline2"),
            // A file that ends with a blank line keeps that blank line.
            ("blank_last", "line1  \n\n", "line1\n\n"),
            // Whitespace-only lines collapse to empty, terminator preserved.
            ("ws_only", "a\r\n   \r\nb\r\n", "a\r\n\r\nb\r\n"),
        ];

        for (name, input, expected) in cases {
            let test_file = test_dir.join(format!("{}.txt", name));
            fs::write(&test_file, input).unwrap();

            let cleaner = WhitespaceCleaner::with_defaults();
            cleaner.process(&test_file).unwrap();

            let content = fs::read_to_string(&test_file).unwrap();
            assert_eq!(
                content, expected,
                "case '{}': line endings were not preserved",
                name
            );
        }
    }

    /// A file with no trailing whitespace must not be rewritten at all --
    /// in particular a CRLF file must not be "normalised" as a side effect.
    #[test]
    fn test_clean_file_untouched_when_nothing_to_strip() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.txt");
        let original = "line1\r\nline2\r\n";
        fs::write(&test_file, original).unwrap();

        let cleaner = WhitespaceCleaner::with_defaults();
        let (files, lines) = cleaner.process(&test_file).unwrap();

        assert_eq!(files, 0);
        assert_eq!(lines, 0);
        assert_eq!(fs::read_to_string(&test_file).unwrap(), original);
    }

    #[test]
    fn test_dry_run_mode() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.txt");
        let original = "line1   \nline2\n";
        fs::write(&test_file, original).unwrap();

        let opts = WhitespaceOptions {
            dry_run: true,

            ..Default::default()
        };

        let cleaner = WhitespaceCleaner::new(opts);
        cleaner.process(&test_file).unwrap();

        // File should be unchanged
        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, original);
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
        fs::write(&hidden_file, "line1   \n").unwrap();

        let cleaner = WhitespaceCleaner::with_defaults();
        let (files, _) = cleaner.process(&hidden_file).unwrap();

        // Hidden file should be skipped
        assert_eq!(files, 0);
    }

    #[test]
    fn test_file_extension_filtering() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let test_dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&test_dir).unwrap();

        let txt_file = test_dir.join("test.txt");
        let other_file = test_dir.join("test.xyz");

        fs::write(&txt_file, "line1   \n").unwrap();
        fs::write(&other_file, "line1   \n").unwrap();

        let opts = WhitespaceOptions {
            file_extensions: vec![".txt".to_string()],

            ..Default::default()
        };

        let cleaner = WhitespaceCleaner::new(opts);
        let (files, _) = cleaner.process(&test_dir).unwrap();

        // Only .txt should be processed
        assert_eq!(files, 1);

        let txt_content = fs::read_to_string(&txt_file).unwrap();
        let other_content = fs::read_to_string(&other_file).unwrap();

        assert_eq!(txt_content, "line1\n");
        assert_eq!(other_content, "line1   \n"); // Unchanged
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

        let file1 = test_dir.join("file1.txt");
        let file2 = sub_dir.join("file2.txt");

        fs::write(&file1, "line1   \n").unwrap();
        fs::write(&file2, "line2\t\n").unwrap();

        let cleaner = WhitespaceCleaner::with_defaults();
        let (files, lines) = cleaner.process(&test_dir).unwrap();

        assert_eq!(files, 2);
        assert_eq!(lines, 2);
    }

    fn clean_text(options: WhitespaceOptions, text: &str) -> String {
        let target = FileTarget::file(Path::new("x.txt"));
        WhitespaceCleaner::new(options)
            .transform(text, &target)
            .map(|(s, _)| s)
            .unwrap_or_else(|| text.to_string())
    }

    #[test]
    fn test_end_of_file_options_are_off_by_default() {
        let o = WhitespaceOptions::default();
        assert_eq!(clean_text(o.clone(), "a\n\n\n"), "a\n\n\n");
        assert_eq!(clean_text(o, "a"), "a");
    }

    #[test]
    fn test_insert_final_newline_uses_the_file_terminator() {
        let o = WhitespaceOptions {
            insert_final_newline: true,
            ..Default::default()
        };
        assert_eq!(clean_text(o.clone(), "a\nb"), "a\nb\n");
        assert_eq!(clean_text(o.clone(), "a\r\nb"), "a\r\nb\r\n");
        assert_eq!(clean_text(o.clone(), "a"), "a\n");
        assert_eq!(clean_text(o.clone(), "a\n"), "a\n");
        assert_eq!(clean_text(o, ""), "", "an empty file stays empty");
    }

    #[test]
    fn test_trim_trailing_blank_lines() {
        let o = WhitespaceOptions {
            trim_trailing_blank_lines: true,
            ..Default::default()
        };
        assert_eq!(clean_text(o.clone(), "a\n\n\n"), "a\n");
        assert_eq!(clean_text(o.clone(), "a\r\n  \r\n\r\n"), "a\r\n");
        assert_eq!(clean_text(o.clone(), "a\n\nb\n"), "a\n\nb\n");
        assert_eq!(clean_text(o, "  \n\n"), "");
    }

    #[test]
    fn test_end_of_file_options_combined() {
        let o = WhitespaceOptions {
            insert_final_newline: true,
            trim_trailing_blank_lines: true,
            ..Default::default()
        };
        assert_eq!(clean_text(o.clone(), "a  \n\n   "), "a\n");
        assert_eq!(clean_text(o, "a\nb  "), "a\nb\n");
    }

    #[test]
    fn test_unprefixed_extension_is_accepted() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.py");
        fs::write(&file, "x  \n").unwrap();
        let cleaner = WhitespaceCleaner::new(WhitespaceOptions {
            file_extensions: vec!["py".to_string()],
            ..Default::default()
        });
        assert_eq!(cleaner.process(tmp.path()).unwrap(), (1, 1));
    }
}
