//! Configuration file loading for reformat presets.
//!
//! `reformat.json` is found with `--config`, or by searching the current
//! directory, the target paths, and their ancestors.

use std::fs;
use std::path::{Path, PathBuf};

use reformat_core::config::{validate_steps, ReformatConfig};
use reformat_core::Preset;

pub const CONFIG_FILENAME: &str = "reformat.json";

/// Load and parse a config file at `path`.
pub fn load_config_file(path: &Path) -> anyhow::Result<ReformatConfig> {
    log::debug!("Loading config from: {}", path.display());
    let content = fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path.display(), e))?;
    serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {}", path.display(), e))
}

/// Load and parse `reformat.json` from the given directory.
/// Returns `None` if the file does not exist.
pub fn load_config_from(dir: &Path) -> anyhow::Result<Option<ReformatConfig>> {
    let path = dir.join(CONFIG_FILENAME);
    if !path.is_file() {
        return Ok(None);
    }
    load_config_file(&path).map(Some)
}

/// Finds the config to use, returning its path and contents.
///
/// `explicit` (from `--config`) wins. Otherwise the current directory and its
/// ancestors are searched, then each of `paths` and its ancestors. The
/// current directory comes first so that existing invocations keep the
/// config they found before target paths were searched.
pub fn find_config(
    explicit: Option<&Path>,
    paths: &[PathBuf],
) -> anyhow::Result<Option<(PathBuf, ReformatConfig)>> {
    find_config_in(explicit, &std::env::current_dir()?, paths)
}

fn find_config_in(
    explicit: Option<&Path>,
    cwd: &Path,
    paths: &[PathBuf],
) -> anyhow::Result<Option<(PathBuf, ReformatConfig)>> {
    if let Some(path) = explicit {
        return Ok(Some((path.to_path_buf(), load_config_file(path)?)));
    }

    let mut starts = vec![cwd.to_path_buf()];
    for path in paths {
        let absolute = cwd.join(path);
        starts.push(if absolute.is_dir() {
            absolute
        } else {
            absolute.parent().map(Path::to_path_buf).unwrap_or(absolute)
        });
    }

    for start in &starts {
        for dir in start.ancestors() {
            if let Some(config) = load_config_from(dir)? {
                return Ok(Some((dir.join(CONFIG_FILENAME), config)));
            }
        }
    }
    Ok(None)
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
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let tmp = _tmp.path().to_path_buf();
        fs::create_dir_all(&tmp).unwrap();

        let result = load_config_from(&tmp).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_load_config_valid() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let tmp = _tmp.path().to_path_buf();
        fs::create_dir_all(&tmp).unwrap();

        fs::write(
            tmp.join("reformat.json"),
            r#"{"mypreset": {"steps": ["clean"]}}"#,
        )
        .unwrap();

        let config = load_config_from(&tmp).unwrap().unwrap();
        assert!(config.contains_key("mypreset"));
    }

    #[test]
    fn test_load_config_malformed_json() {
        // A unique directory per test: these run in parallel, and a shared
        // fixture path lets them clobber each other. TempDir also cleans up
        // when a test panics, which explicit teardown at the end does not.
        let _tmp = tempfile::tempdir().unwrap();
        let tmp = _tmp.path().to_path_buf();
        fs::create_dir_all(&tmp).unwrap();

        fs::write(tmp.join("reformat.json"), "not valid json {{{").unwrap();

        let result = load_config_from(&tmp);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_searches_ancestors() {
        let tmp = tempfile::Builder::new()
            .prefix("reformat-cfg-")
            .tempdir()
            .unwrap();
        let root = tmp.path();
        let nested = root.join("src").join("deep");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            root.join("reformat.json"),
            r#"{"mypreset": {"steps": ["clean"]}}"#,
        )
        .unwrap();

        // Found from the root itself...
        assert!(load_config_from(root).unwrap().is_some());
        // ...but not by a directory-only lookup further down.
        assert!(load_config_from(&nested).unwrap().is_none());

        // The ancestor walk finds it from the nested directory.
        let found = nested
            .ancestors()
            .find_map(|d| load_config_from(d).ok().flatten());
        assert!(
            found.is_some(),
            "a preset at the project root should be visible from a subdirectory"
        );
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

    #[test]
    fn test_find_config_searches_target_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("project");
        let nested = project.join("src");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            project.join("reformat.json"),
            r#"{"p": {"steps": ["clean"]}}"#,
        )
        .unwrap();

        let elsewhere = tmp.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();

        let (path, config) = find_config_in(None, &elsewhere, &[nested.join("a.rs")])
            .unwrap()
            .unwrap();
        assert_eq!(path, project.join("reformat.json"));
        assert!(config.contains_key("p"));

        // The working directory's config comes first.
        fs::write(
            elsewhere.join("reformat.json"),
            r#"{"q": {"steps": ["clean"]}}"#,
        )
        .unwrap();
        let (path, _) = find_config_in(None, &elsewhere, &[nested])
            .unwrap()
            .unwrap();
        assert_eq!(path, elsewhere.join("reformat.json"));
    }

    #[test]
    fn test_explicit_config_wins_and_must_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("custom.json");
        fs::write(&file, r#"{"mine": {"steps": ["clean"]}}"#).unwrap();

        let (path, config) = find_config(Some(&file), &[]).unwrap().unwrap();
        assert_eq!(path, file);
        assert!(config.contains_key("mine"));

        let missing = tmp.path().join("missing.json");
        let err = find_config(Some(&missing), &[]).unwrap_err().to_string();
        assert!(err.contains("missing.json"), "{}", err);
    }
}
