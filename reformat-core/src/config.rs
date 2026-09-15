//! Configuration file support for reformat presets
//!
//! Presets are named transformation pipelines defined in `reformat.json`.
//! Each preset specifies an ordered list of steps and per-step settings.

use std::collections::HashMap;

use serde::Deserialize;

use crate::case::CaseFormat;
use crate::converter::{CaseConverter, ConvertOptions};
use crate::emoji::EmojiOptions;
use crate::endings::{EndingsOptions, LineEnding};
use crate::group::GroupOptions;
use crate::header::HeaderOptions;
use crate::indent::{IndentOptions, IndentStyle};
use crate::rename::{CaseTransform, RenameOptions, SpaceReplace, TimestampFormat};
use crate::replace::{ReplaceOptions, ReplacePattern};
use crate::whitespace::WhitespaceOptions;

/// Root configuration: a map of preset names to preset definitions.
pub type ReformatConfig = HashMap<String, Preset>;

/// A named preset defining an ordered list of transformation steps.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    /// Ordered list of step names to execute.
    /// Valid values: see [`VALID_STEPS`].
    pub steps: Vec<String>,
    #[serde(default)]
    pub rename: Option<RenameConfig>,
    #[serde(default)]
    pub emojis: Option<EmojiConfig>,
    #[serde(default)]
    pub clean: Option<CleanConfig>,
    #[serde(default)]
    pub convert: Option<ConvertConfig>,
    #[serde(default)]
    pub group: Option<GroupConfig>,
    #[serde(default)]
    pub endings: Option<EndingsConfig>,
    #[serde(default)]
    pub indent: Option<IndentConfig>,
    #[serde(default)]
    pub replace: Option<ReplaceConfig>,
    #[serde(default)]
    pub header: Option<HeaderConfig>,
    #[serde(default)]
    pub editorconfig: Option<EditorConfigConfig>,
}

/// Valid step names for presets.
pub const VALID_STEPS: &[&str] = &[
    "rename",
    "emojis",
    "clean",
    "convert",
    "group",
    "endings",
    "indent",
    "replace",
    "header",
    "editorconfig",
];

/// Validate that all steps in a preset are recognized.
pub fn validate_steps(preset_name: &str, steps: &[String]) -> crate::Result<()> {
    for step in steps {
        if !VALID_STEPS.contains(&step.as_str()) {
            anyhow::bail!(
                "preset '{}': unknown step '{}'. Valid steps: {}",
                preset_name,
                step,
                VALID_STEPS.join(", ")
            );
        }
    }
    Ok(())
}

/// Configuration for the rename step.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameConfig {
    pub case_transform: Option<String>,
    pub space_replace: Option<String>,
    pub recursive: Option<bool>,
    pub include_symlinks: Option<bool>,
    pub add_prefix: Option<String>,
    pub remove_prefix: Option<String>,
    pub add_suffix: Option<String>,
    pub remove_suffix: Option<String>,
    /// Two entries: the prefix to match and its replacement.
    pub replace_prefix: Option<Vec<String>>,
    /// Two entries: the suffix to match and its replacement.
    pub replace_suffix: Option<Vec<String>>,
    /// "long" (YYYYMMDD) or "short" (YYMMDD).
    pub timestamp: Option<String>,
    /// Only rename files with these extensions. Applied by the caller that
    /// selects files; `RenameOptions` has no extension filter.
    pub file_extensions: Option<Vec<String>>,
}

impl RenameConfig {
    /// Parses `case_transform`, rejecting unrecognised values.
    ///
    /// An unknown value used to fall through to `CaseTransform::None`, so a
    /// typo such as `"lowercse"` silently disabled the transform with no
    /// diagnostic at all.
    pub fn parse_case_transform(&self) -> crate::Result<Option<CaseTransform>> {
        match self.case_transform.as_deref() {
            None => Ok(None),
            Some("lowercase") => Ok(Some(CaseTransform::Lowercase)),
            Some("uppercase") => Ok(Some(CaseTransform::Uppercase)),
            Some("capitalize") => Ok(Some(CaseTransform::Capitalize)),
            Some("none") => Ok(Some(CaseTransform::None)),
            Some(other) => anyhow::bail!(
                "unknown rename.case_transform '{}'. Valid values: lowercase, uppercase, capitalize, none",
                other
            ),
        }
    }

    /// Parses `space_replace`, rejecting unrecognised values.
    pub fn parse_space_replace(&self) -> crate::Result<Option<SpaceReplace>> {
        match self.space_replace.as_deref() {
            None => Ok(None),
            Some("underscore") => Ok(Some(SpaceReplace::Underscore)),
            Some("hyphen") => Ok(Some(SpaceReplace::Hyphen)),
            Some("none") => Ok(Some(SpaceReplace::None)),
            Some(other) => anyhow::bail!(
                "unknown rename.space_replace '{}'. Valid values: underscore, hyphen, none",
                other
            ),
        }
    }
}

/// Configuration for the emojis step.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmojiConfig {
    pub replace_task_emojis: Option<bool>,
    pub remove_other_emojis: Option<bool>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

/// Configuration for the clean step.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanConfig {
    pub remove_trailing: Option<bool>,
    pub insert_final_newline: Option<bool>,
    pub trim_trailing_blank_lines: Option<bool>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

/// Configuration for the convert step.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConvertConfig {
    pub from_format: Option<String>,
    pub to_format: Option<String>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub glob: Option<String>,
    pub word_filter: Option<String>,
    pub strip_prefix: Option<String>,
    pub strip_suffix: Option<String>,
    pub replace_prefix_from: Option<String>,
    pub replace_prefix_to: Option<String>,
    pub replace_suffix_from: Option<String>,
    pub replace_suffix_to: Option<String>,
}

/// Configuration for the group step.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupConfig {
    pub separator: Option<String>,
    pub min_count: Option<usize>,
    pub strip_prefix: Option<bool>,
    pub from_suffix: Option<bool>,
    pub recursive: Option<bool>,
}

/// Configuration for the endings step.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndingsConfig {
    pub style: Option<String>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

/// Configuration for the indent step.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndentConfig {
    pub style: Option<String>,
    pub width: Option<usize>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

/// A single replace pattern in config.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplacePatternEntry {
    pub find: String,
    pub replace: String,
    /// Match `find` as plain text and insert `replace` without `$` expansion.
    #[serde(default)]
    pub literal: bool,
    /// Match regardless of case.
    #[serde(default)]
    pub ignore_case: bool,
}

impl ReplacePatternEntry {
    /// The regex pattern this entry describes, with `literal` and
    /// `ignore_case` folded into `find` and `replace`.
    pub fn to_pattern(&self) -> ReplacePattern {
        let (mut find, replace) = if self.literal {
            (regex::escape(&self.find), self.replace.replace('$', "$$"))
        } else {
            (self.find.clone(), self.replace.clone())
        };
        if self.ignore_case {
            find = format!("(?i){}", find);
        }
        ReplacePattern { find, replace }
    }
}

/// Configuration for the replace step.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplaceConfig {
    pub patterns: Option<Vec<ReplacePatternEntry>>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

/// Configuration for the header step.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderConfig {
    pub text: Option<String>,
    pub update_year: Option<bool>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

/// Configuration for the editorconfig step.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorConfigConfig {
    /// Also rewrite indentation from `indent_style` and `tab_width`.
    pub indent: Option<bool>,
    pub recursive: Option<bool>,
}

/// Converts a two-element `[from, to]` config entry into a pair.
fn pair(values: &Option<Vec<String>>, field: &str) -> crate::Result<Option<(String, String)>> {
    match values {
        None => Ok(None),
        Some(v) if v.len() == 2 => Ok(Some((v[0].clone(), v[1].clone()))),
        Some(v) => anyhow::bail!("{} expects exactly 2 values, got {}", field, v.len()),
    }
}

impl RenameConfig {
    /// Builds the renamer options this config describes.
    ///
    /// This is the single place option assembly happens. The CLI builds a
    /// `RenameConfig` from its flags and calls this, and so does the preset
    /// runner -- previously each had its own copy, and they drifted.
    pub fn to_options(&self, dry_run: bool) -> crate::Result<RenameOptions> {
        let mut options = RenameOptions {
            dry_run,
            ..Default::default()
        };
        if let Some(v) = self.parse_case_transform()? {
            options.case_transform = v;
        }
        if let Some(v) = self.parse_space_replace()? {
            options.space_replace = v;
        }
        if let Some(v) = self.recursive {
            options.recursive = v;
        }
        if let Some(v) = self.include_symlinks {
            options.include_symlinks = v;
        }
        options.add_prefix = self.add_prefix.clone();
        options.remove_prefix = self.remove_prefix.clone();
        options.add_suffix = self.add_suffix.clone();
        options.remove_suffix = self.remove_suffix.clone();
        options.replace_prefix = pair(&self.replace_prefix, "rename.replace_prefix")?;
        options.replace_suffix = pair(&self.replace_suffix, "rename.replace_suffix")?;
        options.timestamp_format = match self.timestamp.as_deref() {
            None => TimestampFormat::None,
            Some("long") => TimestampFormat::Long,
            Some("short") => TimestampFormat::Short,
            Some("none") => TimestampFormat::None,
            Some(other) => anyhow::bail!(
                "unknown rename.timestamp '{}'. Valid values: long, short, none",
                other
            ),
        };
        Ok(options)
    }
}

impl EmojiConfig {
    /// Builds the emoji transformer options this config describes.
    pub fn to_options(&self, dry_run: bool) -> EmojiOptions {
        let mut options = EmojiOptions {
            dry_run,
            ..Default::default()
        };
        if let Some(v) = self.replace_task_emojis {
            options.replace_task_emojis = v;
        }
        if let Some(v) = self.remove_other_emojis {
            options.remove_other_emojis = v;
        }
        if let Some(ref v) = self.file_extensions {
            options.file_extensions = v.clone();
        }
        if let Some(v) = self.recursive {
            options.recursive = v;
        }
        options
    }
}

impl CleanConfig {
    /// Builds the whitespace cleaner options this config describes.
    pub fn to_options(&self, dry_run: bool) -> WhitespaceOptions {
        let mut options = WhitespaceOptions {
            dry_run,
            ..Default::default()
        };
        if let Some(v) = self.remove_trailing {
            options.remove_trailing = v;
        }
        if let Some(v) = self.insert_final_newline {
            options.insert_final_newline = v;
        }
        if let Some(v) = self.trim_trailing_blank_lines {
            options.trim_trailing_blank_lines = v;
        }
        if let Some(ref v) = self.file_extensions {
            options.file_extensions = v.clone();
        }
        if let Some(v) = self.recursive {
            options.recursive = v;
        }
        options
    }
}

impl ConvertConfig {
    pub fn parse_from_format(&self) -> Option<CaseFormat> {
        self.from_format.as_deref().and_then(parse_case_format)
    }

    pub fn parse_to_format(&self) -> Option<CaseFormat> {
        self.to_format.as_deref().and_then(parse_case_format)
    }

    /// Builds the case converter options this config describes.
    pub fn to_options(&self, dry_run: bool) -> crate::Result<ConvertOptions> {
        let from = self
            .parse_from_format()
            .ok_or_else(|| anyhow::anyhow!("convert.from_format is missing or invalid"))?;
        let to = self
            .parse_to_format()
            .ok_or_else(|| anyhow::anyhow!("convert.to_format is missing or invalid"))?;

        let defaults = ConvertOptions::new(from, to);
        Ok(ConvertOptions {
            file_extensions: self
                .file_extensions
                .clone()
                .unwrap_or(defaults.file_extensions.clone()),
            recursive: self.recursive.unwrap_or(true),
            dry_run,
            prefix: self.prefix.clone().unwrap_or_default(),
            suffix: self.suffix.clone().unwrap_or_default(),
            strip_prefix: self.strip_prefix.clone(),
            strip_suffix: self.strip_suffix.clone(),
            replace_prefix: both(
                &self.replace_prefix_from,
                &self.replace_prefix_to,
                "convert.replace_prefix",
            )?,
            replace_suffix: both(
                &self.replace_suffix_from,
                &self.replace_suffix_to,
                "convert.replace_suffix",
            )?,
            glob: self.glob.clone(),
            word_filter: self.word_filter.clone(),
            ..defaults
        })
    }

    /// Builds the case converter this config describes.
    pub fn to_converter(&self, dry_run: bool) -> crate::Result<CaseConverter> {
        CaseConverter::new(self.to_options(dry_run)?)
    }
}

/// Pairs `{field}_from` and `{field}_to`, which must be given together.
fn both(
    from: &Option<String>,
    to: &Option<String>,
    field: &str,
) -> crate::Result<Option<(String, String)>> {
    match (from, to) {
        (None, None) => Ok(None),
        (Some(f), Some(t)) => Ok(Some((f.clone(), t.clone()))),
        _ => anyhow::bail!("{}_from and {}_to must be given together", field, field),
    }
}

impl GroupConfig {
    /// Builds the file grouper options this config describes.
    pub fn to_options(&self, dry_run: bool) -> crate::Result<GroupOptions> {
        let mut options = GroupOptions {
            dry_run,
            ..Default::default()
        };
        if let Some(ref v) = self.separator {
            let mut chars = v.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => options.separator = c,
                _ => anyhow::bail!("group.separator must be a single character, got '{}'", v),
            }
        }
        if let Some(v) = self.min_count {
            options.min_count = v;
        }
        if let Some(v) = self.strip_prefix {
            options.strip_prefix = v;
        }
        if let Some(v) = self.from_suffix {
            options.from_suffix = v;
            // Splitting at the last separator only makes sense together with
            // stripping the prefix that precedes it.
            if v {
                options.strip_prefix = true;
            }
        }
        if let Some(v) = self.recursive {
            options.recursive = v;
        }
        Ok(options)
    }
}

impl EndingsConfig {
    /// Builds the line ending normalizer options this config describes.
    pub fn to_options(&self, dry_run: bool) -> crate::Result<EndingsOptions> {
        let mut options = EndingsOptions {
            dry_run,
            ..Default::default()
        };
        if let Some(ref v) = self.style {
            options.style = LineEnding::parse(v).ok_or_else(|| {
                anyhow::anyhow!("unknown endings.style '{}'. Valid values: lf, crlf, cr", v)
            })?;
        }
        if let Some(ref v) = self.file_extensions {
            options.file_extensions = v.clone();
        }
        if let Some(v) = self.recursive {
            options.recursive = v;
        }
        Ok(options)
    }
}

impl IndentConfig {
    /// Builds the indentation normalizer options this config describes.
    pub fn to_options(&self, dry_run: bool) -> crate::Result<IndentOptions> {
        let mut options = IndentOptions {
            dry_run,
            ..Default::default()
        };
        if let Some(ref v) = self.style {
            options.style = IndentStyle::parse(v).ok_or_else(|| {
                anyhow::anyhow!("unknown indent.style '{}'. Valid values: spaces, tabs", v)
            })?;
        }
        if let Some(v) = self.width {
            if v == 0 {
                anyhow::bail!("indent.width must be greater than 0");
            }
            options.width = v;
        }
        if let Some(ref v) = self.file_extensions {
            options.file_extensions = v.clone();
        }
        if let Some(v) = self.recursive {
            options.recursive = v;
        }
        Ok(options)
    }
}

impl ReplaceConfig {
    /// Builds the content replacer options this config describes.
    pub fn to_options(&self, dry_run: bool) -> crate::Result<ReplaceOptions> {
        let patterns = self
            .patterns
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("replace.patterns is missing"))?
            .iter()
            .map(ReplacePatternEntry::to_pattern)
            .collect();

        let mut options = ReplaceOptions {
            patterns,
            dry_run,
            ..Default::default()
        };
        if let Some(ref v) = self.file_extensions {
            options.file_extensions = v.clone();
        }
        if let Some(v) = self.recursive {
            options.recursive = v;
        }
        Ok(options)
    }
}

impl HeaderConfig {
    /// Builds the header manager options this config describes.
    pub fn to_options(&self, dry_run: bool) -> crate::Result<HeaderOptions> {
        let text = self
            .text
            .clone()
            .ok_or_else(|| anyhow::anyhow!("header.text is missing"))?;

        let mut options = HeaderOptions {
            text,
            dry_run,
            ..Default::default()
        };
        if let Some(v) = self.update_year {
            options.update_year = v;
        }
        if let Some(ref v) = self.file_extensions {
            options.file_extensions = v.clone();
        }
        if let Some(v) = self.recursive {
            options.recursive = v;
        }
        Ok(options)
    }
}

fn parse_case_format(s: &str) -> Option<CaseFormat> {
    match s {
        "camel" | "camelCase" => Some(CaseFormat::CamelCase),
        "pascal" | "PascalCase" => Some(CaseFormat::PascalCase),
        "snake" | "snake_case" => Some(CaseFormat::SnakeCase),
        "screaming_snake" | "SCREAMING_SNAKE_CASE" => Some(CaseFormat::ScreamingSnakeCase),
        "kebab" | "kebab-case" => Some(CaseFormat::KebabCase),
        "screaming_kebab" | "SCREAMING-KEBAB-CASE" => Some(CaseFormat::ScreamingKebabCase),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_full_config() {
        let json = r#"{
            "code": {
                "steps": ["rename", "emojis", "clean"],
                "rename": {
                    "case_transform": "lowercase",
                    "space_replace": "hyphen",
                    "recursive": true,
                    "include_symlinks": false
                },
                "emojis": {
                    "replace_task_emojis": true,
                    "remove_other_emojis": false,
                    "file_extensions": [".md", ".txt"]
                },
                "clean": {
                    "remove_trailing": true,
                    "file_extensions": [".rs", ".py"]
                }
            },
            "templates": {
                "steps": ["group", "clean"],
                "group": {
                    "separator": "_",
                    "min_count": 3,
                    "strip_prefix": true,
                    "from_suffix": false
                }
            }
        }"#;

        let config: ReformatConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.len(), 2);

        let code = &config["code"];
        assert_eq!(code.steps, vec!["rename", "emojis", "clean"]);

        let rename = code.rename.as_ref().unwrap();
        assert_eq!(rename.case_transform.as_deref(), Some("lowercase"));
        assert_eq!(
            rename.parse_case_transform().unwrap(),
            Some(CaseTransform::Lowercase)
        );
        assert_eq!(
            rename.parse_space_replace().unwrap(),
            Some(SpaceReplace::Hyphen)
        );

        let emojis = code.emojis.as_ref().unwrap();
        assert_eq!(emojis.replace_task_emojis, Some(true));
        assert_eq!(emojis.remove_other_emojis, Some(false));
        assert_eq!(
            emojis.file_extensions.as_ref().unwrap(),
            &vec![".md".to_string(), ".txt".to_string()]
        );

        let templates = &config["templates"];
        assert_eq!(templates.steps, vec!["group", "clean"]);
        let group = templates.group.as_ref().unwrap();
        assert_eq!(group.min_count, Some(3));
        assert_eq!(group.strip_prefix, Some(true));
    }

    #[test]
    fn test_deserialize_minimal_config() {
        let json = r#"{
            "quick": {
                "steps": ["clean"]
            }
        }"#;

        let config: ReformatConfig = serde_json::from_str(json).unwrap();
        let quick = &config["quick"];
        assert_eq!(quick.steps, vec!["clean"]);
        assert!(quick.rename.is_none());
        assert!(quick.emojis.is_none());
        assert!(quick.clean.is_none());
        assert!(quick.convert.is_none());
        assert!(quick.group.is_none());
    }

    #[test]
    fn test_deserialize_convert_config() {
        let json = r#"{
            "case-fix": {
                "steps": ["convert"],
                "convert": {
                    "from_format": "camel",
                    "to_format": "snake",
                    "file_extensions": [".py"],
                    "recursive": true,
                    "prefix": "pre_",
                    "suffix": "_suf"
                }
            }
        }"#;

        let config: ReformatConfig = serde_json::from_str(json).unwrap();
        let preset = &config["case-fix"];
        let convert = preset.convert.as_ref().unwrap();
        assert_eq!(convert.parse_from_format(), Some(CaseFormat::CamelCase));
        assert_eq!(convert.parse_to_format(), Some(CaseFormat::SnakeCase));
        assert_eq!(convert.prefix.as_deref(), Some("pre_"));
        assert_eq!(convert.suffix.as_deref(), Some("_suf"));
    }

    #[test]
    fn test_replace_prefix_halves_must_be_paired() {
        let json = r#"{"p": {"steps": ["convert"], "convert": {
            "from_format": "camel", "to_format": "snake", "replace_prefix_from": "I"}}}"#;
        let config: ReformatConfig = serde_json::from_str(json).unwrap();
        let err = config["p"]
            .convert
            .as_ref()
            .unwrap()
            .to_options(false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("replace_prefix_to"), "got: {}", err);
    }

    #[test]
    fn test_literal_and_ignore_case_patterns() {
        let entry = |find: &str, replace: &str, literal, ignore_case| ReplacePatternEntry {
            find: find.to_string(),
            replace: replace.to_string(),
            literal,
            ignore_case,
        };
        let apply = |e: ReplacePatternEntry, text: &str| {
            let p = e.to_pattern();
            regex::Regex::new(&p.find)
                .unwrap()
                .replace_all(text, p.replace.as_str())
                .into_owned()
        };

        assert_eq!(
            apply(entry("a.b(", "$1 x", true, false), "a.b( axb("),
            "$1 x axb("
        );
        assert_eq!(
            apply(entry("todo", "DONE", false, true), "TODO todo"),
            "DONE DONE"
        );
        assert_eq!(apply(entry("f(", "g(", true, true), "F(1)"), "g(1)");
    }

    #[test]
    fn test_unknown_enum_value_is_rejected() {
        let json = r#"{"p": {"steps": ["rename"], "rename": {"case_transform": "lowercse"}}}"#;
        let config: ReformatConfig = serde_json::from_str(json).unwrap();
        let err = config["p"]
            .rename
            .as_ref()
            .unwrap()
            .parse_case_transform()
            .unwrap_err()
            .to_string();
        assert!(err.contains("lowercse"), "got: {}", err);
        assert!(err.contains("Valid values"), "got: {}", err);
    }

    #[test]
    fn test_validate_steps_valid() {
        let steps = vec![
            "rename".to_string(),
            "emojis".to_string(),
            "clean".to_string(),
        ];
        assert!(validate_steps("test", &steps).is_ok());
    }

    #[test]
    fn test_validate_steps_invalid() {
        let steps = vec!["rename".to_string(), "bogus".to_string()];
        let err = validate_steps("test", &steps).unwrap_err();
        assert!(err.to_string().contains("unknown step 'bogus'"));
    }

    #[test]
    fn test_parse_case_format() {
        assert_eq!(parse_case_format("camel"), Some(CaseFormat::CamelCase));
        assert_eq!(parse_case_format("camelCase"), Some(CaseFormat::CamelCase));
        assert_eq!(parse_case_format("pascal"), Some(CaseFormat::PascalCase));
        assert_eq!(parse_case_format("snake"), Some(CaseFormat::SnakeCase));
        assert_eq!(
            parse_case_format("screaming_snake"),
            Some(CaseFormat::ScreamingSnakeCase)
        );
        assert_eq!(parse_case_format("kebab"), Some(CaseFormat::KebabCase));
        assert_eq!(
            parse_case_format("screaming_kebab"),
            Some(CaseFormat::ScreamingKebabCase)
        );
        assert_eq!(parse_case_format("unknown"), None);
    }

    /// A misspelled key must be reported, not silently dropped: the user
    /// believes they configured something that never took effect.
    #[test]
    fn test_unknown_fields_are_rejected() {
        let json = r#"{
            "test": {
                "steps": ["clean"],
                "clean": {
                    "remove_trailing": true,
                    "some_future_field": 42
                }
            }
        }"#;

        let result: Result<ReformatConfig, _> = serde_json::from_str(json);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("some_future_field"),
            "the unknown key should be named in the error, got: {}",
            err
        );
    }
}
