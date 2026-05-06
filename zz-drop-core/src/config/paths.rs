use std::path::{Path, PathBuf};

use thiserror::Error;

const APP_DIR_NAME: &str = "zz-drop";
const CONFIG_FILE_NAME: &str = "config.toml";
/// Encrypted container of profiles that live only on this machine.
/// If you lose either the file or the passphrase, the contents are
/// gone.
pub const PROFILES_LOCAL_FILE_NAME: &str = "profiles-local.zz";
/// Encrypted container of profiles cached from a
/// `zz-drop.net`-compatible API server. Recoverable from any shell
/// once the operator authenticates again.
pub const PROFILES_REMOTE_FILE_NAME: &str = "profiles-remote.zz";
/// Plaintext sidecar storing the alias of the inner profile last
/// selected from `profiles-local.zz`. Single line `<alias>\n`,
/// chmod 0600. Treated as untrusted input on read.
pub const LAST_DEFAULT_LOCAL_FILE_NAME: &str = "last-default-local";
/// Plaintext sidecar storing `<email>\n<alias>\n` for the remote
/// container — the email spares the operator a re-type on cold
/// start. chmod 0600. Treated as untrusted input on read.
pub const LAST_DEFAULT_REMOTE_FILE_NAME: &str = "last-default-remote";
const AGENT_SOCKET_NAME: &str = "agent.sock";
const TOKEN_FILE_NAME: &str = "token";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub config_file: PathBuf,
    /// `profiles-local.zz` — encrypted container of local-only
    /// profiles. Never synced anywhere.
    pub profiles_local_file: PathBuf,
    /// `profiles-remote.zz` — local cache of the encrypted
    /// container synced with the server.
    pub profiles_remote_file: PathBuf,
    /// `last-default-local` — sidecar plaintext: alias of the
    /// inner profile last picked from the local container.
    pub last_default_local_file: PathBuf,
    /// `last-default-remote` — sidecar plaintext: email + alias
    /// last used for the remote container.
    pub last_default_remote_file: PathBuf,
    pub agent_socket: PathBuf,
    pub token_file: PathBuf,
}

impl Paths {
    /// Pick the file `zz x` should unlock. Precedence: remote first
    /// (synced state is the source of truth), then local-only.
    /// Returns `None` if neither file exists.
    pub fn active_profile_file(&self) -> Option<&Path> {
        if self.profiles_remote_file.exists() {
            Some(&self.profiles_remote_file)
        } else if self.profiles_local_file.exists() {
            Some(&self.profiles_local_file)
        } else {
            None
        }
    }

    /// Path of the diagnostic log shared by all three binaries
    /// (CLI, TUI, agent). Lives in `cache_dir` so it survives a
    /// reboot (unlike `runtime_dir`) but isn't part of the
    /// "secrets" tree under `config_dir`. See
    /// `zz_drop_core::diag_log` for the no-secret rule.
    pub fn debug_log_file(&self) -> PathBuf {
        self.cache_dir.join("zz-drop.log")
    }
}

#[derive(Clone, Debug, Default)]
pub struct PathOverrides {
    pub config_dir: Option<PathBuf>,
    pub cache_dir: Option<PathBuf>,
    pub runtime_dir: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum PathError {
    #[error("could not resolve home directory for the current user")]
    NoBaseDirs,

    #[error("io error: {0}")]
    Io(String),
}

pub fn discover_paths(uid: u32, overrides: &PathOverrides) -> Result<Paths, PathError> {
    let base = directories::BaseDirs::new().ok_or(PathError::NoBaseDirs)?;

    let config_dir = match &overrides.config_dir {
        Some(p) => p.clone(),
        None => base.config_dir().join(APP_DIR_NAME),
    };
    let cache_dir = match &overrides.cache_dir {
        Some(p) => p.clone(),
        None => base.cache_dir().join(APP_DIR_NAME),
    };
    let runtime_dir = match &overrides.runtime_dir {
        Some(p) => p.clone(),
        None => match base.runtime_dir() {
            Some(p) => p.join(APP_DIR_NAME),
            None => PathBuf::from(format!("/tmp/{APP_DIR_NAME}-{uid}")),
        },
    };

    let config_file = config_dir.join(CONFIG_FILE_NAME);
    let profiles_local_file = config_dir.join(PROFILES_LOCAL_FILE_NAME);
    let profiles_remote_file = config_dir.join(PROFILES_REMOTE_FILE_NAME);
    let last_default_local_file = config_dir.join(LAST_DEFAULT_LOCAL_FILE_NAME);
    let last_default_remote_file = config_dir.join(LAST_DEFAULT_REMOTE_FILE_NAME);
    let agent_socket = runtime_dir.join(AGENT_SOCKET_NAME);
    let token_file = runtime_dir.join(TOKEN_FILE_NAME);

    Ok(Paths {
        config_dir,
        cache_dir,
        runtime_dir,
        config_file,
        profiles_local_file,
        profiles_remote_file,
        last_default_local_file,
        last_default_remote_file,
        agent_socket,
        token_file,
    })
}

#[cfg(unix)]
pub fn ensure_dir(path: &Path, mode: u32) -> Result<(), PathError> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::create_dir_all(path).map_err(|e| PathError::Io(e.to_string()))?;
    let perms = std::fs::Permissions::from_mode(mode);
    std::fs::set_permissions(path, perms).map_err(|e| PathError::Io(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn overrides_are_used_verbatim_and_files_are_appended() {
        let tmp = tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let cache = tmp.path().join("cache");
        let runtime = tmp.path().join("rt");

        let paths = discover_paths(
            1234,
            &PathOverrides {
                config_dir: Some(cfg.clone()),
                cache_dir: Some(cache.clone()),
                runtime_dir: Some(runtime.clone()),
            },
        )
        .unwrap();

        assert_eq!(paths.config_dir, cfg);
        assert_eq!(paths.cache_dir, cache);
        assert_eq!(paths.runtime_dir, runtime);
        assert_eq!(paths.config_file, cfg.join("config.toml"));
        assert_eq!(paths.profiles_local_file, cfg.join("profiles-local.zz"));
        assert_eq!(paths.profiles_remote_file, cfg.join("profiles-remote.zz"));
        assert_eq!(
            paths.last_default_local_file,
            cfg.join("last-default-local")
        );
        assert_eq!(
            paths.last_default_remote_file,
            cfg.join("last-default-remote")
        );
        assert_eq!(paths.agent_socket, runtime.join("agent.sock"));
        assert_eq!(paths.token_file, runtime.join("token"));
    }

    #[test]
    fn active_profile_prefers_remote_then_falls_back_to_local() {
        let tmp = tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        std::fs::create_dir_all(&cfg).unwrap();
        let paths = discover_paths(
            1,
            &PathOverrides {
                config_dir: Some(cfg.clone()),
                cache_dir: Some(tmp.path().join("ca")),
                runtime_dir: Some(tmp.path().join("rt")),
            },
        )
        .unwrap();

        assert!(paths.active_profile_file().is_none());

        std::fs::write(&paths.profiles_local_file, b"x").unwrap();
        assert_eq!(
            paths.active_profile_file().unwrap(),
            paths.profiles_local_file.as_path()
        );

        std::fs::write(&paths.profiles_remote_file, b"y").unwrap();
        assert_eq!(
            paths.active_profile_file().unwrap(),
            paths.profiles_remote_file.as_path()
        );
    }

    #[cfg(unix)]
    #[test]
    fn ensure_dir_creates_with_mode() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("nested").join("dir");
        ensure_dir(&target, 0o700).unwrap();
        let mode = std::fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }
}
