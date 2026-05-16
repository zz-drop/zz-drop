//! Universal lint: every CLI verb either emits at least one
//! valid NDJSON record in `--json` mode or surfaces a documented
//! `failed reason=<rejection>` record (e.g. `interactive_only`
//! for `zz c`). A new verb cannot ship without scriptable support
//! — adding one and forgetting the emit path fails this test
//! before the freeze.
//!
//! Each cell of the matrix:
//!
//! - spawns the real `zz-drop` binary
//! - isolates state via `ZZ_CONFIG_DIR=<tempdir>` (no agent
//!   socket, no profile container)
//! - asserts every stdout line is well-formed NDJSON (`v: "1"`,
//!   `event`, `ts`)
//! - asserts the first record's `event` is in the per-verb
//!   allow-list and, for `failed`, the `reason` is in the
//!   per-verb allow-list
//!
//! The matrix lives close to the verbs it covers because a new
//! `Command` variant means a new row here, and the cargo test
//! runner will yell if anyone forgets.

use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zz-drop"))
}

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

/// Parse every non-empty stdout line as JSON and validate the
/// mandatory schema fields. Returns the parsed events in order.
fn parse_ndjson(stdout: &[u8]) -> Vec<Value> {
    let s = std::str::from_utf8(stdout).expect("stdout must be UTF-8 in --json mode");
    let mut out = Vec::new();
    for (i, line) in s.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(line).unwrap_or_else(|e| {
            panic!("line {i} is not valid JSON: {e}\nline: `{line}`")
        });
        assert_eq!(
            v.get("v").and_then(|x| x.as_str()),
            Some("1"),
            "missing or wrong `v`: {line}"
        );
        assert!(v.get("event").and_then(|x| x.as_str()).is_some(), "missing `event`: {line}");
        let ts = v.get("ts").and_then(|x| x.as_str()).expect("missing `ts`");
        // Loose check: shape `YYYY-MM-DDTHH:MM:SSZ`.
        assert_eq!(ts.len(), 20, "ts shape: {line}");
        assert!(ts.ends_with('Z'), "ts must end in Z: {line}");
        out.push(v);
    }
    out
}

/// Run `zz-drop --json <argv...>` under an isolated ZZ_CONFIG_DIR.
/// Returns the parsed NDJSON event list, exit code, and stderr.
fn run_json(argv: &[&str]) -> (Vec<Value>, i32, String) {
    let (_dir, mut c) = cmd_in_tmp();
    c.arg("--json");
    for a in argv {
        c.arg(a);
    }
    let out = run(&mut c);
    let events = parse_ndjson(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (events, out.status.code().unwrap_or(-1), stderr)
}

/// Assert there is at least one event and its `event` field is in
/// `allowed`. If it's a `failed`, the `reason` must be in
/// `allowed_reasons`.
fn assert_first_event_in(events: &[Value], allowed: &[&str], allowed_reasons: &[&str]) {
    assert!(!events.is_empty(), "expected ≥ 1 NDJSON record, got 0");
    let first = &events[0];
    let ev = first.get("event").and_then(|x| x.as_str()).unwrap();
    assert!(
        allowed.contains(&ev),
        "first event `{ev}` not in allowed set {allowed:?}\nrecord: {first}"
    );
    if ev == "failed" {
        let reason = first.get("reason").and_then(|x| x.as_str()).unwrap_or("?");
        assert!(
            allowed_reasons.contains(&reason),
            "failed reason `{reason}` not in allowed set {allowed_reasons:?}\nrecord: {first}"
        );
    }
}

// ---------------------------------------------------------------
// Per-verb cases. Each verb either reaches its happy path (within
// the constraints of an empty state tree) or fails with a
// documented reason.
// ---------------------------------------------------------------

#[test]
fn verb_q_lock_emits_locked() {
    // No agent in the tempdir → q_lock takes the idempotent path
    // and emits `locked`.
    let (events, code, _) = run_json(&["q"]);
    assert_eq!(code, 0);
    assert_first_event_in(&events, &["locked"], &[]);
}

#[test]
fn verb_w_wipe_without_yes_fails_interactive_required() {
    let (events, code, _) = run_json(&["w"]);
    assert_eq!(code, 2);
    assert_first_event_in(&events, &["failed"], &["interactive_required"]);
}

#[test]
fn verb_w_wipe_with_yes_emits_wiped() {
    let (_dir, mut c) = cmd_in_tmp();
    c.args(["--json", "--yes", "w"]);
    let out = run(&mut c);
    let events = parse_ndjson(&out.stdout);
    assert_eq!(out.status.code(), Some(0));
    assert_first_event_in(&events, &["wiped"], &[]);
}

#[test]
fn verb_c_open_tui_rejects_in_json() {
    let (events, code, _) = run_json(&["c"]);
    assert_eq!(code, 2);
    assert_first_event_in(&events, &["failed"], &["interactive_only"]);
}

#[test]
fn verb_f_doctor_streams_checks_and_summary() {
    let (events, code, _) = run_json(&["f"]);
    assert_eq!(code, 0);
    // Doctor must emit at least one `doctor_check` and one
    // `doctor_summary` as the final record.
    assert!(events
        .iter()
        .any(|v| v.get("event").and_then(|x| x.as_str()) == Some("doctor_check")));
    let last = events.last().unwrap();
    assert_eq!(
        last.get("event").and_then(|x| x.as_str()),
        Some("doctor_summary")
    );
}

#[test]
fn verb_z_unlock_without_profile_fails_profile_missing() {
    let (events, code, _) = run_json(&["z"]);
    assert_eq!(code, 6);
    assert_first_event_in(&events, &["failed"], &["profile_missing"]);
}

#[test]
fn verb_s_upload_without_agent_fails_agent_locked() {
    // No agent → upload bails before touching any provider.
    let (events, code, _) = run_json(&["s", "/tmp/nonexistent-zz-test"]);
    assert_eq!(code, 10);
    assert_first_event_in(&events, &["failed"], &["agent_locked"]);
}

#[test]
fn verb_default_upload_without_agent_fails_agent_locked() {
    // Bare `zz <file>` (no explicit verb) routes through the
    // same agent gate.
    let (events, code, _) = run_json(&["/tmp/nonexistent-zz-test"]);
    assert_eq!(code, 10);
    assert_first_event_in(&events, &["failed"], &["agent_locked"]);
}

#[test]
fn verb_d_download_without_agent_fails_agent_locked() {
    let (events, code, _) = run_json(&["d", "remote-name.md"]);
    assert_eq!(code, 10);
    assert_first_event_in(&events, &["failed"], &["agent_locked"]);
}

#[test]
fn verb_da_download_all_without_agent_fails_agent_locked() {
    let (events, code, _) = run_json(&["da"]);
    assert_eq!(code, 10);
    assert_first_event_in(&events, &["failed"], &["agent_locked"]);
}

#[test]
fn verb_dax_bulk_decompress_blocked_by_agent_gate_first() {
    // `dax` is documented as "not_implemented" but the agent
    // gate runs first — with no unlocked profile we never reach
    // the bulk-decompress branch. This row pins the externally
    // visible behaviour: in an empty state tree, dax fails with
    // `agent_locked`, not `not_implemented`. The `not_implemented`
    // path is exercised by unit-level tests where state is
    // injected directly.
    let (events, code, _) = run_json(&["dax"]);
    assert_eq!(code, 10);
    assert_first_event_in(&events, &["failed"], &["agent_locked"]);
}

#[test]
fn verb_sa_save_all_without_agent_fails_agent_locked() {
    let (events, code, _) = run_json(&["sa", "/tmp"]);
    assert_eq!(code, 10);
    assert_first_event_in(&events, &["failed"], &["agent_locked"]);
}

#[test]
fn verb_z_with_passphrase_file_arg_consumed() {
    // `--passphrase-file` reaches the verb dispatcher; without a
    // profile container the failure is still `profile_missing`
    // — but the flag has been parsed without error, which is
    // what this row checks.
    let (events, code, _) = run_json(&["--passphrase-file", "/tmp/nope", "z"]);
    assert_eq!(code, 6);
    assert_first_event_in(&events, &["failed"], &["profile_missing"]);
}

// ---------------------------------------------------------------
// Cross-cutting checks
// ---------------------------------------------------------------

#[test]
fn stderr_is_empty_in_json_mode_when_dispatch_reached() {
    // Every record-bearing verb should NOT also leak text to
    // stderr — JSON consumers expect a clean stderr.
    let verbs: &[&[&str]] = &[
        &["q"],
        &["f"],
        &["z"],
        &["d", "x.md"],
        &["da"],
    ];
    for argv in verbs {
        let (_events, _code, stderr) = run_json(argv);
        assert!(
            stderr.is_empty(),
            "stderr must be empty in --json mode for `zz {}`; got: `{stderr}`",
            argv.join(" ")
        );
    }
}

#[test]
fn every_record_has_v_event_ts_in_order() {
    // Parse a representative verb's output and check the field
    // order: serde_json preserves struct field order, so the
    // first three keys MUST be `v`, `event`, `ts`. Scripts that
    // pin against the schema can rely on this.
    let (_dir, mut c) = cmd_in_tmp();
    c.args(["--json", "f"]);
    let out = run(&mut c);
    let s = std::str::from_utf8(&out.stdout).unwrap();
    for line in s.lines().filter(|l| !l.is_empty()) {
        assert!(
            line.starts_with("{\"v\":\"1\",\"event\":\""),
            "line must start with v then event: {line}"
        );
        let ts_pos = line.find("\"ts\":").unwrap();
        let event_pos = line.find("\"event\":").unwrap();
        assert!(event_pos < ts_pos, "event must come before ts: {line}");
    }
}
