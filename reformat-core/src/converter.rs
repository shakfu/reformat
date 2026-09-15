//! Case converter implementation for file processing

use crate::case::CaseFormat;
use regex::Regex;
use std::path::Path;

use crate::step::{ContentStep, FileTarget};

/// Options for case conversion.
///
/// Build with [`ConvertOptions::new`] and struct update syntax:
///
/// ```
/// use reformat_core::{CaseConverter, CaseFormat, ConvertOptions};
///
/// let converter = CaseConverter::new(ConvertOptions {
///     file_extensions: vec![".py".to_string()],
///     strip_prefix: Some("m_".to_string()),
///     ..ConvertOptions::new(CaseFormat::CamelCase, CaseFormat::SnakeCase)
/// })
/// .unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    /// Case format of the identifiers to convert
    pub from: CaseFormat,
    /// Case format to convert them to
    pub to: CaseFormat,
    /// File extensions to process; empty matches every file
    pub file_extensions: Vec<String>,
    /// Process directories recursively
    pub recursive: bool,
    /// Dry run mode (don't modify files)
    pub dry_run: bool,
    /// Added to every converted identifier
    pub prefix: String,
    /// Added to every converted identifier
    pub suffix: String,
    /// Removed before conversion, e.g. `m_` from `m_userName`
    pub strip_prefix: Option<String>,
    /// Removed before conversion
    pub strip_suffix: Option<String>,
    /// `(from, to)`: a prefix replaced before conversion, e.g. `("I", "Abstract")`
    pub replace_prefix: Option<(String, String)>,
    /// `(from, to)`: a suffix replaced before conversion
    pub replace_suffix: Option<(String, String)>,
    /// Only process files whose name or relative path matches this glob
    pub glob: Option<String>,
    /// Only convert identifiers matching this regex
    pub word_filter: Option<String>,
}

impl ConvertOptions {
    /// Options converting `from` to `to`, with every other setting at its default.
    pub fn new(from: CaseFormat, to: CaseFormat) -> Self {
        ConvertOptions {
            from,
            to,
            file_extensions: [
                ".c", ".h", ".py", ".md", ".js", ".ts", ".java", ".cpp", ".hpp",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            recursive: true,
            dry_run: false,
            prefix: String::new(),
            suffix: String::new(),
            strip_prefix: None,
            strip_suffix: None,
            replace_prefix: None,
            replace_suffix: None,
            glob: None,
            word_filter: None,
        }
    }
}

/// Main converter for transforming case formats in files
pub struct CaseConverter {
    options: ConvertOptions,
    glob_pattern: Option<glob::Pattern>,
    word_filter: Option<Regex>,
    source_pattern: Regex,
}

/// Builds the regex used to find conversion candidates, widening it to admit
/// any configured prefix or suffix so that the affix options can act on what
/// was matched.
fn build_source_pattern(
    format: CaseFormat,
    prefixes: [Option<&str>; 2],
    suffixes: [Option<&str>; 2],
) -> String {
    let base = format.pattern();
    // Every format pattern is anchored with \b at both ends; work with the core.
    let core = base
        .strip_prefix(r"\b")
        .unwrap_or(base)
        .strip_suffix(r"\b")
        .unwrap_or(base);

    let group = |affixes: [Option<&str>; 2]| -> String {
        let alts: Vec<String> = affixes
            .iter()
            .flatten()
            .filter(|a| !a.is_empty())
            .map(|a| regex::escape(a))
            .collect();
        if alts.is_empty() {
            String::new()
        } else {
            format!("(?:{})?", alts.join("|"))
        }
    };

    format!(r"\b{}{}{}\b", group(prefixes), core, group(suffixes))
}

impl CaseConverter {
    /// Creates a new case converter. Fails if `glob` or `word_filter` is invalid.
    pub fn new(options: ConvertOptions) -> crate::Result<Self> {
        fn first(pair: &Option<(String, String)>) -> Option<&str> {
            pair.as_ref().map(|(from, _)| from.as_str())
        }

        // The affix options act on an identifier after it has been matched,
        // so the match must be able to include the affix: `\b[a-z]+...` alone
        // never matches `m_userName`, since `_` is a word character.
        let source_pattern = Regex::new(&build_source_pattern(
            options.from,
            [
                options.strip_prefix.as_deref(),
                first(&options.replace_prefix),
            ],
            [
                options.strip_suffix.as_deref(),
                first(&options.replace_suffix),
            ],
        ))?;
        let glob_pattern = options
            .glob
            .as_deref()
            .map(glob::Pattern::new)
            .transpose()?;
        let word_filter = options.word_filter.as_deref().map(Regex::new).transpose()?;

        Ok(CaseConverter {
            options,
            glob_pattern,
            word_filter,
            source_pattern,
        })
    }

    /// Converts a single identifier
    fn convert(&self, name: &str) -> String {
        let o = &self.options;
        let mut processed = name.to_string();

        if let Some(rest) = o
            .strip_prefix
            .as_deref()
            .and_then(|p| processed.strip_prefix(p))
        {
            processed = rest.to_string();
        }
        if let Some(rest) = o
            .strip_suffix
            .as_deref()
            .and_then(|p| processed.strip_suffix(p))
        {
            processed = rest.to_string();
        }
        if let Some((from, to)) = &o.replace_prefix {
            if let Some(rest) = processed.strip_prefix(from.as_str()) {
                processed = format!("{}{}", to, rest);
            }
        }
        if let Some((from, to)) = &o.replace_suffix {
            if let Some(rest) = processed.strip_suffix(from.as_str()) {
                processed = format!("{}{}", rest, to);
            }
        }

        if let Some(ref filter) = self.word_filter {
            if !filter.is_match(&processed) {
                return name.to_string();
            }
        }

        let words = o.from.split_words(&processed);
        o.to.join_words(&words, &o.prefix, &o.suffix)
    }

    /// Checks if a file matches the glob pattern
    fn matches_glob(&self, filepath: &Path, base_path: &Path) -> bool {
        let Some(ref pattern) = self.glob_pattern else {
            return true;
        };
        // Match the file name, or the path relative to the directory walked.
        filepath
            .file_name()
            .is_some_and(|name| pattern.matches(name.to_string_lossy().as_ref()))
            || filepath
                .strip_prefix(base_path)
                .is_ok_and(|rel| pattern.matches_path(rel))
    }

    /// Processes a single file
    pub fn process_file(&self, filepath: &Path, base_path: &Path) -> crate::Result<()> {
        let target = FileTarget {
            path: filepath.to_path_buf(),
            root: base_path.to_path_buf(),
            depth: 0,
        };
        crate::step::apply_one(self, &target, self.options.dry_run)?;
        Ok(())
    }

    /// Processes a directory or file.
    ///
    /// Every file is attempted; the first per-file error is returned at the end.
    pub fn process_directory(&self, directory_path: &Path) -> crate::Result<()> {
        if !directory_path.exists() {
            anyhow::bail!("path '{}' does not exist", directory_path.display());
        }
        crate::step::process_path(
            self,
            directory_path,
            self.options.recursive,
            self.options.dry_run,
        )?;
        Ok(())
    }
}

impl ContentStep for CaseConverter {
    fn name(&self) -> &'static str {
        "convert"
    }

    fn accepts(&self, file: &FileTarget) -> bool {
        crate::step::accepts_by_extension(
            file,
            &self.options.file_extensions,
            self.options.recursive,
        ) && self.matches_glob(&file.path, &file.root)
    }

    fn transform(&self, text: &str, _file: &FileTarget) -> Option<(String, usize)> {
        let mut count = 0;
        let modified = self
            .source_pattern
            .replace_all(text, |caps: &regex::Captures| {
                let converted = self.convert(&caps[0]);
                if converted != caps[0] {
                    count += 1;
                }
                converted
            });
        (modified != text).then(|| (modified.into_owned(), count))
    }

    fn describe(&self, units: usize, dry_run: bool) -> String {
        if dry_run {
            format!("Would convert {} identifier(s) in", units)
        } else {
            format!("Converted {} identifier(s) in", units)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// `--strip-prefix m_` is documented with exactly this example and used to
    /// do nothing: the candidate pattern could not match `m_userName` at all.
    #[test]
    fn test_strip_prefix_is_matchable() {
        let converter = CaseConverter::new(ConvertOptions {
            file_extensions: vec![".py".to_string()],
            recursive: false,
            strip_prefix: Some("m_".to_string()),
            ..ConvertOptions::new(CaseFormat::CamelCase, CaseFormat::SnakeCase)
        })
        .unwrap();

        // A unique directory per test: these run in parallel, and a shared

        // fixture path lets them clobber each other. TempDir also cleans up

        // when a test panics, which explicit teardown at the end does not.

        let _tmp = tempfile::tempdir().unwrap();

        let dir = _tmp.path().to_path_buf();
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.py");
        fs::write(&file, "m_userName = 1\nplainName = 2\n").unwrap();

        converter.process_file(&file, &dir).unwrap();

        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "user_name = 1\nplain_name = 2\n",
            "the prefixed identifier was not converted"
        );
    }

    #[test]
    fn test_source_pattern_without_affixes_is_unchanged() {
        assert_eq!(
            build_source_pattern(CaseFormat::CamelCase, [None, None], [None, None]),
            CaseFormat::CamelCase.pattern()
        );
    }

    #[test]
    fn test_camel_to_snake() {
        let words = CaseFormat::CamelCase.split_words("firstName");
        assert_eq!(words, vec!["first", "name"]);
        assert_eq!(
            CaseFormat::SnakeCase.join_words(&words, "", ""),
            "first_name"
        );
    }

    #[test]
    fn test_snake_to_camel() {
        let words = CaseFormat::SnakeCase.split_words("first_name");
        assert_eq!(words, vec!["first", "name"]);
        assert_eq!(
            CaseFormat::CamelCase.join_words(&words, "", ""),
            "firstName"
        );
    }

    #[test]
    fn test_pascal_to_kebab() {
        let words = CaseFormat::PascalCase.split_words("FirstName");
        assert_eq!(words, vec!["first", "name"]);
        assert_eq!(
            CaseFormat::KebabCase.join_words(&words, "", ""),
            "first-name"
        );
    }

    #[test]
    fn test_kebab_to_screaming_snake() {
        let words = CaseFormat::KebabCase.split_words("first-name");
        assert_eq!(words, vec!["first", "name"]);
        assert_eq!(
            CaseFormat::ScreamingSnakeCase.join_words(&words, "", ""),
            "FIRST_NAME"
        );
    }

    #[test]
    fn test_camel_pattern_match() {
        let pattern = Regex::new(CaseFormat::CamelCase.pattern()).unwrap();
        assert!(pattern.is_match("firstName"));
        assert!(pattern.is_match("myVariableName"));
        assert!(!pattern.is_match("firstname"));
        assert!(!pattern.is_match("FirstName")); // PascalCase, not camelCase
    }

    #[test]
    fn test_pascal_pattern_match() {
        let pattern = Regex::new(CaseFormat::PascalCase.pattern()).unwrap();
        assert!(pattern.is_match("FirstName"));
        assert!(pattern.is_match("MyVariableName"));
        assert!(!pattern.is_match("firstName")); // camelCase, not PascalCase
        assert!(!pattern.is_match("FIRSTNAME")); // Not PascalCase
    }

    #[test]
    fn test_snake_pattern_match() {
        let pattern = Regex::new(CaseFormat::SnakeCase.pattern()).unwrap();
        assert!(pattern.is_match("first_name"));
        assert!(pattern.is_match("my_variable_name"));
        assert!(!pattern.is_match("firstname"));
        assert!(!pattern.is_match("FIRST_NAME")); // SCREAMING_SNAKE_CASE
    }
}
