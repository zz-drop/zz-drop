use zz_drop_core::config::{
    self, ConfigError, LocalConfig, PathError, Paths, config_root_from_env, discover_paths,
    load_or_default,
};

pub fn current_uid() -> u32 {
    rustix::process::getuid().as_raw()
}

/// Resolve the path layout for the current process. Honours
/// `ZZ_CONFIG_DIR` (absolute root) by routing config / cache /
/// runtime under it; otherwise falls back to OS defaults via
/// `directories`. The env lookup uses `std::env::var_os` and
/// silently ignores non-UTF-8 values.
pub fn discover() -> Result<Paths, PathError> {
    let overrides = config_root_from_env(|k| {
        std::env::var_os(k).and_then(|v| v.into_string().ok())
    })?
    .unwrap_or_default();
    discover_paths(current_uid(), &overrides)
}

pub fn load_config(paths: &Paths) -> Result<LocalConfig, ConfigError> {
    load_or_default(&paths.config_file)
}

#[cfg(unix)]
pub fn ensure_dirs(paths: &Paths) -> Result<(), PathError> {
    config::ensure_dir(&paths.config_dir, 0o700)?;
    config::ensure_dir(&paths.runtime_dir, 0o700)?;
    Ok(())
}
