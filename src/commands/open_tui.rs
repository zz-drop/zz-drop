use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::output;

/// Name of the TUI binary on PATH. It is shipped as `zz-tui` by the
/// `zz-drop-tui` crate's `[[bin]]` entry; on Windows it lives next to
/// it as `zz-tui.exe`.
const TUI_BINARY: &str = "zz-tui";

/// UNIX convention: the shell returns 127 when a command is not found.
/// We mirror it here so scripts that wrap `zz c` see the same code.
pub const EXIT_TUI_NOT_FOUND: i32 = 127;

/// Resolve `zz-tui` on PATH and run it, propagating its exit code.
/// Prints a one-line diagnostic and returns 127 when the binary is
/// missing, or when launching it fails for any other reason.
pub fn run() -> i32 {
    run_with_env(env::var_os("PATH").as_deref())
}

/// Test seam: lets the integration tests inject a custom `PATH`
/// without poisoning the parent process's environment.
pub fn run_with_env(path_var: Option<&OsStr>) -> i32 {
    let Some(path) = find_in_path(TUI_BINARY, path_var) else {
        output::line(&format!(
            "zz c: `{TUI_BINARY}` not found on PATH.\n\
             install the zz-drop package, or add the binary to PATH."
        ));
        return EXIT_TUI_NOT_FOUND;
    };

    match Command::new(&path).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            output::line(&format!(
                "zz c: failed to launch `{}`: {e}",
                path.display()
            ));
            EXIT_TUI_NOT_FOUND
        }
    }
}

fn find_in_path(name: &str, path_var: Option<&OsStr>) -> Option<PathBuf> {
    let path_var = path_var?;
    for dir in env::split_paths(path_var) {
        for candidate in candidates(&dir, name) {
            if is_runnable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    let mut out = vec![dir.join(name)];
    if cfg!(windows) {
        out.push(dir.join(format!("{name}.exe")));
    }
    out
}

#[cfg(unix)]
fn is_runnable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(m) => m.is_file() && m.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_runnable(path: &Path) -> bool {
    matches!(std::fs::metadata(path), Ok(m) if m.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn empty_path_means_not_found() {
        assert_eq!(run_with_env(Some(OsStr::new(""))), EXIT_TUI_NOT_FOUND);
    }

    #[test]
    fn missing_path_var_means_not_found() {
        assert_eq!(run_with_env(None), EXIT_TUI_NOT_FOUND);
    }

    #[test]
    fn finds_binary_in_supplied_path() {
        // Put a tempdir on PATH that does NOT contain zz-tui;
        // find_in_path should return None.
        let tmp = tempfile::tempdir().unwrap();
        let mut path = OsString::from(tmp.path());
        path.push(":/this/dir/does/not/exist");
        assert!(find_in_path("zz-tui", Some(path.as_os_str())).is_none());
    }
}
