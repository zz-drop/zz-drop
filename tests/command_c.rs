// Integration tests for the `zz c` launcher.
//
// We avoid mutating the test process's PATH (would race with other
// tests and leak state). Instead we drive `commands::config::run_with_env`
// with a synthetic PATH pointing at a tempdir we control.

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use zz_drop::commands::open_tui::{EXIT_TUI_NOT_FOUND, run_with_env};

fn write_executable(path: &Path, body: &str) {
    fs::write(path, body).expect("write fake binary");
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod +x");
}

#[test]
fn empty_path_returns_not_found() {
    let code = run_with_env(Some(OsString::from("").as_os_str()));
    assert_eq!(code, EXIT_TUI_NOT_FOUND);
}

#[test]
fn finds_and_runs_fake_zz_tui_with_zero_exit() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("zz-tui");
    write_executable(&bin, "#!/bin/sh\nexit 0\n");

    let code = run_with_env(Some(tmp.path().as_os_str()));
    assert_eq!(code, 0);
}

#[test]
fn propagates_non_zero_exit_from_fake_zz_tui() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("zz-tui");
    write_executable(&bin, "#!/bin/sh\nexit 42\n");

    let code = run_with_env(Some(tmp.path().as_os_str()));
    assert_eq!(code, 42);
}

#[test]
fn non_executable_file_does_not_match() {
    // A file named zz-tui that is NOT executable must be ignored,
    // not "found and tried".
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("zz-tui");
    fs::write(&bin, "not executable").unwrap();
    let mut perms = fs::metadata(&bin).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&bin, perms).unwrap();

    let code = run_with_env(Some(tmp.path().as_os_str()));
    assert_eq!(code, EXIT_TUI_NOT_FOUND);
}
