//! File header management transformer

use regex::Regex;
use std::path::Path;

use crate::step::{ContentStep, FileTarget};

/// Options for header management
#[derive(Debug, Clone)]
pub struct HeaderOptions {
    /// The header text to insert (without comment markers -- those are part of the text)
    pub text: String,
    /// If true, replace {year} in the header text with the current year
    pub update_year: bool,
    /// File extensions to process; empty matches every file
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

const BOM: &str = "\u{FEFF}";

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

        // Escape the header, then let any year and any line terminator vary,
        // so a header from a previous year or in a CRLF file is recognised.
        // `\d{2}` is a quantifier; escaping its braces matched a literal "{2}".
        let header_detector =
            if !resolved_header.is_empty() {
                let escaped = regex::escape(&resolved_header);
                let flexible = Regex::new(r"(?:19|20)\d{2}")
                    .unwrap()
                    .replace_all(&escaped, r"\d{4}")
                    .replace('\n', r"(?:\r\n|\n|\r)");
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

    /// Byte offset at which a header may legitimately begin: the start of the
    /// file, skipping a shebang line and any leading blank lines.
    fn header_zone(content: &str) -> usize {
        let mut pos = 0;
        if content.starts_with("#!") {
            pos = content.find('\n').map(|i| i + 1).unwrap_or(content.len());
        }
        let rest = &content[pos..];
        let trimmed = rest.trim_start_matches(['\n', '\r', ' ', '\t']);
        pos + (rest.len() - trimmed.len())
    }

    /// Process a single file. Returns true if the file was modified (or would be in dry-run).
    pub fn process_file(&self, path: &Path) -> crate::Result<bool> {
        if !path.is_file() {
            return Ok(false);
        }
        Ok(crate::step::apply_one(self, &FileTarget::file(path), self.options.dry_run)? > 0)
    }

    /// Processes a directory or file. Returns (files_changed, files_changed).
    pub fn process(&self, path: &Path) -> crate::Result<(usize, usize)> {
        crate::step::process_path(self, path, self.options.recursive, self.options.dry_run)
    }
}

impl ContentStep for HeaderManager {
    fn name(&self) -> &'static str {
        "header"
    }

    fn accepts(&self, file: &FileTarget) -> bool {
        !self.resolved_header.is_empty()
            && crate::step::accepts_by_extension(
                file,
                &self.options.file_extensions,
                self.options.recursive,
            )
    }

    fn transform(&self, text: &str, _file: &FileTarget) -> Option<(String, usize)> {
        // A byte-order mark must stay the first thing in the file.
        let (bom, content) = match text.strip_prefix(BOM) {
            Some(rest) => (BOM, rest),
            None => ("", text),
        };
        // The header is written with the file's own terminator, so a CRLF
        // file does not gain LF lines.
        let newline = crate::lines::first_terminator(content);
        let header = self.resolved_header.replace('\n', newline);

        // Detection is anchored to the header zone, so a year-variant string
        // elsewhere in the body is not mistaken for this file's header.
        let zone = Self::header_zone(content);
        if let Some(ref detector) = self.header_detector {
            if let Some(m) = detector
                .find_at(content, zone)
                .filter(|m| m.start() == zone)
            {
                if m.as_str() == header {
                    return None;
                }
                let updated = format!(
                    "{}{}{}{}",
                    bom,
                    &content[..m.start()],
                    header,
                    &content[m.end()..]
                );
                return Some((updated, 1));
            }
        }

        // Insert after a shebang line, if there is one.
        let split = if content.starts_with("#!") {
            content
                .find('\n')
                .map(|pos| pos + 1)
                .unwrap_or(content.len())
        } else {
            0
        };
        let (prefix, rest) = content.split_at(split);
        let prefix_newline = if !prefix.is_empty() && !prefix.ends_with('\n') {
            newline
        } else {
            ""
        };
        let updated = format!(
            "{}{}{}{}{}{}{}",
            bom, prefix, prefix_newline, header, newline, newline, rest
        );
        Some((updated, 1))
    }

    fn describe(&self, _units: usize, dry_run: bool) -> String {
        if dry_run {
            "Would update header in".to_string()
        } else {
            "Updated header in".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_insert_header() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
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
    }

    #[test]
    fn test_header_already_present() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
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
    }

    /// An existing header carrying a different year must be *replaced*, not
    /// shadowed by a second header inserted above it. Asserting only
    /// `starts_with` is not enough: that passes when the old header is
    /// duplicated below the new one, which is exactly what used to happen.
    #[test]
    fn test_update_year_in_header() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.rs");
        fs::write(&file, "// Copyright 2020 TestCorp\n\nfn main() {}\n").unwrap();

        let current_year = chrono::Utc::now().format("%Y").to_string();
        let header = format!("// Copyright {} TestCorp", current_year);
        let options = HeaderOptions {
            text: header.clone(),
            ..Default::default()
        };
        let manager = HeaderManager::new(options).unwrap();
        let (files, _) = manager.process(&file).unwrap();

        assert_eq!(files, 1);

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(
            content,
            format!("{}\n\nfn main() {{}}\n", header),
            "the old header should have been replaced in place"
        );
        assert!(
            !content.contains("2020"),
            "the superseded year is still present -- the header was duplicated"
        );
        assert_eq!(
            content.matches("TestCorp").count(),
            1,
            "the file gained a second header instead of having one updated"
        );
    }

    /// The `{year}` template plus `--update-year` must be idempotent across
    /// years: running it in successive years leaves exactly one header.
    #[test]
    fn test_update_year_is_idempotent_across_years() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.rs");
        // Simulates a header written in a previous year.
        fs::write(&file, "// Copyright 1999 TestCorp\n\nfn main() {}\n").unwrap();

        let options = HeaderOptions {
            text: "// Copyright {year} TestCorp".to_string(),
            update_year: true,
            ..Default::default()
        };

        // First run updates 1999 -> current year.
        let manager = HeaderManager::new(options.clone()).unwrap();
        assert!(manager.process_file(&file).unwrap());

        // Second run is a no-op: the header already carries the current year.
        let manager = HeaderManager::new(options).unwrap();
        assert!(
            !manager.process_file(&file).unwrap(),
            "a second run in the same year should change nothing"
        );

        let content = fs::read_to_string(&file).unwrap();
        let year = chrono::Utc::now().format("%Y").to_string();
        assert_eq!(
            content,
            format!("// Copyright {} TestCorp\n\nfn main() {{}}\n", year)
        );
        assert_eq!(content.matches("Copyright").count(), 1);
    }

    /// A year-variant header buried in the body of a file (a test fixture, a
    /// vendored blob) must not be mistaken for the file's own header.
    #[test]
    fn test_header_detection_is_anchored_to_top_of_file() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("test.rs");
        let body = "fn main() {}\n\nconst FIXTURE: &str = \"// Copyright 2020 TestCorp\";\n";
        fs::write(&file, body).unwrap();

        let header = "// Copyright 2026 TestCorp".to_string();
        let options = HeaderOptions {
            text: header.clone(),
            ..Default::default()
        };
        let manager = HeaderManager::new(options).unwrap();
        manager.process_file(&file).unwrap();

        let content = fs::read_to_string(&file).unwrap();
        assert!(
            content.starts_with(&header),
            "the header should have been inserted at the top"
        );
        assert!(
            content.contains("// Copyright 2020 TestCorp\";"),
            "the mid-file fixture string was rewritten"
        );
    }

    #[test]
    fn test_preserve_shebang() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
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
    }

    #[test]
    fn test_dry_run() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
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
    }

    #[test]
    fn test_empty_header() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
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
    }

    #[test]
    fn test_year_template_substitution() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
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
    }

    #[test]
    fn test_multiline_header() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let dir = _tmp.path().to_path_buf();
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
    }

    fn apply(text: &str, header: &str) -> String {
        let manager = HeaderManager::new(HeaderOptions {
            text: header.to_string(),
            ..Default::default()
        })
        .unwrap();
        let target = FileTarget::file(Path::new("x.rs"));
        manager
            .transform(text, &target)
            .map(|(s, _)| s)
            .unwrap_or_else(|| text.to_string())
    }

    /// Inserting before a BOM moved it mid-file, where compilers reject it.
    #[test]
    fn test_header_goes_after_byte_order_mark() {
        let out = apply("\u{FEFF}int x;\n", "// H");
        assert_eq!(out, "\u{FEFF}// H\n\nint x;\n");
        assert_eq!(apply(&out, "// H"), out, "a second run must change nothing");
    }

    /// A CRLF file used to gain LF separators, leaving mixed line endings.
    #[test]
    fn test_header_uses_file_line_terminator() {
        let out = apply("int x;\r\nint y;\r\n", "// A\n// B");
        assert_eq!(out, "// A\r\n// B\r\n\r\nint x;\r\nint y;\r\n");
        assert_eq!(
            apply(&out, "// A\n// B"),
            out,
            "a second run must change nothing"
        );
    }

    #[test]
    fn test_shebang_without_trailing_newline() {
        assert_eq!(apply("#!/bin/sh", "# H"), "#!/bin/sh\n# H\n\n");
    }
}
