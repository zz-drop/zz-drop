use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

const HEADER: &str = "# zz-drop config — never put secrets here\n";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalConfig {
    #[serde(default = "default_server_base_url")]
    pub server_base_url: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_alias: Option<String>,
}

fn default_server_base_url() -> String {
    "https://zz-drop.net".to_string()
}

impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            server_base_url: default_server_base_url(),
            default_alias: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io error: {0}")]
    Io(String),

    #[error("malformed config: {0}")]
    Malformed(String),
}

pub fn load(path: &Path) -> Result<LocalConfig, ConfigError> {
    let raw = std::fs::read_to_string(path).map_err(|e| ConfigError::Io(e.to_string()))?;
    toml::from_str(&raw).map_err(|e| ConfigError::Malformed(e.to_string()))
}

pub fn load_or_default(path: &Path) -> Result<LocalConfig, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(raw) => toml::from_str(&raw).map_err(|e| ConfigError::Malformed(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LocalConfig::default()),
        Err(e) => Err(ConfigError::Io(e.to_string())),
    }
}

pub fn save(path: &Path, config: &LocalConfig) -> Result<(), ConfigError> {
    let body = toml::to_string(config).map_err(|e| ConfigError::Malformed(e.to_string()))?;
    let content = format!("{HEADER}{body}");
    std::fs::write(path, content).map_err(|e| ConfigError::Io(e.to_string()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms).map_err(|e| ConfigError::Io(e.to_string()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_are_zz_drop_net_and_no_alias() {
        let cfg = LocalConfig::default();
        assert_eq!(cfg.server_base_url, "https://zz-drop.net");
        assert!(cfg.default_alias.is_none());
    }

    #[test]
    fn round_trip_save_load() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("config.toml");

        let mut cfg = LocalConfig::default();
        cfg.default_alias = Some("casa-nc".into());

        save(&path, &cfg).unwrap();
        let restored = load(&path).unwrap();
        assert_eq!(restored, cfg);
    }

    #[test]
    fn load_or_default_for_missing_file_returns_defaults() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("nope.toml");
        let cfg = load_or_default(&path).unwrap();
        assert_eq!(cfg, LocalConfig::default());
    }

    #[test]
    fn malformed_file_is_a_loud_error() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("bad.toml");
        std::fs::write(&path, "this is = = not toml { [").unwrap();
        let res = load_or_default(&path);
        assert!(matches!(res, Err(ConfigError::Malformed(_))), "got {res:?}");
    }

    #[cfg(unix)]
    #[test]
    fn save_sets_0600() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        save(&path, &LocalConfig::default()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn serialized_config_contains_header_and_no_secret_keys() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        save(&path, &LocalConfig::default()).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.starts_with("# zz-drop config — never put secrets here"));

        // Skip comment lines so the warning header itself doesn't match.
        let body: String = raw
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();

        for forbidden in ["password", "passphrase", "secret", "token"] {
            assert!(
                !body.contains(forbidden),
                "config.toml body must not contain `{forbidden}`: got `{body}`"
            );
        }
    }
}
