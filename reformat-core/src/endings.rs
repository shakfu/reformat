//! Line ending normalization transformer

use std::path::Path;

use crate::step::{ContentStep, FileTarget};

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
    /// File extensions to process; empty matches every file
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

    /// Normalize line endings in a single file. Returns the number of lines changed.
    pub fn normalize_file(&self, path: &Path) -> crate::Result<usize> {
        if !path.is_file() {
            return Ok(0);
        }
        crate::step::apply_one(self, &FileTarget::file(path), self.options.dry_run)
    }

    /// Processes a directory or file. Returns (files_changed, endings_changed).
    pub fn process(&self, path: &Path) -> crate::Result<(usize, usize)> {
        crate::step::process_path(self, path, self.options.recursive, self.options.dry_run)
    }

    /// Rewrites every line ending in `bytes` to the target style. Returns the
    /// new bytes and the number of endings that differed.
    pub fn normalize_bytes(&self, bytes: &[u8]) -> (Vec<u8>, usize) {
        let target = self.options.style;
        let target_bytes = target.as_bytes();
        let mut changed = 0usize;
        let mut output: Vec<u8> = Vec::with_capacity(bytes.len());
        let mut i = 0;

        while i < bytes.len() {
            let (found, len) = match bytes[i] {
                b'\r' if bytes.get(i + 1) == Some(&b'\n') => (Some(LineEnding::Crlf), 2),
                b'\r' => (Some(LineEnding::Cr), 1),
                b'\n' => (Some(LineEnding::Lf), 1),
                _ => (None, 1),
            };
            match found {
                Some(ending) => {
                    if ending != target {
                        changed += 1;
                    }
                    output.extend_from_slice(target_bytes);
                }
                None => output.push(bytes[i]),
            }
            i += len;
        }

        (output, changed)
    }
}

impl ContentStep for EndingsNormalizer {
    fn name(&self) -> &'static str {
        "endings"
    }

    fn accepts(&self, file: &FileTarget) -> bool {
        crate::step::accepts_by_extension(
            file,
            &self.options.file_extensions,
            self.options.recursive,
        )
    }

    fn transform(&self, text: &str, _file: &FileTarget) -> Option<(String, usize)> {
        let (bytes, changed) = self.normalize_bytes(text.as_bytes());
        // Only CR and LF bytes are rewritten, so the output is still UTF-8.
        (changed > 0).then(|| {
            (
                String::from_utf8(bytes).expect("line endings are ASCII"),
                changed,
            )
        })
    }

    fn transform_bytes(&self, bytes: &[u8], _file: &FileTarget) -> Option<(Vec<u8>, usize)> {
        let (out, changed) = self.normalize_bytes(bytes);
        (changed > 0).then_some((out, changed))
    }

    fn supports_bytes(&self) -> bool {
        true
    }

    fn describe(&self, units: usize, dry_run: bool) -> String {
        if dry_run {
            format!("Would normalize {} line ending(s) in", units)
        } else {
            format!("Normalized {} line ending(s) in", units)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_crlf_to_lf() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, b"line1\r\nline2\r\nline3\r\n").unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, endings) = normalizer.process(&file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(endings, 3);

        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"line1\nline2\nline3\n");
    }

    #[test]
    fn test_lf_to_crlf() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
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
    }

    #[test]
    fn test_cr_to_lf() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, b"line1\rline2\rline3\r").unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, endings) = normalizer.process(&file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(endings, 3);

        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"line1\nline2\nline3\n");
    }

    #[test]
    fn test_mixed_endings() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, b"line1\r\nline2\nline3\rline4\n").unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, endings) = normalizer.process(&file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(endings, 2); // CRLF and CR converted, LFs already correct

        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"line1\nline2\nline3\nline4\n");
    }

    #[test]
    fn test_already_normalized() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, b"line1\nline2\nline3\n").unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, endings) = normalizer.process(&file).unwrap();

        assert_eq!(files, 0);
        assert_eq!(endings, 0);
    }

    #[test]
    fn test_dry_run() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
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
    }

    #[test]
    fn test_skip_binary_files() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        let mut content = b"line1\r\nline2\r\n".to_vec();
        content.push(0); // null byte makes it binary
        fs::write(&file, &content).unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, _) = normalizer.process(&file).unwrap();

        assert_eq!(files, 0);
    }

    #[test]
    fn test_skip_hidden_files() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join(".hidden.txt");
        fs::write(&file, b"line1\r\n").unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, _) = normalizer.process(&file).unwrap();

        assert_eq!(files, 0);
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

        let f1 = dir.join("a.txt");
        let f2 = sub.join("b.txt");
        fs::write(&f1, b"a\r\n").unwrap();
        fs::write(&f2, b"b\r\n").unwrap();

        let normalizer = EndingsNormalizer::with_defaults();
        let (files, endings) = normalizer.process(&dir).unwrap();

        assert_eq!(files, 2);
        assert_eq!(endings, 2);
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
