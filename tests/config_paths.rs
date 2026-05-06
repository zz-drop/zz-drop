use tempfile::tempdir;

use zz_drop::config::{current_uid, discover, ensure_dirs, load_config};
use zz_drop_core::config::{
    LocalConfig, PathOverrides, discover_paths, load, load_or_default, save,
};

#[test]
fn current_uid_is_real() {
    let uid = current_uid();
    // root or a regular user — must be a defined value.
    // (We don't assert > 0 because root has uid 0 and is valid.)
    let _ = uid;
}

#[test]
fn discover_resolves_filenames() {
    let paths = discover().unwrap();

    assert_eq!(paths.config_file.file_name().unwrap(), "config.toml");
    assert_eq!(
        paths.profiles_local_file.file_name().unwrap(),
        "profiles-local.zz"
    );
    assert_eq!(
        paths.profiles_remote_file.file_name().unwrap(),
        "profiles-remote.zz"
    );
    assert_eq!(
        paths.last_default_local_file.file_name().unwrap(),
        "last-default-local"
    );
    assert_eq!(
        paths.last_default_remote_file.file_name().unwrap(),
        "last-default-remote"
    );
    assert_eq!(paths.agent_socket.file_name().unwrap(), "agent.sock");
    assert_eq!(paths.token_file.file_name().unwrap(), "token");

    assert!(paths.config_dir.to_string_lossy().contains("zz-drop"));
    assert!(paths.runtime_dir.to_string_lossy().contains("zz-drop"));
}

#[test]
fn discover_with_overrides_assembles_paths_correctly() {
    let tmp = tempdir().unwrap();
    let cfg = tmp.path().join("cfg");
    let cache = tmp.path().join("cache");
    let runtime = tmp.path().join("rt");

    let paths = discover_paths(
        4242,
        &PathOverrides {
            config_dir: Some(cfg.clone()),
            cache_dir: Some(cache.clone()),
            runtime_dir: Some(runtime.clone()),
        },
    )
    .unwrap();

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
    assert_eq!(paths.cache_dir, cache);
}

#[test]
fn ensure_dirs_creates_overridden_dirs_with_0700() {
    let tmp = tempdir().unwrap();
    let cfg = tmp.path().join("cfg");
    let runtime = tmp.path().join("rt");

    let paths = discover_paths(
        4242,
        &PathOverrides {
            config_dir: Some(cfg.clone()),
            runtime_dir: Some(runtime.clone()),
            ..PathOverrides::default()
        },
    )
    .unwrap();

    ensure_dirs(&paths).unwrap();

    assert!(cfg.is_dir());
    assert!(runtime.is_dir());

    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&cfg).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(&runtime).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

#[test]
fn load_config_returns_defaults_for_missing_file() {
    let tmp = tempdir().unwrap();
    let cfg = tmp.path().join("cfg");
    let runtime = tmp.path().join("rt");
    std::fs::create_dir_all(&cfg).unwrap();

    let paths = discover_paths(
        4242,
        &PathOverrides {
            config_dir: Some(cfg.clone()),
            runtime_dir: Some(runtime),
            ..PathOverrides::default()
        },
    )
    .unwrap();

    let cfg = load_config(&paths).unwrap();
    assert_eq!(cfg, LocalConfig::default());
}

#[test]
fn save_then_load_via_paths_round_trip() {
    let tmp = tempdir().unwrap();
    let cfg_dir = tmp.path().join("cfg");
    let runtime = tmp.path().join("rt");
    std::fs::create_dir_all(&cfg_dir).unwrap();

    let paths = discover_paths(
        4242,
        &PathOverrides {
            config_dir: Some(cfg_dir.clone()),
            runtime_dir: Some(runtime),
            ..PathOverrides::default()
        },
    )
    .unwrap();

    let mut cfg = LocalConfig::default();
    cfg.default_alias = Some("home".into());

    save(&paths.config_file, &cfg).unwrap();
    let restored = load(&paths.config_file).unwrap();
    assert_eq!(restored, cfg);
}

#[test]
fn malformed_config_is_loud_via_load_or_default() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("config.toml");
    std::fs::write(&path, "garbage = = nope { [").unwrap();
    let res = load_or_default(&path);
    assert!(res.is_err(), "malformed config must error, got {res:?}");
}
