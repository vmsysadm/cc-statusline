use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug, Default)]
pub(crate) struct ConfigColors {
    pub(crate) reset: Option<String>,
    pub(crate) bold: Option<String>,
    pub(crate) italic: Option<String>,
    pub(crate) foreground: Option<HashMap<String, String>>,
    pub(crate) ui: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct Config {
    pub(crate) colors: Option<ConfigColors>,
    pub(crate) icons: Option<serde_json::Value>,
    /// Force the YOLO badge on regardless of the detected permission mode.
    #[serde(default)]
    pub(crate) yolo: Option<bool>,
    #[serde(default)]
    pub(crate) use_ascii: Option<bool>,
    #[serde(default, alias = "nerd_fonts")]
    pub(crate) use_nerd_fonts: Option<bool>,
}

/// Directory, under Claude Code's config dir, that holds the binary and its config.
pub(crate) const INSTALL_DIR_NAME: &str = "cc-statusline";

/// Config file name inside [`INSTALL_DIR_NAME`].
pub(crate) const CONFIG_FILE_NAME: &str = "statusline_config.json";

/// Where the statusline config lives.
///
/// An explicit `CC_STATUSLINE_CONFIG` path wins outright. Otherwise the file sits next to the
/// installed binary inside Claude Code's own config directory, which is `$CLAUDE_CONFIG_DIR`
/// when the user relocated it and `~/.claude` when they did not -- the same rule Claude Code
/// itself follows, so a relocated install keeps finding its config.
pub(crate) fn config_path(home_path: &str, claude_config_dir: Option<&str>, explicit: Option<&str>) -> PathBuf {
    if let Some(p) = explicit.filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let base = match claude_config_dir.filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => Path::new(home_path).join(".claude"),
    };
    base.join(INSTALL_DIR_NAME).join(CONFIG_FILE_NAME)
}

/// Load the statusline config, falling back to defaults when it is missing or malformed.
pub(crate) fn load_config(path: &Path) -> Config {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path_default() {
        let home = std::env::temp_dir().join("cc-home");
        let path = config_path(&home.to_string_lossy(), None, None);
        assert_eq!(path, home.join(".claude").join("cc-statusline").join("statusline_config.json"));
    }

    #[test]
    fn test_config_path_follows_claude_config_dir() {
        let dir = std::env::temp_dir().join("relocated-claude");
        let path = config_path("/unused", Some(&dir.to_string_lossy()), None);
        assert_eq!(path, dir.join("cc-statusline").join("statusline_config.json"));
    }

    #[test]
    fn test_config_path_explicit_wins() {
        let path = config_path("/unused", Some("/also-unused"), Some("/etc/my.json"));
        assert_eq!(path, PathBuf::from("/etc/my.json"));
    }

    #[test]
    fn test_config_path_ignores_empty_overrides() {
        let home = std::env::temp_dir().join("cc-home");
        let path = config_path(&home.to_string_lossy(), Some(""), Some(""));
        assert_eq!(path, home.join(".claude").join("cc-statusline").join("statusline_config.json"));
    }

    #[test]
    fn test_load_config_missing_or_malformed() {
        let missing = std::env::temp_dir().join("cc-statusline-no-such-config.json");
        assert!(load_config(&missing).use_ascii.is_none());

        let bad = std::env::temp_dir().join(format!("cc-statusline-bad-{}.json", std::process::id()));
        fs::write(&bad, "{ not json").unwrap();
        assert!(load_config(&bad).use_ascii.is_none());
        let _ = fs::remove_file(&bad);
    }
}
