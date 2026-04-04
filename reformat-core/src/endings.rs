//! Line ending normalization transformer

use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Line ending style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix-style: \n
    Lf,
    /// Windows-style: \r\n
    Crlf,
    /// Classic Mac-style: \r
    Cr,
}

impl LineEnding {
    /// Parse from string representation
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "lf" | "LF" | "unix" => Some(LineEnding::Lf),
            "crlf" | "CRLF" | "windows" => Some(LineEnding::Crlf),
            "cr" | "CR" | "mac" => Some(LineEnding::Cr),
            _ => None,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        match self {
            LineEnding::Lf => b"\n",
            LineEnding::Crlf => b"\r\n",
            LineEnding::Cr => b"\r",
        }
    }
}

/// Options for line ending normalization
#[derive(Debug, Clone)]
pub struct EndingsOptions {
    /// Target line ending style
    pub style: LineEnding,
    /// File extensions to process
    pub file_extensions: Vec<String>,
    /// Process directories recursively
    pub recursive: bool,
    /// Dry run mode (don't modify files)
    pub dry_run: bool,
}

impl Default for EndingsOptions {
    fn default() -> Self {
        EndingsOptions {
            style: LineEnding::Lf,
            file_extensions: vec![
                ".py", ".pyx", ".pxd", ".pxi", ".c", ".h", ".cpp", ".hpp", ".rs", ".go", ".java",
                ".js", ".ts", ".jsx", ".tsx", ".md", ".qmd", ".txt", ".toml", ".yaml", ".yml",
                ".json", ".xml", ".html", ".css", ".sh", ".bat",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            recursive: true,
            dry_run: false,
        }
    }
}

/// Line ending normalizer
pub struct EndingsNormalizer {
    options: EndingsOptions,
}

impl EndingsNormalizer {
    /// Creates a new normalizer with the given options
    pub fn new(options: EndingsOptions) -> Self {
        EndingsNormalizer { options }
    }

    /// Creates a normalizer with default options
    pub fn with_defaults() -> Self {
        EndingsNormalizer {
            options: EndingsOptions::default(),
        }
    }

    /// Checks if a file should be processed
    fn should_process(&self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        // Skip hidden files
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

        if let Some(ext) = path.extension() {
            let ext_str = format!(".{}", ext.to_string_lossy());
            self.options.file_extensions.contains(&ext_str)
        } else {
            false
        }
    }

    /// Normalize line endings in a single file. Returns the number of lines changed.
    pub fn normalize_file(&self, path: &Path) -> crate::Result<usize> {
        if !self.should_process(path) {
            return Ok(0);
        }

        let bytes = fs::read(path)?;

        // Detect if file is binary (contains null bytes)
        if bytes.contains(&0) {
            return Ok(0);
        }

        let target = self.options.style;
        let target_bytes = target.as_bytes();

        // Split into lines preserving original endings for counting
        let mut changed = 0usize;
        let mut output: Vec<u8> = Vec::with_capacity(bytes.len());
        let mut i = 0;

        while i < bytes.len() {
            if bytes[i] == b'\r' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    // CRLF
                    if target != LineEnding::Crlf {
                        changed += 1;
                    }
                    output.extend_from_slice(target_bytes);
                    i += 2;
                } else {
                    // CR only
                    if target != LineEnding::Cr {
                        changed += 1;
                    }
                    output.extend_from_slice(target_bytes);
                    i += 1;
                }
            } else if bytes[i] == b'\n' {
                // LF only
                if target != LineEnding::Lf {
                    changed += 1;
                }
                output.extend_from_slice(target_bytes);
                i += 1;
            } else {
                output.push(bytes[i]);
                i += 1;
            }
        }

        if changed > 0 {
            if self.options.dry_run {
                println!(
                    "Would normalize {} line ending(s) in '{}'",
                    changed,
                    path.display()
                );
            } else {
                fs::write(path, output)?;
                println!(
                    "Normalized {} line ending(s) in '{}'",
                    changed,
                    path.display()
                );
            }
        }

        Ok(changed)
    }

    /// Processes a directory or file. Returns (files_changed, endings_changed).
    pub fn process(&self, path: &Path) -> crate::Result<(usize, usize)> {
        let mut total_files = 0;
        let mut total_endings = 0;

        if path.is_file() {
            let endings = self.normalize_file(path)?;
            if endings > 0 {
                total_files = 1;
                total_endings = endings;
            }
        } else if path.is_dir() {
            if self.options.recursive {
                for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        let endings = self.normalize_file(entry.path())?;
                        if endings > 0 {
                            total_files += 1;
                            total_endings += endings;
                        }
                    }
                }
            } else {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        let endings = self.normalize_file(&entry_path)?;
                        if endings > 0 {
                            total_files += 1;
                            total_endings += endings;
                        }
                    }
                }
            }
        }

        Ok((total_files, total_endings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_crlf_to_lf() {
        let dir = std::env::temp_dir().join("reformat_endings_crlf_lf");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, b"line1\r\nline2\r\nline3\r\n").unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, endings) = normalizer.process(&file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(endings, 3);

        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"line1\nline2\nline3\n");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_lf_to_crlf() {
        let dir = std::env::temp_dir().join("reformat_endings_lf_crlf");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, b"line1\nline2\nline3\n").unwrap();

        let options = EndingsOptions {
            style: LineEnding::Crlf,
            ..Default::default()
        };
        let normalizer = EndingsNormalizer::new(options);
        let (files, endings) = normalizer.process(&file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(endings, 3);

        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"line1\r\nline2\r\nline3\r\n");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_cr_to_lf() {
        let dir = std::env::temp_dir().join("reformat_endings_cr_lf");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, b"line1\rline2\rline3\r").unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, endings) = normalizer.process(&file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(endings, 3);

        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"line1\nline2\nline3\n");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_mixed_endings() {
        let dir = std::env::temp_dir().join("reformat_endings_mixed");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, b"line1\r\nline2\nline3\rline4\n").unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, endings) = normalizer.process(&file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(endings, 2); // CRLF and CR converted, LFs already correct

        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"line1\nline2\nline3\nline4\n");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_already_normalized() {
        let dir = std::env::temp_dir().join("reformat_endings_noop");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, b"line1\nline2\nline3\n").unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, endings) = normalizer.process(&file).unwrap();

        assert_eq!(files, 0);
        assert_eq!(endings, 0);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_dry_run() {
        let dir = std::env::temp_dir().join("reformat_endings_dry");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        let original = b"line1\r\nline2\r\n";
        fs::write(&file, original).unwrap();

        let options = EndingsOptions {
            dry_run: true,
            ..Default::default()
        };
        let normalizer = EndingsNormalizer::new(options);
        let (_, endings) = normalizer.process(&file).unwrap();

        assert_eq!(endings, 2);

        // File should be unchanged
        let content = fs::read(&file).unwrap();
        assert_eq!(content, original);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_skip_binary_files() {
        let dir = std::env::temp_dir().join("reformat_endings_binary");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        let mut content = b"line1\r\nline2\r\n".to_vec();
        content.push(0); // null byte makes it binary
        fs::write(&file, &content).unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, _) = normalizer.process(&file).unwrap();

        assert_eq!(files, 0);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_skip_hidden_files() {
        let dir = std::env::temp_dir().join("reformat_endings_hidden");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join(".hidden.txt");
        fs::write(&file, b"line1\r\n").unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, _) = normalizer.process(&file).unwrap();

        assert_eq!(files, 0);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_recursive_processing() {
        let dir = std::env::temp_dir().join("reformat_endings_recursive");
        fs::create_dir_all(&dir).unwrap();

        let sub = dir.join("sub");
        fs::create_dir_all(&sub).unwrap();

        let f1 = dir.join("a.txt");
        let f2 = sub.join("b.txt");
        fs::write(&f1, b"a\r\n").unwrap();
        fs::write(&f2, b"b\r\n").unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, endings) = normalizer.process(&dir).unwrap();

        assert_eq!(files, 2);
        assert_eq!(endings, 2);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_parse_line_ending() {
        assert_eq!(LineEnding::parse("lf"), Some(LineEnding::Lf));
        assert_eq!(LineEnding::parse("LF"), Some(LineEnding::Lf));
        assert_eq!(LineEnding::parse("unix"), Some(LineEnding::Lf));
        assert_eq!(LineEnding::parse("crlf"), Some(LineEnding::Crlf));
        assert_eq!(LineEnding::parse("CRLF"), Some(LineEnding::Crlf));
        assert_eq!(LineEnding::parse("windows"), Some(LineEnding::Crlf));
        assert_eq!(LineEnding::parse("cr"), Some(LineEnding::Cr));
        assert_eq!(LineEnding::parse("mac"), Some(LineEnding::Cr));
        assert_eq!(LineEnding::parse("bogus"), None);
    }
}
