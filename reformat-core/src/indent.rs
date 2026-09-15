//! Indentation normalization transformer

use std::path::Path;

use crate::step::{ContentStep, FileTarget};

/// Indentation style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    /// Use spaces for indentation
    Spaces,
    /// Use tabs for indentation
    Tabs,
}

impl IndentStyle {
    /// Parse from string representation
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "spaces" | "space" => Some(IndentStyle::Spaces),
            "tabs" | "tab" => Some(IndentStyle::Tabs),
            _ => None,
        }
    }
}

/// Options for indentation normalization
#[derive(Debug, Clone)]
pub struct IndentOptions {
    /// Target indentation style
    pub style: IndentStyle,
    /// Number of spaces per indent level (used when converting tabs to spaces,
    /// or as the tab width when converting spaces to tabs)
    pub width: usize,
    /// File extensions to process; empty matches every file
    pub file_extensions: Vec<String>,
    /// Process directories recursively
    pub recursive: bool,
    /// Dry run mode (don't modify files)
    pub dry_run: bool,
}

impl Default for IndentOptions {
    fn default() -> Self {
        IndentOptions {
            style: IndentStyle::Spaces,
            width: 4,
            file_extensions: vec![
                ".py", ".pyx", ".pxd", ".pxi", ".c", ".h", ".cpp", ".hpp", ".rs", ".go", ".java",
                ".js", ".ts", ".jsx", ".tsx", ".md", ".qmd", ".txt", ".toml", ".yaml", ".yml",
                ".json", ".xml", ".html", ".css",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            recursive: true,
            dry_run: false,
        }
    }
}

/// Indentation normalizer
pub struct IndentNormalizer {
    options: IndentOptions,
}

impl IndentNormalizer {
    /// Creates a new normalizer with the given options
    pub fn new(options: IndentOptions) -> Self {
        IndentNormalizer { options }
    }

    /// Creates a normalizer with default options
    pub fn with_defaults() -> Self {
        IndentNormalizer {
            options: IndentOptions::default(),
        }
    }

    /// Convert leading whitespace on a single line.
    /// Returns the converted line and whether it changed.
    fn convert_line(&self, line: &str) -> (String, bool) {
        // Find the leading whitespace
        let trimmed = line.trim_start_matches([' ', '\t']);
        let leading = &line[..line.len() - trimmed.len()];

        if leading.is_empty() {
            return (line.to_string(), false);
        }

        let width = self.options.width;

        match self.options.style {
            IndentStyle::Spaces => {
                // Convert tabs to spaces
                if !leading.contains('\t') {
                    return (line.to_string(), false);
                }
                let mut spaces = 0usize;
                for ch in leading.chars() {
                    if ch == '\t' {
                        // Align to next tab stop
                        spaces = ((spaces / width) + 1) * width;
                    } else {
                        spaces += 1;
                    }
                }
                let new_leading: String = " ".repeat(spaces);
                (format!("{}{}", new_leading, trimmed), true)
            }
            IndentStyle::Tabs => {
                // Convert spaces to tabs
                if !leading.contains(' ') {
                    return (line.to_string(), false);
                }
                // Count effective column width
                let mut col = 0usize;
                for ch in leading.chars() {
                    if ch == '\t' {
                        col = ((col / width) + 1) * width;
                    } else {
                        col += 1;
                    }
                }
                let tabs = col / width;
                let remaining_spaces = col % width;
                let new_leading = format!("{}{}", "\t".repeat(tabs), " ".repeat(remaining_spaces));
                let changed = new_leading != leading;
                (format!("{}{}", new_leading, trimmed), changed)
            }
        }
    }

    /// Normalize indentation in a single file. Returns the number of lines changed.
    pub fn normalize_file(&self, path: &Path) -> crate::Result<usize> {
        if !path.is_file() {
            return Ok(0);
        }
        crate::step::apply_one(self, &FileTarget::file(path), self.options.dry_run)
    }

    /// Processes a directory or file. Returns (files_changed, lines_changed).
    pub fn process(&self, path: &Path) -> crate::Result<(usize, usize)> {
        crate::step::process_path(self, path, self.options.recursive, self.options.dry_run)
    }
}

impl ContentStep for IndentNormalizer {
    fn name(&self) -> &'static str {
        "indent"
    }

    fn accepts(&self, file: &FileTarget) -> bool {
        crate::step::accepts_by_extension(
            file,
            &self.options.file_extensions,
            self.options.recursive,
        )
    }

    fn transform(&self, text: &str, _file: &FileTarget) -> Option<(String, usize)> {
        let mut output = String::with_capacity(text.len());
        let mut changed_count = 0;

        // Only leading whitespace is rewritten; terminators are kept as found.
        for (body, terminator) in crate::lines::split_lines(text) {
            let (converted, changed) = self.convert_line(body);
            if changed {
                changed_count += 1;
            }
            output.push_str(&converted);
            output.push_str(terminator);
        }

        (changed_count > 0).then_some((output, changed_count))
    }

    fn describe(&self, units: usize, dry_run: bool) -> String {
        if dry_run {
            format!("Would normalize {} line(s) of indentation in", units)
        } else {
            format!("Normalized {} line(s) of indentation in", units)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_tabs_to_spaces() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        fs::write(&file, "\tline1\n\t\tline2\nline3\n").unwrap();

        let normalizer = IndentNormalizer::with_defaults();
        let (files, lines) = normalizer.process(&file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(lines, 2);

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "    line1\n        line2\nline3\n");
    }

    #[test]
    fn test_spaces_to_tabs() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        fs::write(&file, "    line1\n        line2\nline3\n").unwrap();

        let options = IndentOptions {
            style: IndentStyle::Tabs,
            width: 4,
            ..Default::default()
        };
        let normalizer = IndentNormalizer::new(options);
        let (files, lines) = normalizer.process(&file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(lines, 2);

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "\tline1\n\t\tline2\nline3\n");
    }

    #[test]
    fn test_width_2_spaces() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        fs::write(&file, "\tline1\n\t\tline2\n").unwrap();

        let options = IndentOptions {
            style: IndentStyle::Spaces,
            width: 2,
            ..Default::default()
        };
        let normalizer = IndentNormalizer::new(options);
        normalizer.process(&file).unwrap();

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "  line1\n    line2\n");
    }

    #[test]
    fn test_partial_tab_stop_spaces_to_tabs() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        // 6 spaces with width 4: 1 tab + 2 spaces
        fs::write(&file, "      line1\n").unwrap();

        let options = IndentOptions {
            style: IndentStyle::Tabs,
            width: 4,
            ..Default::default()
        };
        let normalizer = IndentNormalizer::new(options);
        normalizer.process(&file).unwrap();

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "\t  line1\n");
    }

    #[test]
    fn test_already_normalized() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        fs::write(&file, "    line1\n        line2\n").unwrap();

        let normalizer = IndentNormalizer::with_defaults();
        let (files, lines) = normalizer.process(&file).unwrap();

        assert_eq!(files, 0);
        assert_eq!(lines, 0);
    }

    #[test]
    fn test_dry_run() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        let original = "\tline1\n";
        fs::write(&file, original).unwrap();

        let options = IndentOptions {
            dry_run: true,
            ..Default::default()
        };
        let normalizer = IndentNormalizer::new(options);
        let (_, lines) = normalizer.process(&file).unwrap();

        assert_eq!(lines, 1);
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn test_preserves_trailing_newline() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        fs::write(&file, "\tline1\n\tline2\n").unwrap();

        let normalizer = IndentNormalizer::with_defaults();
        normalizer.process(&file).unwrap();

        let content = fs::read_to_string(&file).unwrap();
        assert!(content.ends_with('\n'));
        assert_eq!(content, "    line1\n    line2\n");
    }

    #[test]
    fn test_mixed_indent() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        // Tab followed by spaces
        fs::write(&file, "\t  line1\n").unwrap();

        let normalizer = IndentNormalizer::with_defaults();
        normalizer.process(&file).unwrap();

        let content = fs::read_to_string(&file).unwrap();
        // Tab (=4 col) + 2 spaces = 6 spaces
        assert_eq!(content, "      line1\n");
    }

    /// Indentation conversion must not rewrite line terminators.
    #[test]
    fn test_indent_preserves_line_endings() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let cases = [
            (
                "crlf",
                "\tif x:\r\n\t\tpass\r\n",
                "    if x:\r\n        pass\r\n",
            ),
            ("cr", "\tif x:\r\t\tpass\r", "    if x:\r        pass\r"),
            (
                "mixed",
                "\tif x:\r\n\t\tpass\n",
                "    if x:\r\n        pass\n",
            ),
            ("no_final_newline", "\tif x:", "    if x:"),
        ];

        for (name, input, expected) in cases {
            let file = dir.join(format!("{}.py", name));
            fs::write(&file, input).unwrap();

            let normalizer = IndentNormalizer::new(IndentOptions {
                style: IndentStyle::Spaces,
                width: 4,
                ..Default::default()
            });
            normalizer.process(&file).unwrap();

            assert_eq!(
                fs::read_to_string(&file).unwrap(),
                expected,
                "case '{}': line endings were not preserved",
                name
            );
        }
    }

    #[test]
    fn test_parse_indent_style() {
        assert_eq!(IndentStyle::parse("spaces"), Some(IndentStyle::Spaces));
        assert_eq!(IndentStyle::parse("space"), Some(IndentStyle::Spaces));
        assert_eq!(IndentStyle::parse("tabs"), Some(IndentStyle::Tabs));
        assert_eq!(IndentStyle::parse("tab"), Some(IndentStyle::Tabs));
        assert_eq!(IndentStyle::parse("bogus"), None);
    }

    #[test]
    fn test_recursive_processing() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let sub = dir.join("sub");
        fs::create_dir_all(&sub).unwrap();

        let f1 = dir.join("a.py");
        let f2 = sub.join("b.py");
        fs::write(&f1, "\tline1\n").unwrap();
        fs::write(&f2, "\tline2\n").unwrap();

        let normalizer = IndentNormalizer::with_defaults();
        let (files, lines) = normalizer.process(&dir).unwrap();

        assert_eq!(files, 2);
        assert_eq!(lines, 2);
    }
}
