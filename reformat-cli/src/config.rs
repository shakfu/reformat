//! Configuration file loading for reformat presets.
//!
//! Looks for `reformat.json` in the current working directory.

use std::fs;
use std::path::Path;

use reformat_core::config::{validate_steps, ReformatConfig};
use reformat_core::Preset;

pub const CONFIG_FILENAME: &str = "reformat.json";

/// Load and parse `reformat.json` from the given directory.
/// Returns `None` if the file does not exist.
pub fn load_config_from(dir: &Path) -> anyhow::Result<Option<ReformatConfig>> {
    let path = dir.join(CONFIG_FILENAME);
    if !path.is_file() {
        return Ok(None);
    }
    log::debug!("Loading config from: {}", path.display());
    let content = fs::read_to_string(&path)?;
    let config: ReformatConfig = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {}", CONFIG_FILENAME, e))?;
    Ok(Some(config))
}

/// Load and parse `reformat.json` from the current working directory.
/// Returns `None` if the file does not exist.
pub fn load_config() -> anyhow::Result<Option<ReformatConfig>> {
    let cwd = std::env::current_dir()?;
    load_config_from(&cwd)
}

/// Look up a preset by name in the loaded config.
pub fn get_preset<'a>(config: &'a ReformatConfig, name: &str) -> anyhow::Result<&'a Preset> {
    let preset = config.get(name).ok_or_else(|| {
        let available: Vec<&str> = config.keys().map(|k| k.as_str()).collect();
        anyhow::anyhow!(
            "preset '{}' not found in {}. Available presets: {}",
            name,
            CONFIG_FILENAME,
            if available.is_empty() {
                "(none)".to_string()
            } else {
                available.join(", ")
            }
        )
    })?;
    validate_steps(name, &preset.steps)?;
    Ok(preset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config_file_not_found() {
        let tmp = std::env::temp_dir().join("reformat_cfg_missing");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let result = load_config_from(&tmp).unwrap();
        assert!(result.is_none());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_config_valid() {
        let tmp = std::env::temp_dir().join("reformat_cfg_valid");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        fs::write(
            tmp.join("reformat.json"),
            r#"{"mypreset": {"steps": ["clean"]}}"#,
        )
        .unwrap();

        let config = load_config_from(&tmp).unwrap().unwrap();
        assert!(config.contains_key("mypreset"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_config_malformed_json() {
        let tmp = std::env::temp_dir().join("reformat_cfg_malformed");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        fs::write(tmp.join("reformat.json"), "not valid json {{{").unwrap();

        let result = load_config_from(&tmp);
        assert!(result.is_err());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_get_preset_found() {
        let json = r#"{"code": {"steps": ["rename", "clean"]}}"#;
        let config: ReformatConfig = serde_json::from_str(json).unwrap();
        let preset = get_preset(&config, "code").unwrap();
        assert_eq!(preset.steps, vec!["rename", "clean"]);
    }

    #[test]
    fn test_get_preset_not_found() {
        let json = r#"{"code": {"steps": ["clean"]}}"#;
        let config: ReformatConfig = serde_json::from_str(json).unwrap();
        let err = get_preset(&config, "missing").unwrap_err();
        assert!(err.to_string().contains("preset 'missing' not found"));
        assert!(err.to_string().contains("code"));
    }

    #[test]
    fn test_get_preset_invalid_step() {
        let json = r#"{"bad": {"steps": ["clean", "nope"]}}"#;
        let config: ReformatConfig = serde_json::from_str(json).unwrap();
        let err = get_preset(&config, "bad").unwrap_err();
        assert!(err.to_string().contains("unknown step 'nope'"));
    }
}
