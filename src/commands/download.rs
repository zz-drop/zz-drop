use std::path::{Path, PathBuf};

use zz_drop_core::PlainProfile;
use zz_drop_core::crypto::compression::{decompress, is_tar_ustar, is_zstd_magic};

use super::batch::BatchSummary;
use super::remote_fs::{RemoteError, RemoteFs};
use super::walk::{SkipReason, split_user_path};
use crate::color::ColorPolicy;
use crate::output::{self, TargetLabel};

const ZSTD_SUFFIX: &str = ".zst";
/// Suffix appended to the decompressed sibling file when the
/// remote name doesn't end in `.zst`. Rare in practice (the
/// upload pipeline always produces `.zst` for compressed
/// blobs) but possible if the operator uploaded a `.zst` from
/// outside zz-drop.
const DECOMPRESSED_FALLBACK_SUFFIX: &str = ".dec";

pub fn run_download<R: RemoteFs>(
    remote: &R,
    files: &[String],
    dest_dir: &Path,
    profile: &PlainProfile,
    color: &ColorPolicy,
    decompress_flag: bool,
) -> i32 {
    let mut summary = BatchSummary::default();
    let target = output::profile_target(profile);
    let scope = TargetLabel {
        alias: profile.alias.as_str(),
        target: &target,
    };

    for f in files {
        // Mirror the local-glob ergonomics of `zz s a*` (where
        // zsh expands the pattern against the local filesystem
        // before invoking us): when the operator passes a remote
        // glob to `zz d`, expand it server-side against a
        // listing of the target directory. The shell can't help
        // here — its glob engine only sees the local fs.
        if has_glob(f) {
            match expand_remote_glob(remote, f) {
                Ok(matches) if matches.is_empty() => {
                    output::err_line(&output::render_failed(
                        f,
                        "no remote matches",
                        Some(scope),
                        color,
                    ));
                    summary.record_failure();
                }
                Ok(matches) => {
                    for arg in matches {
                        download_one(
                            remote,
                            &arg,
                            dest_dir,
                            scope,
                            color,
                            &mut summary,
                            decompress_flag,
                        );
                    }
                }
                Err(e) => {
                    output::err_line(&output::render_failed(
                        f,
                        &format!("{e}"),
                        Some(scope),
                        color,
                    ));
                    summary.record_failure();
                }
            }
        } else {
            download_one(
                remote,
                f,
                dest_dir,
                scope,
                color,
                &mut summary,
                decompress_flag,
            );
        }
    }

    summary.exit_code()
}

/// True when the argument carries glob metacharacters supported
/// by the v1 remote-glob expansion. We accept `*` and `?` only;
/// brackets / character classes / extended POSIX globs are not
/// supported and will be sent through literally.
fn has_glob(s: &str) -> bool {
    s.contains('*') || s.contains('?')
}

/// Expand a glob pattern against the target's parent directory.
/// Path-segment globs (a `*` *before* the last `/`) are not
/// supported in v1 — the input is passed through literally so
/// the existing per-file error path reports it. Directory
/// matches are filtered out: `zz d` operates on files.
fn expand_remote_glob<R: RemoteFs>(
    remote: &R,
    arg: &str,
) -> Result<Vec<String>, RemoteError> {
    let (parent, pattern) = match arg.rfind('/') {
        Some(i) => (&arg[..i], &arg[i + 1..]),
        None => ("", arg),
    };
    if has_glob(parent) {
        // No path-segment globbing in v1.
        return Ok(vec![arg.to_string()]);
    }
    let parent_segs: Vec<&str> = parent.split('/').filter(|s| !s.is_empty()).collect();
    let entries = remote.list(&parent_segs)?;
    let mut matches: Vec<String> = entries
        .into_iter()
        .filter(|e| !e.is_directory)
        .filter(|e| glob_match(pattern, &e.name))
        .map(|e| {
            if parent.is_empty() {
                e.name
            } else {
                format!("{parent}/{}", e.name)
            }
        })
        .collect();
    matches.sort();
    Ok(matches)
}

/// Minimal shell-style glob matcher: `*` matches any sequence
/// (including empty), `?` matches one character, every other
/// byte must match literally. Recursive — fine for filenames.
fn glob_match(pattern: &str, name: &str) -> bool {
    fn rec(p: &[u8], n: &[u8]) -> bool {
        match (p.first(), n.first()) {
            (None, None) => true,
            (None, Some(_)) => false,
            (Some(b'*'), _) => rec(&p[1..], n) || (!n.is_empty() && rec(p, &n[1..])),
            (Some(b'?'), Some(_)) => rec(&p[1..], &n[1..]),
            (Some(b'?'), None) => false,
            (Some(c), Some(d)) if c == d => rec(&p[1..], &n[1..]),
            _ => false,
        }
    }
    rec(pattern.as_bytes(), name.as_bytes())
}

pub fn run_download_all<R: RemoteFs>(
    remote: &R,
    dest_dir: &Path,
    recursive: bool,
    profile: &PlainProfile,
    color: &ColorPolicy,
    decompress_flag: bool,
    src_remote: Option<&str>,
) -> i32 {
    if decompress_flag {
        // Bulk decompress (`dax` / `darx`) is paired with the
        // bundle-upload story; the v1 take is "use `dx <name>`
        // per file" since tar bundles are an obvious
        // single-blob shape. Refuse with a hint.
        output::err_line(
            "`x` on `da` / `dar` (bulk decompress) is coming in v1.1 — \
             for now use `dx <name>` per file, or drop the `x`.",
        );
        return crate::commands::EXIT_NOT_IMPLEMENTED;
    }
    let mut summary = BatchSummary::default();
    let target = output::profile_target(profile);
    let scope = TargetLabel {
        alias: profile.alias.as_str(),
        target: &target,
    };

    // `src_remote = Some("docs/sub")` → walk that subprefix
    // instead of the whole remote tree. Reuse the same segment
    // splitter as upload's prefix logic (in zz-drop/upload.rs)
    // so the convention stays consistent.
    let segments: Vec<String> = match src_remote {
        None => Vec::new(),
        Some(p) => p
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect(),
    };
    download_dir(remote, &segments, dest_dir, recursive, scope, color, &mut summary);

    summary.exit_code()
}

fn download_one<R: RemoteFs>(
    remote: &R,
    arg: &str,
    dest_dir: &Path,
    scope: TargetLabel<'_>,
    color: &ColorPolicy,
    summary: &mut BatchSummary,
    decompress_flag: bool,
) {
    let segments = match split_user_path(arg) {
        Ok(s) => s,
        Err(SkipReason::Dotfile) => {
            output::err_line(&output::render_failed(arg, "dotfile", Some(scope), color));
            summary.record_skip();
            return;
        }
        Err(_) => {
            output::err_line(&output::render_failed(arg, "invalid path", Some(scope), color));
            summary.record_failure();
            return;
        }
    };

    let basename = segments.last().expect("non-empty after split").clone();
    let dest = dest_dir.join(&basename);

    let segs: Vec<&str> = segments.iter().map(|s| s.as_str()).collect();
    match remote.download(&segs, &dest) {
        Ok(size) => {
            let s = output::human_size(size);
            output::line(&output::render_downloaded(&basename, &s, scope, color));
            summary.record_success();
            if decompress_flag {
                decompress_alongside(&dest, scope, color);
            }
        }
        Err(e) => {
            output::err_line(&output::render_failed(arg, &format!("{e}"), Some(scope), color));
            summary.record_failure();
        }
    }
}

/// Read the just-downloaded blob, peek at its first four bytes,
/// and (if the zstd magic is there) write the decompressed
/// payload to a sibling file — or, if the decompressed bytes
/// look like a tar archive, extract the archive into a sibling
/// directory. Non-destructive — the original `.zst` stays on
/// disk so the operator can re-decompress with any zstd CLI if
/// zz-drop ever goes away.
fn decompress_alongside(blob: &Path, scope: TargetLabel<'_>, color: &ColorPolicy) {
    let display = blob.display().to_string();
    let bytes = match std::fs::read(blob) {
        Ok(b) => b,
        Err(e) => {
            output::err_line(&output::render_failed(
                &display,
                &format!("read for decompress: {e}"),
                Some(scope),
                color,
            ));
            return;
        }
    };
    if !is_zstd_magic(&bytes) {
        output::err_line(&format!(
            "  · {} is not zstd-compressed; skipped decompress",
            blob.display()
        ));
        return;
    }
    let decoded = match decompress(&bytes) {
        Ok(d) => d,
        Err(e) => {
            output::err_line(&output::render_failed(
                &display,
                &format!("zstd decode: {e}"),
                Some(scope),
                color,
            ));
            return;
        }
    };

    // If the decompressed payload is a tar archive, extract it
    // into a sibling directory named after the bundle (without
    // the `.tar.zst` suffix). The bundle blob stays on disk.
    if is_tar_ustar(&decoded) {
        extract_tar_alongside(blob, &decoded, scope, color);
        return;
    }

    let out_path = decompressed_sibling_path(blob);
    if let Err(e) = std::fs::write(&out_path, &decoded) {
        output::err_line(&output::render_failed(
            &display,
            &format!("write {}: {e}", out_path.display()),
            Some(scope),
            color,
        ));
        return;
    }
    output::line(&format!(
        "  · decompressed to {} ({} bytes)",
        out_path.display(),
        decoded.len()
    ));
}

/// `<name>.tar.zst` → extract into a sibling directory `<name>/`.
/// Refuses if the target directory already exists, to avoid
/// silently overwriting the operator's files.
fn extract_tar_alongside(
    blob: &Path,
    tar_bytes: &[u8],
    scope: TargetLabel<'_>,
    color: &ColorPolicy,
) {
    let display = blob.display().to_string();
    let name = blob
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let stem = name
        .strip_suffix(".tar.zst")
        .or_else(|| name.strip_suffix(ZSTD_SUFFIX))
        .unwrap_or(name);
    let stem = if stem.is_empty() { "extracted" } else { stem };
    let dest = blob
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(stem);
    if dest.exists() {
        output::err_line(&format!(
            "  · refusing to extract: {} already exists",
            dest.display()
        ));
        return;
    }
    if let Err(e) = std::fs::create_dir(&dest) {
        output::err_line(&output::render_failed(
            &display,
            &format!("mkdir {}: {e}", dest.display()),
            Some(scope),
            color,
        ));
        return;
    }
    let mut archive = tar::Archive::new(tar_bytes);
    if let Err(e) = archive.unpack(&dest) {
        output::err_line(&output::render_failed(
            &display,
            &format!("untar into {}: {e}", dest.display()),
            Some(scope),
            color,
        ));
        return;
    }
    output::line(&format!(
        "  · extracted bundle into {}/",
        dest.display()
    ));
}

/// `file.md.zst` → `file.md`; `archive.tar.zst` → `archive.tar`
/// (only used when the decompressed payload turns out NOT to be
/// a tar — see `extract_tar_alongside` for the tar path);
/// `weirdname` (no `.zst` suffix) → `weirdname.dec`.
fn decompressed_sibling_path(blob: &Path) -> PathBuf {
    let name = blob
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let stripped = name.strip_suffix(ZSTD_SUFFIX);
    let new_name = match stripped {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => format!("{name}{DECOMPRESSED_FALLBACK_SUFFIX}"),
    };
    blob.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(new_name)
}

fn download_dir<R: RemoteFs>(
    remote: &R,
    remote_segments: &[String],
    local_dir: &Path,
    recursive: bool,
    scope: TargetLabel<'_>,
    color: &ColorPolicy,
    summary: &mut BatchSummary,
) {
    let segs: Vec<&str> = remote_segments.iter().map(|s| s.as_str()).collect();
    let entries = match remote.list(&segs) {
        Ok(v) => v,
        Err(e) => {
            let display = if remote_segments.is_empty() {
                "/".to_string()
            } else {
                remote_segments.join("/")
            };
            output::err_line(&output::render_failed(&display, &format!("{e}"), Some(scope), color));
            summary.record_failure();
            return;
        }
    };

    if let Err(e) = std::fs::create_dir_all(local_dir) {
        output::err_line(&format!(
            "could not create local dir {}: {e}",
            local_dir.display()
        ));
        summary.record_failure();
        return;
    }

    for entry in entries {
        if entry.is_directory {
            if !recursive {
                continue;
            }
            let mut child_remote = remote_segments.to_vec();
            child_remote.push(entry.name.clone());
            let child_local = local_dir.join(&entry.name);
            download_dir(
                remote,
                &child_remote,
                &child_local,
                recursive,
                scope,
                color,
                summary,
            );
        } else {
            let mut full = remote_segments.to_vec();
            full.push(entry.name.clone());
            let full_refs: Vec<&str> = full.iter().map(|s| s.as_str()).collect();
            let dest = local_dir.join(&entry.name);
            match remote.download(&full_refs, &dest) {
                Ok(size) => {
                    let s = output::human_size(size);
                    let display = full.join("/");
                    output::line(&output::render_downloaded(&display, &s, scope, color));
                    summary.record_success();
                }
                Err(e) => {
                    let display = full.join("/");
                    output::err_line(&output::render_failed(&display, &format!("{e}"), Some(scope), color));
                    summary.record_failure();
                }
            }
        }
    }
}

#[allow(dead_code)]
fn _unused(_: PathBuf) {}

#[cfg(test)]
mod tests {
    use super::{glob_match, has_glob};

    #[test]
    fn glob_match_handles_star() {
        assert!(glob_match("*", "anything.md"));
        assert!(glob_match("Q*", "Quectel.pdf"));
        assert!(glob_match("*.pdf", "report.pdf"));
        assert!(glob_match("Q*ec*.pdf", "Quectel.pdf"));
        assert!(!glob_match("Q*", "Bquectel.pdf"));
        assert!(!glob_match("*.pdf", "report.txt"));
    }

    #[test]
    fn glob_match_handles_question_mark() {
        assert!(glob_match("?ello", "hello"));
        assert!(glob_match("h?llo", "hello"));
        assert!(!glob_match("?ello", "hhello")); // ? matches exactly one
        assert!(!glob_match("?ello", "ello"));
    }

    #[test]
    fn glob_match_handles_literal() {
        assert!(glob_match("readme.md", "readme.md"));
        assert!(!glob_match("readme.md", "readme.txt"));
        assert!(!glob_match("readme.md", ""));
        assert!(glob_match("", ""));
    }

    #[test]
    fn glob_match_combines_star_and_question() {
        assert!(glob_match("Q*?.pdf", "Quectel.pdf"));
        assert!(!glob_match("Q*?.pdf", "Q.pdf")); // ? requires at least one char after *
    }

    #[test]
    fn has_glob_detects_metacharacters() {
        assert!(has_glob("Q*"));
        assert!(has_glob("*.pdf"));
        assert!(has_glob("h?llo"));
        assert!(has_glob("docs/Q*"));
        assert!(!has_glob("readme.md"));
        assert!(!has_glob("docs/readme.md"));
        assert!(!has_glob(""));
    }
}
