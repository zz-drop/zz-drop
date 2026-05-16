//! Smoke tests for the scriptable-mode flag routing.
//!
//! Spawns the real `zz-drop` binary with a tempdir as
//! `ZZ_CONFIG_DIR`, so each test runs against an isolated state
//! tree with no agent socket. The per-command JSON emitters are
//! still TODO — these tests only verify that the global-flag and
//! env-var routing reaches dispatch without crashing.
//!
//! Path note: `cargo test` for an integration test sets
//! `CARGO_BIN_EXE_zz-drop` to the freshly-built binary path.

use std::path::PathBuf;
use std::process::{Command, Output};

use tempfile::TempDir;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zz-drop"))
}

/// Build a `Command` rooted at the binary, with a fresh tempdir
/// set as `ZZ_CONFIG_DIR` so the run can't see real user state.
/// Clears every `ZZ_*` env var first so a parent shell doesn't
/// poison the test.
fn cmd_in_tmp() -> (TempDir, Command) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Command::new(binary());
    for (k, _) in std::env::vars() {
        if k.starts_with("ZZ_") {
            c.env_remove(&k);
        }
    }
    c.env("ZZ_CONFIG_DIR", dir.path());
    (dir, c)
}

fn run(c: &mut Command) -> Output {
    c.output().expect("failed to spawn zz-drop")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn plain_q_with_no_agent_says_already_locked() {
    // Sanity baseline — confirms the dispatch reaches q_lock and
    // the tempdir really has no agent socket.
    let (_dir, mut c) = cmd_in_tmp();
    let out = run(c.arg("q"));
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("already locked"), "stdout: {}", stdout(&out));
}

#[test]
fn json_flag_is_consumed_before_verb_parser() {
    // `--json q` reaches q_lock with no parser error. The text
    // output is still text (per-command JSON emitters come later)
    // — this test only proves the pre-pass strips the flag and
    // the verb parser sees `["q"]`.
    let (_dir, mut c) = cmd_in_tmp();
    let out = run(c.args(["--json", "q"]));
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

#[test]
fn quiet_flag_is_consumed_before_verb_parser() {
    let (_dir, mut c) = cmd_in_tmp();
    let out = run(c.args(["--quiet", "q"]));
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

#[test]
fn quiet_json_conflict_exits_usage() {
    let (_dir, mut c) = cmd_in_tmp();
    let out = run(c.args(["--quiet", "--json", "q"]));
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("--quiet and --json"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn unknown_global_flag_exits_usage() {
    let (_dir, mut c) = cmd_in_tmp();
    let out = run(c.args(["--bogus-flag", "q"]));
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown global flag"));
}

#[test]
fn double_dash_terminates_flag_parsing() {
    // After `--`, residual = ["q"] which is a legitimate verb.
    let (_dir, mut c) = cmd_in_tmp();
    let out = run(c.args(["--", "q"]));
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

#[test]
fn passphrase_file_arg_is_consumed_with_value() {
    // No actual unlock happens — `q` doesn't use it. We just
    // prove the pre-pass eats both tokens without surprising the
    // verb parser.
    let (_dir, mut c) = cmd_in_tmp();
    let out = run(c.args(["--passphrase-file", "/tmp/nope", "q"]));
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

#[test]
fn passphrase_file_arg_eq_form() {
    let (_dir, mut c) = cmd_in_tmp();
    let out = run(c.args(["--passphrase-file=/tmp/nope", "q"]));
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

#[test]
fn alias_arg_is_consumed() {
    let (_dir, mut c) = cmd_in_tmp();
    let out = run(c.args(["--alias", "work-nc", "q"]));
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

#[test]
fn env_zz_output_json_is_accepted() {
    let (_dir, mut c) = cmd_in_tmp();
    c.env("ZZ_OUTPUT", "json");
    let out = run(c.arg("q"));
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

#[test]
fn env_zz_output_garbage_exits_usage() {
    let (_dir, mut c) = cmd_in_tmp();
    c.env("ZZ_OUTPUT", "garbage");
    let out = run(c.arg("q"));
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("ZZ_OUTPUT"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn env_zz_output_quiet_is_rejected() {
    // `quiet` is flag-only by design; env support is documented
    // as `text` | `json`.
    let (_dir, mut c) = cmd_in_tmp();
    c.env("ZZ_OUTPUT", "quiet");
    let out = run(c.arg("q"));
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn env_zz_config_dir_relative_path_is_rejected() {
    let mut c = Command::new(binary());
    for (k, _) in std::env::vars() {
        if k.starts_with("ZZ_") {
            c.env_remove(&k);
        }
    }
    c.env("ZZ_CONFIG_DIR", "relative/path");
    let out = run(c.arg("f"));
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("absolute path"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn env_zz_config_dir_absolute_layouts_state_tree() {
    // Verify that `zz f --json`-equivalent dispatch under
    // `ZZ_CONFIG_DIR` lands at `<root>/{config,cache,runtime}`.
    // The `f` (doctor) command renders the resolved paths on
    // stdout, which is the cheapest way to assert from outside.
    let (dir, mut c) = cmd_in_tmp();
    let out = run(c.arg("f"));
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains(&format!("{}/config", dir.path().display())), "stdout:\n{s}");
    assert!(s.contains(&format!("{}/runtime", dir.path().display())), "stdout:\n{s}");
}

#[test]
fn help_still_works_when_json_prefixed_help_does_not() {
    // `zz --help` bypasses the global-flag pre-pass via the SACS
    // tooling intercept on the first arg.
    let (_dir, mut c) = cmd_in_tmp();
    let out = run(c.arg("--help"));
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).to_lowercase().contains("zz-drop"),
        "stdout: {}",
        stdout(&out)
    );

    // `zz --json --help` is documented as "tooling must be the
    // first arg"; the global pre-pass sees `--help` as unknown.
    let (_dir, mut c) = cmd_in_tmp();
    let out = run(c.args(["--json", "--help"]));
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown global flag"));
}
