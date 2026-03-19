//! Case converter implementation for file processing

use crate::case::CaseFormat;
use regex::Regex;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Main converter for transforming case formats in files
pub struct CaseConverter {
    from_format: CaseFormat,
    to_format: CaseFormat,
    file_extensions: Vec<String>,
    recursive: bool,
    dry_run: bool,
    prefix: String,
    suffix: String,
    strip_prefix: Option<String>,
    strip_suffix: Option<String>,
    replace_prefix_from: Option<String>,
    replace_prefix_to: Option<String>,
    replace_suffix_from: Option<String>,
    replace_suffix_to: Option<String>,
    glob_pattern: Option<glob::Pattern>,
    word_filter: Option<Regex>,
    source_pattern: Regex,
}

impl CaseConverter {
    /// Creates a new case converter
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        from_format: CaseFormat,
        to_format: CaseFormat,
        file_extensions: Option<Vec<String>>,
        recursive: bool,
        dry_run: bool,
        prefix: String,
        suffix: String,
        strip_prefix: Option<String>,
        strip_suffix: Option<String>,
        replace_prefix_from: Option<String>,
        replace_prefix_to: Option<String>,
        replace_suffix_from: Option<String>,
        replace_suffix_to: Option<String>,
        glob_pattern: Option<String>,
        word_filter: Option<String>,
    ) -> crate::Result<Self> {
        let file_extensions = file_extensions.unwrap_or_else(|| {
            [
                ".c", ".h", ".py", ".md", ".js", ".ts", ".java", ".cpp", ".hpp",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect()
        });

        let source_pattern = Regex::new(from_format.pattern())?;
        let glob_pattern = match glob_pattern {
            Some(pattern) => Some(glob::Pattern::new(&pattern)?),
            None => None,
        };
        let word_filter = match word_filter {
            Some(pattern) => Some(Regex::new(&pattern)?),
            None => None,
        };

        Ok(CaseConverter {
            from_format,
            to_format,
            file_extensions,
            recursive,
            dry_run,
            prefix,
            suffix,
            strip_prefix,
            strip_suffix,
            replace_prefix_from,
            replace_prefix_to,
            replace_suffix_from,
            replace_suffix_to,
            glob_pattern,
            word_filter,
            source_pattern,
        })
    }

    /// Converts a single identifier
    fn convert(&self, name: &str) -> String {
        let mut processed_name = name.to_string();

        // Step 1: Strip prefix if specified
        if let Some(ref strip_pfx) = self.strip_prefix {
            if processed_name.starts_with(strip_pfx) {
                processed_name = processed_name[strip_pfx.len()..].to_string();
            }
        }

        // Step 2: Strip suffix if specified
        if let Some(ref strip_sfx) = self.strip_suffix {
            if processed_name.ends_with(strip_sfx) {
                processed_name =
                    processed_name[..processed_name.len() - strip_sfx.len()].to_string();
            }
        }

        // Step 3: Replace prefix if specified
        if let (Some(ref from_pfx), Some(ref to_pfx)) =
            (&self.replace_prefix_from, &self.replace_prefix_to)
        {
            if processed_name.starts_with(from_pfx) {
                processed_name = format!("{}{}", to_pfx, &processed_name[from_pfx.len()..]);
            }
        }

        // Step 4: Replace suffix if specified
        if let (Some(ref from_sfx), Some(ref to_sfx)) =
            (&self.replace_suffix_from, &self.replace_suffix_to)
        {
            if processed_name.ends_with(from_sfx) {
                processed_name = format!(
                    "{}{}",
                    &processed_name[..processed_name.len() - from_sfx.len()],
                    to_sfx
                );
            }
        }

        // Step 5: Apply word filter if provided
        if let Some(ref filter) = self.word_filter {
            if !filter.is_match(&processed_name) {
                return name.to_string(); // Return original if filter doesn't match
            }
        }

        // Step 6: Apply case conversion
        let words = self.from_format.split_words(&processed_name);

        // Step 7: Add prefix/suffix (existing functionality)
        self.to_format
            .join_words(&words, &self.prefix, &self.suffix)
    }

    /// Checks if a file matches the glob pattern
    fn matches_glob(&self, filepath: &Path, base_path: &Path) -> bool {
        if let Some(ref pattern) = self.glob_pattern {
            // Match against the filename
            if let Some(filename) = filepath.file_name() {
                if pattern.matches(filename.to_string_lossy().as_ref()) {
                    return true;
                }
            }

            // Also try matching against the full relative path
            if let Ok(rel_path) = filepath.strip_prefix(base_path) {
                if pattern.matches_path(rel_path) {
                    return true;
                }
            }

            false
        } else {
            true
        }
    }

    /// Processes a single file
    pub fn process_file(&self, filepath: &Path, base_path: &Path) -> crate::Result<()> {
        // Check file extension
        let extension = filepath
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e));

        if let Some(ext) = extension {
            if !self.file_extensions.contains(&ext) {
                return Ok(());
            }
        } else {
            return Ok(());
        }

        // Check glob pattern
        if !self.matches_glob(filepath, base_path) {
            return Ok(());
        }

        // Read file content
        let content = fs::read_to_string(filepath)?;

        // Replace all matches of the source pattern
        let modified_content = self
            .source_pattern
            .replace_all(&content, |caps: &regex::Captures| self.convert(&caps[0]));

        if content != modified_content {
            if self.dry_run {
                println!("Would convert '{}'", filepath.display());
            } else {
                fs::write(filepath, modified_content.as_ref())?;
                println!("Converted '{}'", filepath.display());
            }
        } else if !self.dry_run {
            println!("No changes needed in '{}'", filepath.display());
        }

        Ok(())
    }

    /// Processes a directory or file
    pub fn process_directory(&self, directory_path: &Path) -> crate::Result<()> {
        if !directory_path.exists() {
            eprintln!("Path '{}' does not exist.", directory_path.display());
            return Ok(());
        }

        // If it's a single file, process it directly
        if directory_path.is_file() {
            if let Some(parent) = directory_path.parent() {
                self.process_file(directory_path, parent)?;
            } else {
                self.process_file(directory_path, Path::new("."))?;
            }
            return Ok(());
        }

        // Otherwise, process directory
        if !directory_path.is_dir() {
            eprintln!(
                "Path '{}' is not a directory or file.",
                directory_path.display()
            );
            return Ok(());
        }

        if self.recursive {
            for entry in WalkDir::new(directory_path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    if let Err(e) = self.process_file(entry.path(), directory_path) {
                        eprintln!("Error processing file '{}': {}", entry.path().display(), e);
                    }
                }
            }
        } else {
            for entry in fs::read_dir(directory_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Err(e) = self.process_file(&path, directory_path) {
                        eprintln!("Error processing file '{}': {}", path.display(), e);
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
