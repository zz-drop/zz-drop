// Integration tests for the `zz c` launcher.
//
// We avoid mutating the test process's PATH (would race with other
// tests and leak state). Instead we drive `commands::config::run_with_env`
// with a synthetic PATH pointing at a tempdir we control.

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use zz_drop::commands::open_tui::{EXIT_TUI_NOT_FOUND, run_with_env};

fn write_executable(path: &Path, body: &str) {
    // Linux can return ETXTBSY ("Text file busy") on exec() of a
    // file that was just written, even after the writable fd is
    // closed — the kernel's i_writecount tracking can lag for a
    // few ms on the GitHub-hosted runner. sync_all + drop is not
    // enough; we additionally warm up the exec path below until
    // the kernel accepts it, so the caller has a contractual
    // guarantee that a subsequent spawn won't flake.
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .expect("open fake binary");
    f.write_all(body.as_bytes()).expect("write fake binary");
    f.sync_all().expect("sync_all");
    drop(f);
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod +x");

    // libc ETXTBSY = 26. Retry the spawn for ~200ms; the script
    // body runs each time (harmless: it's `exit 0` / `exit 42`
    // that the test will then re-run and observe).
    for _ in 0..20 {
        match Command::new(path).status() {
            Ok(_) => return,
            Err(e) if e.raw_os_error() == Some(26) => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("warmup spawn of {path:?} failed: {e}"),
        }
    }
    panic!("ETXTBSY persisted on {path:?} after 200ms");
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
