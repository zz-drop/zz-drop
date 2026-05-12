//! Filename rename helper for the `Rename` collision policy.
//!
//! On collision the upload pipeline re-tries with progressively
//! disambiguated filenames. The disambiguator is an ISO-8601
//! *basic* UTC timestamp (`YYYYMMDDTHHMMSSZ`) inserted before
//! the extension:
//!
//! - `n = 0` → original name unchanged.
//! - `n = 1` → `foo-20260503T103017Z.md`.
//! - `n ≥ 2` → `foo-20260503T103017Z-2.md` (rare — same-second
//!   collision; the timestamp resolution is one second so two
//!   uploads landing within the same wall-clock second need the
//!   trailing counter to disambiguate).
//!
//! ISO-8601 basic was picked over the older `foo (1).md`
//! Windows-style suffix because:
//! - filenames sort chronologically by ASCII,
//! - no `:` (illegal on Windows filesystems, awkward on macOS),
//! - timezone-unambiguous (the trailing `Z` pins it to UTC),
//! - readable a year later — the operator can tell when each
//!   collision happened without opening the file.
//!
//! Files starting with a single dot (e.g. `.bashrc`) are treated
//! as having no extension.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Compute a renamed filename for the `n`-th collision attempt.
/// `n = 0` returns the original name unchanged.
pub fn rename_with_suffix(original: &str, n: u32) -> String {
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    rename_with_suffix_at(original, n, now_secs)
}

/// Like [`rename_with_suffix`] but takes an explicit epoch (in
/// seconds) — the seam used by unit tests so the suffix is
/// deterministic.
pub fn rename_with_suffix_at(original: &str, n: u32, epoch_secs: u64) -> String {
    if n == 0 {
        return original.to_string();
    }
    let timestamp = iso_basic_utc(epoch_secs);
    let suffix = if n == 1 {
        format!("-{timestamp}")
    } else {
        format!("-{timestamp}-{n}")
    };

    let path = Path::new(original);
    let file_stem = path.file_stem().and_then(|s| s.to_str());
    let ext = path.extension().and_then(|s| s.to_str());

    match (file_stem, ext) {
        (Some(stem), Some(e)) => format!("{stem}{suffix}.{e}"),
        (Some(stem), None) => format!("{stem}{suffix}"),
        (None, _) => format!("{original}{suffix}"),
    }
}

/// Format an epoch-seconds value as ISO-8601 basic UTC:
/// `YYYYMMDDTHHMMSSZ`. Pure function. Date math via Howard
/// Hinnant's `civil_from_days` algorithm — exact for any
/// representable epoch.
fn iso_basic_utc(epoch_secs: u64) -> String {
    let s = (epoch_secs % 60) as u32;
    let mi = ((epoch_secs / 60) % 60) as u32;
    let h = ((epoch_secs / 3600) % 24) as u32;
    let days = (epoch_secs / 86_400) as i64;

    // Days since 1970-01-01 → (year, month, day) with March
    // anchoring the year so leap-day handling falls out of the
    // month-shift.
    let z = days + 719_468;
    let era = if z >= 0 {
        z / 146_097
    } else {
        (z - 146_096) / 146_097
    };
    let doe = (z - era * 146_097) as u64; // [0, 146_096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let mut year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    if month <= 2 {
        year += 1;
    }

    format!(
        "{year:04}{month:02}{day:02}T{h:02}{mi:02}{s:02}Z",
        year = year as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2024-05-03T07:00:00Z — picked because it's a sane recent
    /// date that exercises the leap-year math (2024 is a leap
    /// year) and the post-March month branch.
    const KNOWN_EPOCH: u64 = 1_714_719_600;

    #[test]
    fn iso_basic_utc_formats_known_epoch() {
        assert_eq!(iso_basic_utc(KNOWN_EPOCH), "20240503T070000Z");
    }

    #[test]
    fn iso_basic_utc_formats_unix_zero() {
        assert_eq!(iso_basic_utc(0), "19700101T000000Z");
    }

    #[test]
    fn iso_basic_utc_formats_far_future() {
        // 2100-01-01T00:00:00Z (NOT a leap year — 100 % 400 ≠ 0)
        let epoch = 4_102_444_800;
        assert_eq!(iso_basic_utc(epoch), "21000101T000000Z");
    }

    #[test]
    fn iso_basic_utc_formats_leap_day() {
        // 2024-02-29T12:34:56Z
        let epoch = 1_709_210_096;
        assert_eq!(iso_basic_utc(epoch), "20240229T123456Z");
    }

    #[test]
    fn n_zero_returns_unchanged() {
        assert_eq!(rename_with_suffix_at("AGENTS.md", 0, KNOWN_EPOCH), "AGENTS.md");
        assert_eq!(rename_with_suffix_at("Makefile", 0, KNOWN_EPOCH), "Makefile");
        assert_eq!(rename_with_suffix_at(".bashrc", 0, KNOWN_EPOCH), ".bashrc");
    }

    #[test]
    fn n_one_inserts_iso_basic_timestamp_before_extension() {
        assert_eq!(
            rename_with_suffix_at("AGENTS.md", 1, KNOWN_EPOCH),
            "AGENTS-20240503T070000Z.md"
        );
        assert_eq!(
            rename_with_suffix_at("photo.jpeg", 1, KNOWN_EPOCH),
            "photo-20240503T070000Z.jpeg"
        );
    }

    #[test]
    fn n_one_handles_extensionless_files() {
        assert_eq!(
            rename_with_suffix_at("Makefile", 1, KNOWN_EPOCH),
            "Makefile-20240503T070000Z"
        );
    }

    #[test]
    fn n_one_treats_dotfile_as_extensionless() {
        assert_eq!(
            rename_with_suffix_at(".bashrc", 1, KNOWN_EPOCH),
            ".bashrc-20240503T070000Z"
        );
    }

    #[test]
    fn n_two_or_more_appends_counter_after_timestamp() {
        // The `-2` / `-3` / … suffix only kicks in if the same
        // basename collides twice within the same wall-clock
        // second. Real-world rare; locked here so it exists.
        assert_eq!(
            rename_with_suffix_at("AGENTS.md", 2, KNOWN_EPOCH),
            "AGENTS-20240503T070000Z-2.md"
        );
        assert_eq!(
            rename_with_suffix_at("AGENTS.md", 7, KNOWN_EPOCH),
            "AGENTS-20240503T070000Z-7.md"
        );
    }

    #[test]
    fn double_extension_keeps_last_segment_as_extension() {
        // `Path::file_stem` strips only the last `.` — that
        // matches the expected behavior for `archive.tar.gz`
        // → stem `archive.tar`, ext `gz`.
        assert_eq!(
            rename_with_suffix_at("archive.tar.gz", 1, KNOWN_EPOCH),
            "archive.tar-20240503T070000Z.gz"
        );
    }

    #[test]
    fn timestamp_string_is_sixteen_chars_so_filenames_sort_chronologically() {
        // Fixed-width timestamp: ASCII-sort of two collisions
        // of the same basename gives chronological order.
        assert_eq!(iso_basic_utc(KNOWN_EPOCH).len(), 16);
        assert_eq!(iso_basic_utc(0).len(), 16);
    }
}
