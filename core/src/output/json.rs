//! NDJSON event types for `--json` mode.
//!
//! Each struct serializes to exactly one line of NDJSON, with the
//! three mandatory schema fields (`v`, `event`, `ts`) at the start
//! and event-specific fields after. Field ordering matters: the
//! [`SCHEMA_V`]-versioned contract pins both the *set* and the
//! *order* of keys so consumers can pattern-match cheaply.
//!
//! Schema spec: `zz-drop/docs/scriptable.md`.
//! JSON Schema file: `zz-drop/docs/scriptable/zz-drop-output.v1.json`.
//!
//! ## Injection / sanitization
//!
//! All free-form text (file names, error details, host/root) is
//! serialized through `serde_json` which escapes control characters
//! (newlines, `\x1b`, NULs) into `\uXXXX`. The crate never
//! concatenates strings into JSON — every field goes through the
//! serializer.

use serde::Serialize;

use crate::scriptable::schema::{Reason, SCHEMA_V};

/// Generate a UTC RFC 3339 timestamp without fractional seconds:
/// `YYYY-MM-DDTHH:MM:SSZ`. Wall-clock failures (system before
/// epoch — should not happen in practice) collapse to `0`.
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    rfc3339_from_unix(secs)
}

/// Pure function form for deterministic tests. `secs` is seconds
/// since the Unix epoch (UTC).
pub fn rfc3339_from_unix(secs: u64) -> String {
    let (y, mo, d, h, m, s) = civil_from_unix(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Howard Hinnant's `civil_from_days` extended to seconds-of-day.
/// Returns `(year, month [1..12], day [1..31], h [0..23], m [0..59],
/// s [0..59])` for the UTC instant at `secs` past the epoch.
fn civil_from_unix(secs: u64) -> (i32, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let secs_of_day = (secs % 86_400) as u32;
    let hh = secs_of_day / 3600;
    let mm = (secs_of_day / 60) % 60;
    let ss = secs_of_day % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z / 146_097 } else { (z - 146_096) / 146_097 };
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y_final = (if m <= 2 { y + 1 } else { y }) as i32;
    (y_final, m, d, hh, mm, ss)
}

// ---------------------------------------------------------------
// Event structs
// ---------------------------------------------------------------
//
// Every event carries `v`, `event`, `ts` in this exact order at
// the front. Event-specific fields follow. The `event` literal is
// stored as a `&'static str` so the type signature pins it.

/// Successful single-file upload. One per file in a batch.
#[derive(Debug, Serialize)]
pub struct Uploaded<'a> {
    pub v: &'static str,
    pub event: &'static str,
    pub ts: String,
    pub file: &'a str,
    pub bytes: u64,
    /// `Some(N)` when zstd compression ran for this file, with
    /// `N` = saved % (`0..=99`). `None` when the file was uploaded
    /// raw (either no `x` modifier, or below the compression
    /// threshold).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compressed_pct: Option<u32>,
    pub alias: &'a str,
    pub target: &'a str,
}

impl<'a> Uploaded<'a> {
    pub fn new(
        file: &'a str,
        bytes: u64,
        compressed_pct: Option<u32>,
        alias: &'a str,
        target: &'a str,
    ) -> Self {
        Self {
            v: SCHEMA_V,
            event: "uploaded",
            ts: now_rfc3339(),
            file,
            bytes,
            compressed_pct,
            alias,
            target,
        }
    }
}

/// Successful single-file download. One per file in a batch.
#[derive(Debug, Serialize)]
pub struct Downloaded<'a> {
    pub v: &'static str,
    pub event: &'static str,
    pub ts: String,
    pub file: &'a str,
    pub bytes: u64,
    pub alias: &'a str,
    pub target: &'a str,
}

impl<'a> Downloaded<'a> {
    pub fn new(file: &'a str, bytes: u64, alias: &'a str, target: &'a str) -> Self {
        Self {
            v: SCHEMA_V,
            event: "downloaded",
            ts: now_rfc3339(),
            file,
            bytes,
            alias,
            target,
        }
    }
}

/// Single failure record. `file`, `alias`, `target` are optional
/// because some failures happen before any of them is known
/// (e.g. flag parse error, missing profile).
#[derive(Debug, Serialize)]
pub struct Failed<'a> {
    pub v: &'static str,
    pub event: &'static str,
    pub ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<&'a str>,
    pub reason: Reason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<&'a str>,
    /// Populated only when `reason == AliasAmbiguous`: the set of
    /// valid aliases the caller could pick from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<&'a str>,
}

impl<'a> Failed<'a> {
    /// Minimal failure: only a [`Reason`]. Use for usage errors
    /// before any profile is loaded.
    pub fn bare(reason: Reason) -> Self {
        Self {
            v: SCHEMA_V,
            event: "failed",
            ts: now_rfc3339(),
            file: None,
            reason,
            detail: None,
            candidates: None,
            alias: None,
            target: None,
        }
    }

    /// `reason` + free-form `detail` (e.g. provider error body).
    /// Callers must ensure `detail` does not leak credentials.
    pub fn with_detail(reason: Reason, detail: &'a str) -> Self {
        Self {
            v: SCHEMA_V,
            event: "failed",
            ts: now_rfc3339(),
            file: None,
            reason,
            detail: Some(detail),
            candidates: None,
            alias: None,
            target: None,
        }
    }

    /// Per-file failure inside an unlocked session.
    pub fn for_file(
        file: &'a str,
        reason: Reason,
        detail: Option<&'a str>,
        alias: &'a str,
        target: &'a str,
    ) -> Self {
        Self {
            v: SCHEMA_V,
            event: "failed",
            ts: now_rfc3339(),
            file: Some(file),
            reason,
            detail,
            candidates: None,
            alias: Some(alias),
            target: Some(target),
        }
    }

    /// `alias_ambiguous` shorthand: emits the candidate set so the
    /// caller can `--alias` one of them on retry.
    pub fn alias_ambiguous(candidates: Vec<&'a str>) -> Self {
        Self {
            v: SCHEMA_V,
            event: "failed",
            ts: now_rfc3339(),
            file: None,
            reason: Reason::AliasAmbiguous,
            detail: None,
            candidates: Some(candidates),
            alias: None,
            target: None,
        }
    }
}

/// Final record of a batch operation (upload / download / list).
#[derive(Debug, Serialize)]
pub struct BatchSummary {
    pub v: &'static str,
    pub event: &'static str,
    pub ts: String,
    pub total: u32,
    pub ok: u32,
    pub failed: u32,
    pub exit_code: i32,
}

impl BatchSummary {
    pub fn new(total: u32, ok: u32, failed: u32, exit_code: i32) -> Self {
        Self {
            v: SCHEMA_V,
            event: "batch_summary",
            ts: now_rfc3339(),
            total,
            ok,
            failed,
            exit_code,
        }
    }
}

/// Successful `zz z` unlock.
#[derive(Debug, Serialize)]
pub struct Unlocked<'a> {
    pub v: &'static str,
    pub event: &'static str,
    pub ts: String,
    pub alias: &'a str,
    pub target: &'a str,
}

impl<'a> Unlocked<'a> {
    pub fn new(alias: &'a str, target: &'a str) -> Self {
        Self {
            v: SCHEMA_V,
            event: "unlocked",
            ts: now_rfc3339(),
            alias,
            target,
        }
    }
}

/// Successful `zz q` lock.
#[derive(Debug, Serialize)]
pub struct Locked {
    pub v: &'static str,
    pub event: &'static str,
    pub ts: String,
}

impl Default for Locked {
    fn default() -> Self {
        Self::new()
    }
}

impl Locked {
    pub fn new() -> Self {
        Self {
            v: SCHEMA_V,
            event: "locked",
            ts: now_rfc3339(),
        }
    }
}

/// Successful `zz w` wipe.
#[derive(Debug, Serialize)]
pub struct Wiped {
    pub v: &'static str,
    pub event: &'static str,
    pub ts: String,
}

impl Default for Wiped {
    fn default() -> Self {
        Self::new()
    }
}

impl Wiped {
    pub fn new() -> Self {
        Self {
            v: SCHEMA_V,
            event: "wiped",
            ts: now_rfc3339(),
        }
    }
}

/// Single `zz f` (doctor) probe result.
#[derive(Debug, Serialize)]
pub struct DoctorCheck<'a> {
    pub v: &'static str,
    pub event: &'static str,
    pub ts: String,
    pub name: &'a str,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<&'a str>,
}

impl<'a> DoctorCheck<'a> {
    pub fn new(name: &'a str, ok: bool, detail: Option<&'a str>) -> Self {
        Self {
            v: SCHEMA_V,
            event: "doctor_check",
            ts: now_rfc3339(),
            name,
            ok,
            detail,
        }
    }
}

/// Final `zz f` summary record.
#[derive(Debug, Serialize)]
pub struct DoctorSummary<'a> {
    pub v: &'static str,
    pub event: &'static str,
    pub ts: String,
    pub ok: bool,
    pub failed: Vec<&'a str>,
}

impl<'a> DoctorSummary<'a> {
    pub fn new(ok: bool, failed: Vec<&'a str>) -> Self {
        Self {
            v: SCHEMA_V,
            event: "doctor_summary",
            ts: now_rfc3339(),
            ok,
            failed,
        }
    }
}

/// Outcome of one `--setup-completions` invocation. Serializes to
/// a single record with the shell + filesystem state that resulted.
#[derive(Debug, Serialize)]
pub struct CompletionsSetup<'a> {
    pub v: &'static str,
    pub event: &'static str,
    pub ts: String,
    pub shell: &'static str,
    pub completion_path: &'a str,
    /// `"created" | "updated" | "unchanged"` — what happened to the
    /// completion script file on disk.
    pub completion_action: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rc_path: Option<&'a str>,
    /// `"inserted" | "updated" | "unchanged" | "not_needed"` — what
    /// happened to the rc-file block. `not_needed` means the shell
    /// doesn't require an rc edit (fish, or bash with the
    /// bash-completion framework already loaded).
    pub rc_action: &'static str,
    /// `"none"` or one of the known zsh framework names (`oh-my-zsh`,
    /// `prezto`, `zinit`, …). Only meaningful for `shell == "zsh"`.
    pub framework: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<&'a str>,
}

impl<'a> CompletionsSetup<'a> {
    pub fn new(
        shell: &'static str,
        completion_path: &'a str,
        completion_action: &'static str,
        rc_path: Option<&'a str>,
        rc_action: &'static str,
        framework: &'static str,
        hint: Option<&'a str>,
    ) -> Self {
        Self {
            v: SCHEMA_V,
            event: "completions_setup",
            ts: now_rfc3339(),
            shell,
            completion_path,
            completion_action,
            rc_path,
            rc_action,
            framework,
            hint,
        }
    }
}

/// Outcome of `--check-completions`: read-only status report.
#[derive(Debug, Serialize)]
pub struct CompletionsStatus<'a> {
    pub v: &'static str,
    pub event: &'static str,
    pub ts: String,
    pub shell: &'static str,
    /// `true` when SACS will load in a fresh shell session.
    pub wired: bool,
    /// `"wired" | "needs_rc_block" | "missing"` — same triage as
    /// the [`crate::completions::Status`] enum.
    pub status: &'static str,
    pub completion_path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rc_path: Option<&'a str>,
}

impl<'a> CompletionsStatus<'a> {
    pub fn new(
        shell: &'static str,
        wired: bool,
        status: &'static str,
        completion_path: &'a str,
        rc_path: Option<&'a str>,
    ) -> Self {
        Self {
            v: SCHEMA_V,
            event: "completions_status",
            ts: now_rfc3339(),
            shell,
            wired,
            status,
            completion_path,
            rc_path,
        }
    }
}

/// Serialize an event into a single NDJSON line **without** the
/// trailing newline. Callers append `\n` themselves before writing
/// to stdout — keeps the function pure for testing and lets the
/// writer batch lines (`writeln!`, `BufWriter`, etc.).
///
/// Returns an error only if the value is fundamentally
/// unserializable (cannot happen with the structs in this module),
/// so production code can `.expect()` safely.
pub fn serialize_line<T: Serialize>(event: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_epoch_zero() {
        assert_eq!(rfc3339_from_unix(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn rfc3339_known_dates() {
        // 1700000000 = 2023-11-14T22:13:20Z
        assert_eq!(rfc3339_from_unix(1_700_000_000), "2023-11-14T22:13:20Z");
        // 1747353600 = 2025-05-16T00:00:00Z (sanity check across
        // years and Feb-leap year handling — 2024 was a leap year)
        assert_eq!(rfc3339_from_unix(1_747_353_600), "2025-05-16T00:00:00Z");
    }

    #[test]
    fn rfc3339_format_is_strictly_padded() {
        // 86400 + 3600 + 60 + 1 = 90061 → 1970-01-02T01:01:01Z
        let s = rfc3339_from_unix(90_061);
        assert_eq!(s, "1970-01-02T01:01:01Z");
        // Verify the digit count is exactly 4-2-2T2-2-2Z.
        assert_eq!(s.len(), 20);
    }

    #[test]
    fn uploaded_serializes_with_v_event_ts_first() {
        let mut e = Uploaded::new("notes.md", 1234, Some(42), "work-nc", "nextcloud/zz-drop");
        e.ts = "2026-05-16T00:00:00Z".into();
        let s = serialize_line(&e).unwrap();
        assert_eq!(
            s,
            r#"{"v":"1","event":"uploaded","ts":"2026-05-16T00:00:00Z","file":"notes.md","bytes":1234,"compressed_pct":42,"alias":"work-nc","target":"nextcloud/zz-drop"}"#
        );
    }

    #[test]
    fn uploaded_omits_compressed_pct_when_none() {
        let mut e = Uploaded::new("notes.md", 1234, None, "work-nc", "nextcloud/zz-drop");
        e.ts = "2026-05-16T00:00:00Z".into();
        let s = serialize_line(&e).unwrap();
        assert!(!s.contains("compressed_pct"), "got: {s}");
    }

    #[test]
    fn failed_bare_serializes_minimal_fields() {
        let mut e = Failed::bare(Reason::Usage);
        e.ts = "2026-05-16T00:00:00Z".into();
        let s = serialize_line(&e).unwrap();
        assert_eq!(
            s,
            r#"{"v":"1","event":"failed","ts":"2026-05-16T00:00:00Z","reason":"usage"}"#
        );
    }

    #[test]
    fn failed_with_alias_ambiguous_includes_candidates() {
        let mut e = Failed::alias_ambiguous(vec!["work-nc", "home-gdrive"]);
        e.ts = "2026-05-16T00:00:00Z".into();
        let s = serialize_line(&e).unwrap();
        assert!(s.contains("\"reason\":\"alias_ambiguous\""), "got: {s}");
        assert!(s.contains("\"candidates\":[\"work-nc\",\"home-gdrive\"]"), "got: {s}");
    }

    #[test]
    fn batch_summary_serializes_all_counters() {
        let mut e = BatchSummary::new(12, 11, 1, 9);
        e.ts = "2026-05-16T00:00:00Z".into();
        let s = serialize_line(&e).unwrap();
        assert_eq!(
            s,
            r#"{"v":"1","event":"batch_summary","ts":"2026-05-16T00:00:00Z","total":12,"ok":11,"failed":1,"exit_code":9}"#
        );
    }

    #[test]
    fn locked_and_wiped_have_no_payload_fields() {
        let mut l = Locked::new();
        l.ts = "2026-05-16T00:00:00Z".into();
        assert_eq!(
            serialize_line(&l).unwrap(),
            r#"{"v":"1","event":"locked","ts":"2026-05-16T00:00:00Z"}"#
        );

        let mut w = Wiped::new();
        w.ts = "2026-05-16T00:00:00Z".into();
        assert_eq!(
            serialize_line(&w).unwrap(),
            r#"{"v":"1","event":"wiped","ts":"2026-05-16T00:00:00Z"}"#
        );
    }

    #[test]
    fn doctor_check_optional_detail() {
        let mut c = DoctorCheck::new("provider_reachable", false, Some("dns_timeout"));
        c.ts = "2026-05-16T00:00:00Z".into();
        let s = serialize_line(&c).unwrap();
        assert_eq!(
            s,
            r#"{"v":"1","event":"doctor_check","ts":"2026-05-16T00:00:00Z","name":"provider_reachable","ok":false,"detail":"dns_timeout"}"#
        );

        let mut ok = DoctorCheck::new("agent_socket", true, None);
        ok.ts = "2026-05-16T00:00:00Z".into();
        let s = serialize_line(&ok).unwrap();
        assert!(!s.contains("detail"), "got: {s}");
    }

    #[test]
    fn serializer_escapes_control_chars_in_file_names() {
        // Injection safety: file names with embedded newline or
        // ANSI escape must not produce multi-line or
        // terminal-controlling output.
        let mut e = Uploaded::new("evil\nname.\x1b[31m", 0, None, "a", "t");
        e.ts = "2026-05-16T00:00:00Z".into();
        let s = serialize_line(&e).unwrap();
        // Exactly one line.
        assert_eq!(s.lines().count(), 1, "got: {s}");
        // No raw control bytes survive into the serialized form.
        for b in s.bytes() {
            assert!(
                (0x20..0x7f).contains(&b) || b == b'\t',
                "raw control byte 0x{b:02x} in {s}"
            );
        }
        // Both escapes present (`\n` for newline, `\u001b` for ESC).
        assert!(s.contains(r#"evil\nname.\u001b[31m"#), "got: {s}");
    }
}
