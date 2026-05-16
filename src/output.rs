use std::io::Write;

use zz_drop_core::output::json as jsonev;
use zz_drop_core::scriptable::Reason;
use zz_drop_core::{PlainProfile, ProviderProfile};

use crate::color::ColorPolicy;
use crate::runtime::{self, OutputMode};

// Plain print helpers (kept for backward compat with x/q/w handlers).

pub fn line(text: &str) {
    println!("{text}");
}

pub fn err_line(text: &str) {
    eprintln!("{text}");
}

// --------------------------------------------------------------------
// Mode-aware emitters
// --------------------------------------------------------------------
//
// Every CLI result goes through one of these. They pick a
// representation based on `runtime::flags().output`:
//
// - `Text` (default): the human-friendly rendering produced by the
//   `render_*` functions below, with ANSI color where appropriate.
// - `Quiet`: one minimal text line per result (no ANSI, no banner).
// - `Json`: a single NDJSON record on stdout, schema in
//   `zz_drop_core::output::json`.
//
// In `Json` mode stdout is reserved for NDJSON. Errors that happen
// before the JSON path is set up (parse failure, init failure)
// still hit stderr — the per-command emitters here only fire on
// well-formed input.

/// Serialize an event and write it as a single line to stdout
/// with a trailing `\n`. Broken pipe is swallowed silently — the
/// caller of a `zz | head` pipeline is allowed to close early.
fn write_json_line<T: zz_drop_core::output::Serialize>(event: &T) {
    let s = match jsonev::serialize_line(event) {
        Ok(s) => s,
        // The event types in core/output/json.rs are all
        // structs of owned/borrowed primitives — serialization
        // cannot fail. If it ever did, we fall back to silence
        // rather than panic, so the script exit code stays the
        // authoritative signal.
        Err(_) => return,
    };
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let _ = writeln!(handle, "{s}");
}

/// Successful upload of one file.
pub fn emit_uploaded(
    name: &str,
    bytes: u64,
    compressed_pct: Option<u32>,
    scope: TargetLabel<'_>,
    color: &ColorPolicy,
) {
    match runtime::flags().output {
        OutputMode::Text => {
            let size = human_size(bytes);
            line(&render_uploaded(name, &size, compressed_pct, scope, color));
        }
        OutputMode::Quiet => line(name),
        OutputMode::Json => write_json_line(&jsonev::Uploaded::new(
            name,
            bytes,
            compressed_pct,
            scope.alias,
            scope.target,
        )),
    }
}

/// Successful download of one file.
pub fn emit_downloaded(name: &str, bytes: u64, scope: TargetLabel<'_>, color: &ColorPolicy) {
    match runtime::flags().output {
        OutputMode::Text => {
            let size = human_size(bytes);
            line(&render_downloaded(name, &size, scope, color));
        }
        OutputMode::Quiet => line(name),
        OutputMode::Json => write_json_line(&jsonev::Downloaded::new(
            name,
            bytes,
            scope.alias,
            scope.target,
        )),
    }
}

/// Per-file failure inside an unlocked session. `detail` carries
/// the free-form cause string; the structured `reason` is the
/// stable schema field.
pub fn emit_failed_file(
    name: &str,
    reason: Reason,
    detail: &str,
    scope: TargetLabel<'_>,
    color: &ColorPolicy,
) {
    match runtime::flags().output {
        OutputMode::Text => err_line(&render_failed(name, detail, Some(scope), color)),
        OutputMode::Quiet => err_line(&format!("failed {name}: {detail}")),
        OutputMode::Json => write_json_line(&jsonev::Failed::for_file(
            name,
            reason,
            Some(detail),
            scope.alias,
            scope.target,
        )),
    }
}

/// Bare failure with no per-file context (pre-unlock errors,
/// usage failures). `detail` is optional — pass `None` when the
/// reason already says everything.
pub fn emit_failed_bare(reason: Reason, detail: Option<&str>) {
    match runtime::flags().output {
        OutputMode::Text => match detail {
            Some(d) => err_line(d),
            None => err_line(reason.as_str()),
        },
        OutputMode::Quiet => match detail {
            Some(d) => err_line(&format!("failed: {d}")),
            None => err_line(&format!("failed: {}", reason.as_str())),
        },
        OutputMode::Json => {
            let event = match detail {
                Some(d) => jsonev::Failed::with_detail(reason, d),
                None => jsonev::Failed::bare(reason),
            };
            write_json_line(&event);
        }
    }
}

/// `alias_ambiguous` shorthand: only emits a JSON record in JSON
/// mode; text/quiet just print the hint with the candidate list.
pub fn emit_failed_alias_ambiguous(candidates: Vec<&str>) {
    match runtime::flags().output {
        OutputMode::Text | OutputMode::Quiet => {
            err_line(&format!(
                "alias ambiguous; choose one of: {}",
                candidates.join(", ")
            ));
        }
        OutputMode::Json => {
            write_json_line(&jsonev::Failed::alias_ambiguous(candidates));
        }
    }
}

/// Final record of a batch operation.
pub fn emit_batch_summary(total: u32, ok: u32, failed: u32, exit_code: i32) {
    if runtime::flags().output == OutputMode::Json {
        write_json_line(&jsonev::BatchSummary::new(total, ok, failed, exit_code));
    }
    // Text and Quiet have no batch_summary surface today — the
    // per-file lines convey everything the operator needs.
}

/// Successful `zz z` unlock.
pub fn emit_unlocked(alias: &str, target: &str) {
    match runtime::flags().output {
        OutputMode::Text => line(&format!("unlocked · {alias} · {target}")),
        OutputMode::Quiet => line(&format!("unlocked {alias}")),
        OutputMode::Json => write_json_line(&jsonev::Unlocked::new(alias, target)),
    }
}

/// Successful `zz q` lock. `idempotent = true` when the agent
/// wasn't running and nothing actually changed — useful for
/// human-readable text mode, invisible to the JSON schema which
/// emits the same `locked` event either way.
pub fn emit_locked(idempotent: bool) {
    match runtime::flags().output {
        OutputMode::Text => line(if idempotent { "already locked" } else { "locked" }),
        OutputMode::Quiet => line("locked"),
        OutputMode::Json => write_json_line(&jsonev::Locked::new()),
    }
}

/// Successful `zz w` wipe.
pub fn emit_wiped() {
    match runtime::flags().output {
        OutputMode::Text => line("wiped"),
        OutputMode::Quiet => line("wiped"),
        OutputMode::Json => write_json_line(&jsonev::Wiped::new()),
    }
}

/// Single `zz f` probe result.
pub fn emit_doctor_check(name: &str, ok: bool, detail: Option<&str>) {
    match runtime::flags().output {
        OutputMode::Text => {
            let status = if ok { "ok" } else { "fail" };
            match detail {
                Some(d) => line(&format!("  {name:<22} {status:<4} {d}")),
                None => line(&format!("  {name:<22} {status}")),
            }
        }
        OutputMode::Quiet => {
            let status = if ok { "ok" } else { "fail" };
            line(&format!("{name} {status}"));
        }
        OutputMode::Json => {
            write_json_line(&jsonev::DoctorCheck::new(name, ok, detail));
        }
    }
}

/// Final `zz f` summary.
pub fn emit_doctor_summary(ok: bool, failed: Vec<&str>) {
    match runtime::flags().output {
        OutputMode::Text => {
            if ok {
                line("doctor: all checks passed");
            } else {
                line(&format!("doctor: failed checks: {}", failed.join(", ")));
            }
        }
        OutputMode::Quiet => {
            line(if ok { "ok" } else { "fail" });
        }
        OutputMode::Json => {
            write_json_line(&jsonev::DoctorSummary::new(ok, failed));
        }
    }
}

// Pure render functions — easy to snapshot-test, no I/O.

/// Where this command is hitting: `alias · host/root`. Carried
/// alongside every `uploaded` / `downloaded` / `failed` line so the
/// operator always sees which destination they touched (the active
/// profile may be `profile-local.zz` or `profile-remote.zz`, and a
/// single account can have several aliases on the server).
#[derive(Clone, Copy)]
pub struct TargetLabel<'a> {
    pub alias: &'a str,
    pub target: &'a str,
}

/// `compression_pct = Some(N)` appends ` (N% compressed)` after
/// the size — the convention is "saved N% of the original size"
/// (so 90% means a ten-to-one ratio, larger = more compressed,
/// 0% means zstd added overhead and we ate the loss). `None`
/// skips the suffix entirely (no compression in this upload).
pub fn render_uploaded(
    name: &str,
    size: &str,
    compression_pct: Option<u32>,
    scope: TargetLabel<'_>,
    c: &ColorPolicy,
) -> String {
    let prefix = c.green("uploaded");
    let alias = c.cyan(scope.alias);
    let comp = match compression_pct {
        Some(p) => format!(" ({p}% compressed)"),
        None => String::new(),
    };
    format!(
        "{prefix} {name} {size}{comp} → {alias} · {target}",
        target = scope.target
    )
}

pub fn render_downloaded(name: &str, size: &str, scope: TargetLabel<'_>, c: &ColorPolicy) -> String {
    let prefix = c.green("downloaded");
    let alias = c.cyan(scope.alias);
    format!("{prefix} {name} {size} ← {alias} · {target}", target = scope.target)
}

pub fn render_failed(
    name: &str,
    reason: &str,
    scope: Option<TargetLabel<'_>>,
    c: &ColorPolicy,
) -> String {
    let prefix = c.red("failed");
    match scope {
        Some(s) => {
            let alias = c.cyan(s.alias);
            format!(
                "{prefix} {name} {reason} ({alias} · {target})",
                target = s.target
            )
        }
        None => format!("{prefix} {name} {reason}"),
    }
}

/// Compact "where am I writing" string for a profile:
/// `host/root` (e.g. `cloud.example.org/zz-drop`). Strips URL scheme
/// and trims slashes. Returns `"—"` if the profile has no provider.
pub fn profile_target(profile: &PlainProfile) -> String {
    match profile.providers.first() {
        Some(ProviderProfile::Nextcloud(nc)) => {
            let host = nc
                .server_url
                .strip_prefix("https://")
                .or_else(|| nc.server_url.strip_prefix("http://"))
                .unwrap_or(&nc.server_url)
                .trim_end_matches('/');
            let root = nc.remote_root.trim_matches('/');
            if root.is_empty() {
                host.to_string()
            } else {
                format!("{host}/{root}")
            }
        }
        Some(ProviderProfile::GoogleDrive(gd)) => {
            let root = gd.root_folder.trim_matches('/');
            if root.is_empty() {
                "gdrive".to_string()
            } else {
                format!("gdrive/{root}")
            }
        }
        Some(ProviderProfile::OneDrive(od)) => {
            let root = od.root_folder.trim_matches('/');
            if root.is_empty() {
                "onedrive".to_string()
            } else {
                format!("onedrive/{root}")
            }
        }
        Some(ProviderProfile::Dropbox(db)) => {
            let root = db.root_folder.trim_matches('/');
            if root.is_empty() {
                "dropbox".to_string()
            } else {
                format!("dropbox/{root}")
            }
        }
        None => "—".to_string(),
    }
}

pub fn render_hint(command: &str) -> String {
    format!("run: {command}")
}

/// Format a `zz ls` row. Path is wrapped in single quotes per SPEC.
pub fn render_list_entry(alias: &str, size: &str, path: &str) -> String {
    format!("{alias:<10} {size:>6}  '{path}'")
}

/// Binary-prefix size formatting:
/// - `< 1024 B` → `"<n> B"`
/// - `< 10` of the next prefix → one decimal (`"1.5 KiB"`)
/// - otherwise integer (`"12 KiB"`, `"345 MiB"`)
pub fn human_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    const TIB: u64 = GIB * 1024;

    if bytes < KIB {
        format!("{bytes} B")
    } else if bytes < MIB {
        let v = bytes as f64 / KIB as f64;
        if v >= 10.0 {
            format!("{} KiB", bytes / KIB)
        } else {
            format!("{v:.1} KiB")
        }
    } else if bytes < GIB {
        let v = bytes as f64 / MIB as f64;
        if v >= 10.0 {
            format!("{} MiB", bytes / MIB)
        } else {
            format!("{v:.1} MiB")
        }
    } else if bytes < TIB {
        let v = bytes as f64 / GIB as f64;
        if v >= 10.0 {
            format!("{} GiB", bytes / GIB)
        } else {
            format!("{v:.1} GiB")
        }
    } else {
        let v = bytes as f64 / TIB as f64;
        format!("{v:.1} TiB")
    }
}
