use zz_drop_core::{PlainProfile, ProviderProfile};

use crate::color::ColorPolicy;

// Plain print helpers (kept for backward compat with x/q/w handlers).

pub fn line(text: &str) {
    println!("{text}");
}

pub fn err_line(text: &str) {
    eprintln!("{text}");
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
