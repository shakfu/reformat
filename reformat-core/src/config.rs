//! Configuration file support for reformat presets
//!
//! Presets are named transformation pipelines defined in `reformat.json`.
//! Each preset specifies an ordered list of steps and per-step settings.

use std::collections::HashMap;

use serde::Deserialize;

use crate::case::CaseFormat;
use crate::rename::{CaseTransform, SpaceReplace};

/// Root configuration: a map of preset names to preset definitions.
pub type ReformatConfig = HashMap<String, Preset>;

/// A named preset defining an ordered list of transformation steps.
#[derive(Debug, Clone, Deserialize)]
pub struct Preset {
    /// Ordered list of step names to execute.
    /// Valid values: "rename", "emojis", "clean", "convert", "group"
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
}

/// Valid step names for presets.
pub const VALID_STEPS: &[&str] = &[
    "rename", "emojis", "clean", "convert", "group", "endings", "indent", "replace", "header",
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
pub struct RenameConfig {
    pub case_transform: Option<String>,
    pub space_replace: Option<String>,
    pub recursive: Option<bool>,
    pub include_symlinks: Option<bool>,
}

impl RenameConfig {
    pub fn parse_case_transform(&self) -> Option<CaseTransform> {
        self.case_transform.as_deref().map(|s| match s {
            "lowercase" => CaseTransform::Lowercase,
            "uppercase" => CaseTransform::Uppercase,
            "capitalize" => CaseTransform::Capitalize,
            _ => CaseTransform::None,
        })
    }

    pub fn parse_space_replace(&self) -> Option<SpaceReplace> {
        self.space_replace.as_deref().map(|s| match s {
            "underscore" => SpaceReplace::Underscore,
            "hyphen" => SpaceReplace::Hyphen,
            _ => SpaceReplace::None,
        })
    }
}

/// Configuration for the emojis step.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EmojiConfig {
    pub replace_task_emojis: Option<bool>,
    pub remove_other_emojis: Option<bool>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

/// Configuration for the clean step.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CleanConfig {
    pub remove_trailing: Option<bool>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

/// Configuration for the convert step.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConvertConfig {
    pub from_format: Option<String>,
    pub to_format: Option<String>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub glob: Option<String>,
    pub word_filter: Option<String>,
}

impl ConvertConfig {
    pub fn parse_from_format(&self) -> Option<CaseFormat> {
        self.from_format.as_deref().and_then(parse_case_format)
    }

    pub fn parse_to_format(&self) -> Option<CaseFormat> {
        self.to_format.as_deref().and_then(parse_case_format)
    }
}

/// Configuration for the group step.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GroupConfig {
    pub separator: Option<String>,
    pub min_count: Option<usize>,
    pub strip_prefix: Option<bool>,
    pub from_suffix: Option<bool>,
    pub recursive: Option<bool>,
}

/// Configuration for the endings step.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EndingsConfig {
    pub style: Option<String>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

/// Configuration for the indent step.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct IndentConfig {
    pub style: Option<String>,
    pub width: Option<usize>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

/// A single replace pattern in config.
#[derive(Debug, Clone, Deserialize)]
pub struct ReplacePatternEntry {
    pub find: String,
    pub replace: String,
}

/// Configuration for the replace step.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReplaceConfig {
    pub patterns: Option<Vec<ReplacePatternEntry>>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

/// Configuration for the header step.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct HeaderConfig {
    pub text: Option<String>,
    pub update_year: Option<bool>,
    pub file_extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
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
            rename.parse_case_transform(),
            Some(CaseTransform::Lowercase)
        );
        assert_eq!(rename.parse_space_replace(), Some(SpaceReplace::Hyphen));

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

    #[test]
    fn test_unknown_fields_ignored() {
        let json = r#"{
            "test": {
                "steps": ["clean"],
                "clean": {
                    "remove_trailing": true,
                    "some_future_field": 42
                }
            }
        }"#;

        // serde default behavior: unknown fields cause an error unless denied
        // We want to test current behavior
        let result: Result<ReformatConfig, _> = serde_json::from_str(json);
        // By default serde_json ignores unknown fields
        assert!(result.is_ok());
    }
}
