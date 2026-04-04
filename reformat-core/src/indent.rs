//! Indentation normalization transformer

use std::fs;
use std::path::Path;
use walkdir::WalkDir;

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
    /// File extensions to process
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

    /// Checks if a file should be processed
    fn should_process(&self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        if path.components().any(|c| {
            c.as_os_str()
                .to_str()
                .map(|s| s.starts_with('.'))
                .unwrap_or(false)
        }) {
            return false;
        }

        let skip_dirs = [
            "build",
            "__pycache__",
            ".git",
            "node_modules",
            "venv",
            ".venv",
            "target",
        ];
        if path.components().any(|c| {
            c.as_os_str()
                .to_str()
                .map(|s| skip_dirs.contains(&s))
                .unwrap_or(false)
        }) {
            return false;
        }

        if let Some(ext) = path.extension() {
            let ext_str = format!(".{}", ext.to_string_lossy());
            self.options.file_extensions.contains(&ext_str)
        } else {
            false
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
        if !self.should_process(path) {
            return Ok(0);
        }

        let content = fs::read_to_string(path)?;
        let ends_with_newline = content.ends_with('\n');
        let lines: Vec<&str> = content.lines().collect();

        let mut changed_count = 0;
        let mut new_lines: Vec<String> = Vec::with_capacity(lines.len());

        for line in &lines {
            let (converted, changed) = self.convert_line(line);
            if changed {
                changed_count += 1;
            }
            new_lines.push(converted);
        }

        if changed_count > 0 {
            if self.options.dry_run {
                println!(
                    "Would normalize {} line(s) of indentation in '{}'",
                    changed_count,
                    path.display()
                );
            } else {
                let mut output = new_lines.join("\n");
                if ends_with_newline {
                    output.push('\n');
                }
                fs::write(path, output)?;
                println!(
                    "Normalized {} line(s) of indentation in '{}'",
                    changed_count,
                    path.display()
                );
            }
        }

        Ok(changed_count)
    }

    /// Processes a directory or file. Returns (files_changed, lines_changed).
    pub fn process(&self, path: &Path) -> crate::Result<(usize, usize)> {
        let mut total_files = 0;
        let mut total_lines = 0;

        if path.is_file() {
            let lines = self.normalize_file(path)?;
            if lines > 0 {
                total_files = 1;
                total_lines = lines;
            }
        } else if path.is_dir() {
            if self.options.recursive {
                for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        let lines = self.normalize_file(entry.path())?;
                        if lines > 0 {
                            total_files += 1;
                            total_lines += lines;
                        }
                    }
                }
            } else {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        let lines = self.normalize_file(&entry_path)?;
                        if lines > 0 {
                            total_files += 1;
                            total_lines += lines;
                        }
                    }
                }
            }
        }

        Ok((total_files, total_lines))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_tabs_to_spaces() {
        let dir = std::env::temp_dir().join("reformat_indent_t2s");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        fs::write(&file, "\tline1\n\t\tline2\nline3\n").unwrap();

        let normalizer = IndentNormalizer::with_defaults();
        let (files, lines) = normalizer.process(&file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(lines, 2);

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "    line1\n        line2\nline3\n");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_spaces_to_tabs() {
        let dir = std::env::temp_dir().join("reformat_indent_s2t");
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

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_width_2_spaces() {
        let dir = std::env::temp_dir().join("reformat_indent_w2");
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

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_partial_tab_stop_spaces_to_tabs() {
        let dir = std::env::temp_dir().join("reformat_indent_partial");
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

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_already_normalized() {
        let dir = std::env::temp_dir().join("reformat_indent_noop");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        fs::write(&file, "    line1\n        line2\n").unwrap();

        let normalizer = IndentNormalizer::with_defaults();
        let (files, lines) = normalizer.process(&file).unwrap();

        assert_eq!(files, 0);
        assert_eq!(lines, 0);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_dry_run() {
        let dir = std::env::temp_dir().join("reformat_indent_dry");
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

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_preserves_trailing_newline() {
        let dir = std::env::temp_dir().join("reformat_indent_newline");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        fs::write(&file, "\tline1\n\tline2\n").unwrap();

        let normalizer = IndentNormalizer::with_defaults();
        normalizer.process(&file).unwrap();

        let content = fs::read_to_string(&file).unwrap();
        assert!(content.ends_with('\n'));
        assert_eq!(content, "    line1\n    line2\n");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_mixed_indent() {
        let dir = std::env::temp_dir().join("reformat_indent_mixed");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        // Tab followed by spaces
        fs::write(&file, "\t  line1\n").unwrap();

        let normalizer = IndentNormalizer::with_defaults();
        normalizer.process(&file).unwrap();

        let content = fs::read_to_string(&file).unwrap();
        // Tab (=4 col) + 2 spaces = 6 spaces
        assert_eq!(content, "      line1\n");

        fs::remove_dir_all(&dir).unwrap();
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
        let dir = std::env::temp_dir().join("reformat_indent_recursive");
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

        fs::remove_dir_all(&dir).unwrap();
    }
}
