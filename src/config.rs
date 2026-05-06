use zz_drop_core::config::{
    self, ConfigError, LocalConfig, PathError, PathOverrides, Paths, discover_paths,
    load_or_default,
};

pub fn current_uid() -> u32 {
    rustix::process::getuid().as_raw()
}

pub fn discover() -> Result<Paths, PathError> {
    discover_paths(current_uid(), &PathOverrides::default())
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
