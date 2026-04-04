//! File header management transformer

use regex::Regex;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Options for header management
#[derive(Debug, Clone)]
pub struct HeaderOptions {
    /// The header text to insert (without comment markers -- those are part of the text)
    pub text: String,
    /// If true, replace {year} in the header text with the current year
    pub update_year: bool,
    /// File extensions to process
    pub file_extensions: Vec<String>,
    /// Process directories recursively
    pub recursive: bool,
    /// Dry run mode (don't modify files)
    pub dry_run: bool,
}

impl Default for HeaderOptions {
    fn default() -> Self {
        HeaderOptions {
            text: String::new(),
            update_year: false,
            file_extensions: vec![
                ".py", ".pyx", ".pxd", ".pxi", ".c", ".h", ".cpp", ".hpp", ".rs", ".go", ".java",
                ".js", ".ts", ".jsx", ".tsx",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            recursive: true,
            dry_run: false,
        }
    }
}

/// File header manager: insert or update headers at the top of source files
pub struct HeaderManager {
    options: HeaderOptions,
    /// The resolved header text (with year substitution applied)
    resolved_header: String,
    /// Regex to detect if the header (or a year-variant of it) already exists
    header_detector: Option<Regex>,
}

impl HeaderManager {
    /// Creates a new header manager with the given options
    pub fn new(options: HeaderOptions) -> crate::Result<Self> {
        let resolved_header = if options.update_year {
            let year = chrono::Utc::now().format("%Y").to_string();
            options.text.replace("{year}", &year)
        } else {
            options.text.clone()
        };

        // Build a detector regex: escape the header text but replace any 4-digit year
        // with \d{4} so we can find year-variant headers
        let header_detector =
            if !resolved_header.is_empty() {
                let escaped = regex::escape(&resolved_header);
                // Replace any 4-digit year (19xx or 20xx) with a flexible year pattern
                let flexible = Regex::new(r"(?:19|20)\d\{2\}")
                    .unwrap()
                    .replace_all(&escaped, r"\d{4}")
                    .to_string();
                Some(Regex::new(&flexible).map_err(|e| {
                    anyhow::anyhow!("failed to compile header detection regex: {}", e)
                })?)
            } else {
                None
            };

        Ok(HeaderManager {
            options,
            resolved_header,
            header_detector,
        })
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

    /// Process a single file. Returns true if the file was modified (or would be in dry-run).
    pub fn process_file(&self, path: &Path) -> crate::Result<bool> {
        if !self.should_process(path) {
            return Ok(false);
        }

        if self.resolved_header.is_empty() {
            return Ok(false);
        }

        let content = fs::read_to_string(path)?;

        // Check if header already exists (possibly with a different year)
        if let Some(ref detector) = self.header_detector {
            if let Some(m) = detector.find(&content) {
                // Header exists -- check if it needs a year update
                let existing = &content[m.start()..m.end()];
                if existing == self.resolved_header {
                    // Exact match, nothing to do
                    return Ok(false);
                }

                // Replace old header with new one (year update)
                let new_content = format!("{}{}", self.resolved_header, &content[m.end()..]);
                // Preserve content before the header (e.g., shebang lines)
                let prefix = &content[..m.start()];
                let full = format!("{}{}", prefix, new_content);

                if self.options.dry_run {
                    println!("Would update header in '{}'", path.display());
                } else {
                    fs::write(path, &full)?;
                    println!("Updated header in '{}'", path.display());
                }
                return Ok(true);
            }
        }

        // Header doesn't exist -- insert it
        // Preserve shebang lines (e.g., #!/usr/bin/env python)
        let (prefix, rest) = if content.starts_with("#!") {
            if let Some(pos) = content.find('\n') {
                (&content[..=pos], &content[pos + 1..])
            } else {
                (content.as_str(), "")
            }
        } else {
            ("", content.as_str())
        };

        let new_content = if prefix.is_empty() {
            format!("{}\n\n{}", self.resolved_header, rest)
        } else {
            format!("{}{}\n\n{}", prefix, self.resolved_header, rest)
        };

        if self.options.dry_run {
            println!("Would insert header in '{}'", path.display());
        } else {
            fs::write(path, &new_content)?;
            println!("Inserted header in '{}'", path.display());
        }

        Ok(true)
    }

    /// Processes a directory or file. Returns (files_changed, operation_description).
    pub fn process(&self, path: &Path) -> crate::Result<(usize, usize)> {
        let mut total_files = 0;
        // We use the second value as "operations" (1 per file touched)
        let mut total_ops = 0;

        if path.is_file() {
            if self.process_file(path)? {
                total_files = 1;
                total_ops = 1;
            }
        } else if path.is_dir() {
            if self.options.recursive {
                for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() && self.process_file(entry.path())? {
                        total_files += 1;
                        total_ops += 1;
                    }
                }
            } else {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let entry_path = entry.path();
                    if entry_path.is_file() && self.process_file(&entry_path)? {
                        total_files += 1;
                        total_ops += 1;
                    }
                }
            }
        }

        Ok((total_files, total_ops))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_insert_header() {
        let dir = std::env::temp_dir().join("reformat_header_insert");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.rs");
        fs::write(&file, "fn main() {}\n").unwrap();

        let options = HeaderOptions {
            text: "// Copyright 2025 TestCorp".to_string(),
            ..Default::default()
        };
        let manager = HeaderManager::new(options).unwrap();
        let (files, _) = manager.process(&file).unwrap();

        assert_eq!(files, 1);

        let content = fs::read_to_string(&file).unwrap();
        assert!(content.starts_with("// Copyright 2025 TestCorp\n\n"));
        assert!(content.contains("fn main() {}"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_header_already_present() {
        let dir = std::env::temp_dir().join("reformat_header_exists");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.rs");
        let original = "// Copyright 2025 TestCorp\n\nfn main() {}\n";
        fs::write(&file, original).unwrap();

        let options = HeaderOptions {
            text: "// Copyright 2025 TestCorp".to_string(),
            ..Default::default()
        };
        let manager = HeaderManager::new(options).unwrap();
        let (files, _) = manager.process(&file).unwrap();

        assert_eq!(files, 0);

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, original);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_update_year_in_header() {
        let dir = std::env::temp_dir().join("reformat_header_year");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.rs");
        fs::write(&file, "// Copyright 2020 TestCorp\n\nfn main() {}\n").unwrap();

        let current_year = chrono::Utc::now().format("%Y").to_string();
        let options = HeaderOptions {
            text: format!("// Copyright {} TestCorp", current_year),
            ..Default::default()
        };
        let manager = HeaderManager::new(options).unwrap();
        let (files, _) = manager.process(&file).unwrap();

        assert_eq!(files, 1);

        let content = fs::read_to_string(&file).unwrap();
        assert!(content.starts_with(&format!("// Copyright {} TestCorp", current_year)));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_preserve_shebang() {
        let dir = std::env::temp_dir().join("reformat_header_shebang");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.py");
        fs::write(&file, "#!/usr/bin/env python\nprint('hello')\n").unwrap();

        let options = HeaderOptions {
            text: "# Copyright 2025 TestCorp".to_string(),
            file_extensions: vec![".py".to_string()],
            ..Default::default()
        };
        let manager = HeaderManager::new(options).unwrap();
        manager.process(&file).unwrap();

        let content = fs::read_to_string(&file).unwrap();
        assert!(content.starts_with("#!/usr/bin/env python\n"));
        assert!(content.contains("# Copyright 2025 TestCorp"));
        assert!(content.contains("print('hello')"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_dry_run() {
        let dir = std::env::temp_dir().join("reformat_header_dry");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.rs");
        let original = "fn main() {}\n";
        fs::write(&file, original).unwrap();

        let options = HeaderOptions {
            text: "// License Header".to_string(),
            dry_run: true,
            ..Default::default()
        };
        let manager = HeaderManager::new(options).unwrap();
        let (files, _) = manager.process(&file).unwrap();

        assert_eq!(files, 1);
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, original);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_empty_header() {
        let dir = std::env::temp_dir().join("reformat_header_empty");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.rs");
        fs::write(&file, "fn main() {}\n").unwrap();

        let options = HeaderOptions {
            text: String::new(),
            ..Default::default()
        };
        let manager = HeaderManager::new(options).unwrap();
        let (files, _) = manager.process(&file).unwrap();

        assert_eq!(files, 0);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_year_template_substitution() {
        let dir = std::env::temp_dir().join("reformat_header_template");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.rs");
        fs::write(&file, "fn main() {}\n").unwrap();

        let options = HeaderOptions {
            text: "// Copyright {year} TestCorp".to_string(),
            update_year: true,
            ..Default::default()
        };
        let manager = HeaderManager::new(options).unwrap();
        manager.process(&file).unwrap();

        let current_year = chrono::Utc::now().format("%Y").to_string();
        let content = fs::read_to_string(&file).unwrap();
        assert!(content.contains(&format!("Copyright {} TestCorp", current_year)));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_recursive_processing() {
        let dir = std::env::temp_dir().join("reformat_header_recursive");
        fs::create_dir_all(&dir).unwrap();

        let sub = dir.join("sub");
        fs::create_dir_all(&sub).unwrap();

        let f1 = dir.join("a.rs");
        let f2 = sub.join("b.rs");
        fs::write(&f1, "fn a() {}\n").unwrap();
        fs::write(&f2, "fn b() {}\n").unwrap();

        let options = HeaderOptions {
            text: "// Header".to_string(),
            ..Default::default()
        };
        let manager = HeaderManager::new(options).unwrap();
        let (files, _) = manager.process(&dir).unwrap();

        assert_eq!(files, 2);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_multiline_header() {
        let dir = std::env::temp_dir().join("reformat_header_multiline");
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.rs");
        fs::write(&file, "fn main() {}\n").unwrap();

        let options = HeaderOptions {
            text: "// Copyright 2025 TestCorp\n// Licensed under MIT\n// All rights reserved"
                .to_string(),
            ..Default::default()
        };
        let manager = HeaderManager::new(options).unwrap();
        manager.process(&file).unwrap();

        let content = fs::read_to_string(&file).unwrap();
        assert!(content.starts_with(
            "// Copyright 2025 TestCorp\n// Licensed under MIT\n// All rights reserved\n\n"
        ));

        fs::remove_dir_all(&dir).unwrap();
    }
}
