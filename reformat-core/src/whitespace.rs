//! Whitespace cleaning transformer

use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Options for whitespace cleaning
#[derive(Debug, Clone)]
pub struct WhitespaceOptions {
    /// Remove trailing whitespace from lines
    pub remove_trailing: bool,
    /// File extensions to process
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

    /// Checks if a file should be processed
    fn should_process(&self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        // Skip hidden files and directories
        if path.components().any(|c| {
            c.as_os_str()
                .to_str()
                .map(|s| s.starts_with('.'))
                .unwrap_or(false)
        }) {
            return false;
        }

        // Skip build directories
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

        // Check file extension
        if let Some(ext) = path.extension() {
            let ext_str = format!(".{}", ext.to_string_lossy());
            self.options.file_extensions.contains(&ext_str)
        } else {
            false
        }
    }

    /// Removes trailing whitespace from a single file
    pub fn clean_file(&self, path: &Path) -> crate::Result<usize> {
        if !self.should_process(path) {
            return Ok(0);
        }

        let content = fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();
        let mut cleaned_lines = Vec::new();
        let mut modified_count = 0;

        for line in &lines {
            if self.options.remove_trailing {
                let cleaned = line.trim_end();
                if cleaned != *line {
                    modified_count += 1;
                }
                cleaned_lines.push(cleaned);
            } else {
                cleaned_lines.push(*line);
            }
        }

        // Check if file ends with newline
        let ends_with_newline = content.ends_with('\n');

        if modified_count > 0 {
            if self.options.dry_run {
                println!(
                    "Would clean {} lines in '{}'",
                    modified_count,
                    path.display()
                );
            } else {
                let mut cleaned_content = cleaned_lines.join("\n");
                if ends_with_newline {
                    cleaned_content.push('\n');
                }
                fs::write(path, cleaned_content)?;
                println!("Cleaned {} lines in '{}'", modified_count, path.display());
            }
        }

        Ok(modified_count)
    }

    /// Processes a directory or file
    pub fn process(&self, path: &Path) -> crate::Result<(usize, usize)> {
        let mut total_files = 0;
        let mut total_lines = 0;

        if path.is_file() {
            let lines = self.clean_file(path)?;
            if lines > 0 {
                total_files = 1;
                total_lines = lines;
            }
        } else if path.is_dir() {
            if self.options.recursive {
                for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        let lines = self.clean_file(entry.path())?;
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
                        let lines = self.clean_file(&entry_path)?;
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
    fn test_remove_trailing_whitespace() {
        let test_dir = std::env::temp_dir().join("reformat_whitespace_test");
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.txt");
        fs::write(&test_file, "line1   \nline2\t\nline3\n").unwrap();

        let cleaner = WhitespaceCleaner::with_defaults();
        let (files, lines) = cleaner.process(&test_file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(lines, 2); // line1 and line2 had trailing whitespace

        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "line1\nline2\nline3\n");

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn test_preserve_line_endings() {
        let test_dir = std::env::temp_dir().join("reformat_whitespace_endings");
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.txt");
        fs::write(&test_file, "line1  \nline2\n").unwrap();

        let cleaner = WhitespaceCleaner::with_defaults();
        cleaner.process(&test_file).unwrap();

        let content = fs::read_to_string(&test_file).unwrap();
        assert!(content.ends_with('\n'));
        assert_eq!(content, "line1\nline2\n");

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn test_dry_run_mode() {
        let test_dir = std::env::temp_dir().join("reformat_whitespace_dry");
        fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.txt");
        let original = "line1   \nline2\n";
        fs::write(&test_file, original).unwrap();

        let mut opts = WhitespaceOptions::default();
        opts.dry_run = true;

        let cleaner = WhitespaceCleaner::new(opts);
        cleaner.process(&test_file).unwrap();

        // File should be unchanged
        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, original);

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn test_skip_hidden_files() {
        let test_dir = std::env::temp_dir().join("reformat_whitespace_hidden");
        fs::create_dir_all(&test_dir).unwrap();

        let hidden_file = test_dir.join(".hidden.txt");
        fs::write(&hidden_file, "line1   \n").unwrap();

        let cleaner = WhitespaceCleaner::with_defaults();
        let (files, _) = cleaner.process(&hidden_file).unwrap();

        // Hidden file should be skipped
        assert_eq!(files, 0);

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn test_file_extension_filtering() {
        let test_dir = std::env::temp_dir().join("reformat_whitespace_ext");
        fs::create_dir_all(&test_dir).unwrap();

        let txt_file = test_dir.join("test.txt");
        let other_file = test_dir.join("test.xyz");

        fs::write(&txt_file, "line1   \n").unwrap();
        fs::write(&other_file, "line1   \n").unwrap();

        let mut opts = WhitespaceOptions::default();
        opts.file_extensions = vec![".txt".to_string()];

        let cleaner = WhitespaceCleaner::new(opts);
        let (files, _) = cleaner.process(&test_dir).unwrap();

        // Only .txt should be processed
        assert_eq!(files, 1);

        let txt_content = fs::read_to_string(&txt_file).unwrap();
        let other_content = fs::read_to_string(&other_file).unwrap();

        assert_eq!(txt_content, "line1\n");
        assert_eq!(other_content, "line1   \n"); // Unchanged

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn test_recursive_processing() {
        let test_dir = std::env::temp_dir().join("reformat_whitespace_recursive");
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

        fs::remove_dir_all(&test_dir).unwrap();
    }
}
