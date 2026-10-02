//! Regex find-and-replace transformer

use regex::{Regex, RegexBuilder};
use std::path::Path;

use crate::step::{ContentStep, FileTarget};

/// A single find-and-replace pattern
#[derive(Debug, Clone)]
pub struct ReplacePattern {
    /// Regex pattern to search for
    pub find: String,
    /// Replacement string (supports capture groups: $1, $2, etc.)
    pub replace: String,
}

/// Options for content replacement
#[derive(Debug, Clone)]
pub struct ReplaceOptions {
    /// Ordered list of patterns to apply
    pub patterns: Vec<ReplacePattern>,
    /// File extensions to process; empty matches every file
    pub file_extensions: Vec<String>,
    /// Process directories recursively
    pub recursive: bool,
    /// Dry run mode (don't modify files)
    pub dry_run: bool,
}

impl Default for ReplaceOptions {
    fn default() -> Self {
        ReplaceOptions {
            patterns: Vec::new(),
            file_extensions: vec![
                ".py", ".pyx", ".pxd", ".pxi", ".c", ".h", ".cpp", ".hpp", ".rs", ".go", ".java",
                ".js", ".ts", ".jsx", ".tsx", ".md", ".qmd", ".txt", ".toml", ".yaml", ".yml",
                ".json", ".xml", ".html", ".css", ".sh",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            recursive: true,
            dry_run: false,
        }
    }
}

/// Compiled replacement pattern
#[derive(Debug)]
struct CompiledPattern {
    regex: Regex,
    replace: String,
}

/// Content replacer that applies regex find-and-replace across files
#[derive(Debug)]
pub struct ContentReplacer {
    options: ReplaceOptions,
    compiled: Vec<CompiledPattern>,
}

impl ContentReplacer {
    /// Creates a new replacer with the given options.
    /// Returns an error if any regex pattern is invalid.
    pub fn new(options: ReplaceOptions) -> crate::Result<Self> {
        let mut compiled = Vec::with_capacity(options.patterns.len());
        for pattern in &options.patterns {
            // CRLF mode stops `.` at `\r`, so `x = .*` keeps a CRLF terminator.
            let regex = RegexBuilder::new(&pattern.find)
                .crlf(true)
                .build()
                .map_err(|e| anyhow::anyhow!("invalid regex pattern '{}': {}", pattern.find, e))?;
            check_group_refs(&regex, &pattern.find, &pattern.replace)?;
            compiled.push(CompiledPattern {
                regex,
                replace: pattern.replace.clone(),
            });
        }
        Ok(ContentReplacer { options, compiled })
    }

    /// Apply all patterns to a single file. Returns number of replacements made.
    pub fn replace_file(&self, path: &Path) -> crate::Result<usize> {
        if !path.is_file() {
            return Ok(0);
        }
        crate::step::apply_one(self, &FileTarget::file(path), self.options.dry_run)
    }

    /// Processes a directory or file. Returns (files_changed, total_replacements).
    pub fn process(&self, path: &Path) -> crate::Result<(usize, usize)> {
        crate::step::process_path(self, path, self.options.recursive, self.options.dry_run)
    }
}

/// Fails if `replace` refers to a capture group that `regex` lacks.
///
/// The regex crate expands such a reference to the empty string. `$1_new`
/// names a group `1_new`, so it silently deleted the match.
fn check_group_refs(regex: &Regex, find: &str, replace: &str) -> crate::Result<()> {
    let bytes = replace.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'$' {
            i += 1;
            continue;
        }
        let rest = &replace[i + 1..];
        let (name, len) = if rest.starts_with('$') {
            ("", 1)
        } else if let Some(braced) = rest.strip_prefix('{') {
            match braced.find('}') {
                Some(end) => (&braced[..end], end + 2),
                None => ("", 0),
            }
        } else {
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            (&rest[..end], end)
        };
        let defined = match name.parse::<usize>() {
            Ok(index) => index < regex.captures_len(),
            Err(_) => name.is_empty() || regex.capture_names().any(|n| n == Some(name)),
        };
        if !defined {
            anyhow::bail!(
                "replacement '{}' refers to group '{}', which pattern '{}' does not define \
                 (write ${{1}} to follow a group number with a letter, digit or '_', \
                 or $$ for a literal $)",
                replace,
                name,
                find
            );
        }
        i += 1 + len;
    }
    Ok(())
}

impl ContentStep for ContentReplacer {
    fn name(&self) -> &'static str {
        "replace"
    }

    fn accepts(&self, file: &FileTarget) -> bool {
        !self.compiled.is_empty()
            && crate::step::accepts_by_extension(
                file,
                &self.options.file_extensions,
                self.options.recursive,
            )
    }

    fn transform(&self, text: &str, _file: &FileTarget) -> Option<(String, usize)> {
        let mut current = text.to_string();
        let mut total_replacements = 0;

        for cp in &self.compiled {
            let count = cp.regex.find_iter(&current).count();
            if count > 0 {
                let result = cp.regex.replace_all(&current, cp.replace.as_str());
                if result != current {
                    total_replacements += count;
                    current = result.into_owned();
                }
            }
        }

        (current != text).then_some((current, total_replacements))
    }

    fn describe(&self, units: usize, dry_run: bool) -> String {
        if dry_run {
            format!("Would make {} replacement(s) in", units)
        } else {
            format!("Made {} replacement(s) in", units)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_simple_replacement() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, "hello world\nhello rust\n").unwrap();

        let options = ReplaceOptions {
            patterns: vec![ReplacePattern {
                find: "hello".to_string(),
                replace: "greetings".to_string(),
            }],
            ..Default::default()
        };
        let replacer = ContentReplacer::new(options).unwrap();
        let (files, replacements) = replacer.process(&file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(replacements, 2);

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "greetings world\ngreetings rust\n");
    }

    #[test]
    fn test_regex_pattern() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, "foo123 bar456 baz\n").unwrap();

        let options = ReplaceOptions {
            patterns: vec![ReplacePattern {
                find: r"[a-z]+(\d+)".to_string(),
                replace: "num_$1".to_string(),
            }],
            ..Default::default()
        };
        let replacer = ContentReplacer::new(options).unwrap();
        let (files, replacements) = replacer.process(&file).unwrap();

        assert_eq!(files, 1);
        assert_eq!(replacements, 2);

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "num_123 num_456 baz\n");
    }

    #[test]
    fn test_multiple_patterns_sequential() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, "Copyright 2024 OldCorp\n").unwrap();

        let options = ReplaceOptions {
            patterns: vec![
                ReplacePattern {
                    find: "2024".to_string(),
                    replace: "2025".to_string(),
                },
                ReplacePattern {
                    find: "OldCorp".to_string(),
                    replace: "NewCorp".to_string(),
                },
            ],
            ..Default::default()
        };
        let replacer = ContentReplacer::new(options).unwrap();
        replacer.process(&file).unwrap();

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "Copyright 2025 NewCorp\n");
    }

    #[test]
    fn test_no_matches() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, "nothing to change\n").unwrap();

        let options = ReplaceOptions {
            patterns: vec![ReplacePattern {
                find: "xyz".to_string(),
                replace: "abc".to_string(),
            }],
            ..Default::default()
        };
        let replacer = ContentReplacer::new(options).unwrap();
        let (files, replacements) = replacer.process(&file).unwrap();

        assert_eq!(files, 0);
        assert_eq!(replacements, 0);
    }

    #[test]
    fn test_invalid_regex() {
        let options = ReplaceOptions {
            patterns: vec![ReplacePattern {
                find: "[invalid".to_string(),
                replace: "x".to_string(),
            }],
            ..Default::default()
        };
        let result = ContentReplacer::new(options);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid regex"));
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
        let original = "hello world\n";
        fs::write(&file, original).unwrap();

        let options = ReplaceOptions {
            patterns: vec![ReplacePattern {
                find: "hello".to_string(),
                replace: "bye".to_string(),
            }],
            dry_run: true,
            ..Default::default()
        };
        let replacer = ContentReplacer::new(options).unwrap();
        let (_, replacements) = replacer.process(&file).unwrap();

        assert_eq!(replacements, 1);
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn test_empty_patterns() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, "content\n").unwrap();

        let options = ReplaceOptions {
            patterns: vec![],
            ..Default::default()
        };
        let replacer = ContentReplacer::new(options).unwrap();
        let (files, _) = replacer.process(&file).unwrap();

        assert_eq!(files, 0);
    }

    #[test]
    fn test_recursive_replacement() {
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
        fs::write(&f1, "old\n").unwrap();
        fs::write(&f2, "old\n").unwrap();

        let options = ReplaceOptions {
            patterns: vec![ReplacePattern {
                find: "old".to_string(),
                replace: "new".to_string(),
            }],
            ..Default::default()
        };
        let replacer = ContentReplacer::new(options).unwrap();
        let (files, _) = replacer.process(&dir).unwrap();

        assert_eq!(files, 2);
    }

    #[test]
    fn test_capture_group_replacement() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.txt");
        fs::write(&file, "func(a, b)\nfunc(x, y)\n").unwrap();

        let options = ReplaceOptions {
            patterns: vec![ReplacePattern {
                find: r"func\((\w+), (\w+)\)".to_string(),
                replace: "call($2, $1)".to_string(),
            }],
            ..Default::default()
        };
        let replacer = ContentReplacer::new(options).unwrap();
        replacer.process(&file).unwrap();

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "call(b, a)\ncall(y, x)\n");
    }

    fn replacer(find: &str, replace: &str) -> crate::Result<ContentReplacer> {
        ContentReplacer::new(ReplaceOptions {
            patterns: vec![ReplacePattern {
                find: find.to_string(),
                replace: replace.to_string(),
            }],
            ..Default::default()
        })
    }

    #[test]
    fn test_dot_does_not_consume_cr() {
        let r = replacer("x = .*", "x = 1").unwrap();
        let file = FileTarget::file(Path::new("a.py"));
        let (out, _) = r.transform("x = 0\r\ny = 2\r\n", &file).unwrap();
        assert_eq!(out, "x = 1\r\ny = 2\r\n");
        let (out, _) = r.transform("x = 0\ry = 2\r", &file).unwrap();
        assert_eq!(out, "x = 1\ry = 2\r");
    }

    #[test]
    fn test_unknown_group_reference_is_rejected() {
        assert!(replacer(r"(\w+)\.old", "$1_new").is_err());
        assert!(replacer(r"(\w+)", "$2").is_err());
        assert!(replacer(r"(\w+)", "${nope}").is_err());
        assert!(replacer(r"(\w+)\.old", "${1}_new").is_ok());
        assert!(replacer(r"(?P<n>\w+)", "$n $0 $$5 $ {").is_ok());
    }
}
