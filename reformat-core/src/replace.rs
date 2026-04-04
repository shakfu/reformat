//! Regex find-and-replace transformer

use regex::Regex;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

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
    /// File extensions to process
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
            let regex = Regex::new(&pattern.find)
                .map_err(|e| anyhow::anyhow!("invalid regex pattern '{}': {}", pattern.find, e))?;
            compiled.push(CompiledPattern {
                regex,
                replace: pattern.replace.clone(),
            });
        }
        Ok(ContentReplacer { options, compiled })
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

    /// Apply all patterns to a single file. Returns number of replacements made.
    pub fn replace_file(&self, path: &Path) -> crate::Result<usize> {
        if !self.should_process(path) {
            return Ok(0);
        }

        if self.compiled.is_empty() {
            return Ok(0);
        }

        let content = fs::read_to_string(path)?;
        let mut current = content.clone();
        let mut total_replacements = 0;

        for cp in &self.compiled {
            let result = cp.regex.replace_all(&current, cp.replace.as_str());
            if result != current {
                // Count individual matches for this pattern
                let count = cp.regex.find_iter(&current).count();
                total_replacements += count;
                current = result.into_owned();
            }
        }

        if total_replacements > 0 {
            if self.options.dry_run {
                println!(
                    "Would make {} replacement(s) in '{}'",
                    total_replacements,
                    path.display()
                );
            } else {
                fs::write(path, &current)?;
                println!(
                    "Made {} replacement(s) in '{}'",
                    total_replacements,
                    path.display()
                );
            }
        }

        Ok(total_replacements)
    }

    /// Processes a directory or file. Returns (files_changed, total_replacements).
    pub fn process(&self, path: &Path) -> crate::Result<(usize, usize)> {
        let mut total_files = 0;
        let mut total_replacements = 0;

        if path.is_file() {
            let replacements = self.replace_file(path)?;
            if replacements > 0 {
                total_files = 1;
                total_replacements = replacements;
            }
        } else if path.is_dir() {
            if self.options.recursive {
                for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        let replacements = self.replace_file(entry.path())?;
                        if replacements > 0 {
                            total_files += 1;
                            total_replacements += replacements;
                        }
                    }
                }
            } else {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        let replacements = self.replace_file(&entry_path)?;
                        if replacements > 0 {
                            total_files += 1;
                            total_replacements += replacements;
                        }
                    }
                }
            }
        }

        Ok((total_files, total_replacements))
    }
}

/// Serde-compatible pattern for config deserialization
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ReplacePatternConfig {
    pub find: String,
    pub replace: String,
}

impl From<ReplacePatternConfig> for ReplacePattern {
    fn from(cfg: ReplacePatternConfig) -> Self {
        ReplacePattern {
            find: cfg.find,
            replace: cfg.replace,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_simple_replacement() {
        let dir = std::env::temp_dir().join("reformat_replace_simple");
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

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_regex_pattern() {
        let dir = std::env::temp_dir().join("reformat_replace_regex");
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

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_multiple_patterns_sequential() {
        let dir = std::env::temp_dir().join("reformat_replace_multi");
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

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_no_matches() {
        let dir = std::env::temp_dir().join("reformat_replace_none");
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

        fs::remove_dir_all(&dir).unwrap();
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
        let dir = std::env::temp_dir().join("reformat_replace_dry");
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

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_empty_patterns() {
        let dir = std::env::temp_dir().join("reformat_replace_empty");
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

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_recursive_replacement() {
        let dir = std::env::temp_dir().join("reformat_replace_recursive");
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

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_capture_group_replacement() {
        let dir = std::env::temp_dir().join("reformat_replace_capture");
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

        fs::remove_dir_all(&dir).unwrap();
    }
}
