pub mod local_config;
pub mod paths;

pub use local_config::{ConfigError, LocalConfig, load, load_or_default, save};
#[cfg(unix)]
pub use paths::ensure_dir;
pub use paths::{PathError, PathOverrides, Paths, discover_paths};
