use std::path::{Path, PathBuf};

use zz_drop_core::crypto::compression::{
    COMPRESS_SKIP_THRESHOLD_BYTES, DEFAULT_COMPRESSION_LEVEL, compress,
};
use zz_drop_core::scriptable::Reason;
use zz_drop_core::{CollisionPolicy, PlainProfile};

use super::batch::BatchSummary;
use super::remote_fs::RemoteFs;
use super::walk::{LocalFile, walk_local};
use crate::color::ColorPolicy;
use crate::output::{self, TargetLabel};

const DEFAULT_POLICY: CollisionPolicy = CollisionPolicy::Rename;
/// Suffix appended to the cloud leaf when the `x` modifier is on
/// for single-file uploads. Standard zstd suffix; users who lose
/// zz-drop can still decompress with `zstd -d <name>.zst`.
const ZSTD_SUFFIX: &str = ".zst";

pub fn run_upload<R: RemoteFs>(
    remote: &R,
    files: &[PathBuf],
    profile: &PlainProfile,
    color: &ColorPolicy,
    compress_flag: bool,
    dest_remote: Option<&str>,
) -> i32 {
    let mut summary = BatchSummary::default();

    for file in files {
        upload_one(
            remote,
            file,
            profile,
            color,
            &mut summary,
            compress_flag,
            dest_remote,
        );
    }

    summary.emit_and_exit_code()
}

pub fn run_save_all<R: RemoteFs>(
    remote: &R,
    cwd: &Path,
    recursive: bool,
    profile: &PlainProfile,
    color: &ColorPolicy,
    compress_flag: bool,
    dest_remote: Option<&str>,
) -> i32 {
    if compress_flag {
        return run_save_all_bundle(remote, cwd, recursive, profile, color, dest_remote);
    }
    let entries = match walk_local(cwd, recursive) {
        Ok(v) => v,
        Err(e) => {
            output::emit_failed_bare(
                Reason::Usage,
                Some(&format!("could not walk {}: {e}", cwd.display())),
            );
            return crate::commands::EXIT_USAGE;
        }
    };

    if entries.is_empty() {
        if crate::runtime::flags().output == crate::runtime::OutputMode::Text {
            output::line("no files to upload");
        }
        return BatchSummary::default().emit_and_exit_code();
    }

    let mut summary = BatchSummary::default();
    for entry in entries {
        upload_walk_entry(remote, &entry, profile, color, &mut summary, dest_remote);
    }

    summary.emit_and_exit_code()
}

/// Bundle mode: tar all selected files into a single archive,
/// zstd-compress it, upload it as `<dirname>.tar.zst`. The
/// archive includes paths relative to the walk root, so
/// extracting on a fresh machine reproduces the source tree.
fn run_save_all_bundle<R: RemoteFs>(
    remote: &R,
    cwd: &Path,
    recursive: bool,
    profile: &PlainProfile,
    color: &ColorPolicy,
    dest_remote: Option<&str>,
) -> i32 {
    let target = output::profile_target(profile);
    let scope = TargetLabel {
        alias: profile.alias.as_str(),
        target: &target,
    };

    // Resolve the directory's display name. `zz sax .` should
    // produce a bundle named after the cwd, not a literal `.`.
    let canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let dir_name = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("snapshot")
        .to_string();
    let bundle_leaf = format!("{dir_name}.tar.zst");

    let entries = match walk_local(cwd, recursive) {
        Ok(v) => v,
        Err(e) => {
            output::emit_failed_bare(
                Reason::Usage,
                Some(&format!("could not walk {}: {e}", cwd.display())),
            );
            return crate::commands::EXIT_USAGE;
        }
    };
    if entries.is_empty() {
        if crate::runtime::flags().output == crate::runtime::OutputMode::Text {
            output::line("no files to bundle");
        }
        return BatchSummary::default().emit_and_exit_code();
    }

    // Build the tar archive in memory. Streaming-to-tempfile is
    // the obvious upgrade path; v1 caps the wizard guidance at
    // "small/medium archives" via the public docs.
    let mut tar_bytes: Vec<u8> = Vec::with_capacity(64 * 1024);
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        for entry in &entries {
            let archive_path = entry.relative_segments.join("/");
            if let Err(e) = builder.append_path_with_name(&entry.absolute, &archive_path) {
                output::emit_failed_file(
                    &archive_path,
                    Reason::Usage,
                    &format!("tar: {e}"),
                    scope,
                    color,
                );
                return crate::commands::EXIT_USAGE;
            }
        }
        if let Err(e) = builder.finish() {
            output::emit_failed_bare(Reason::Usage, Some(&format!("tar finalize: {e}")));
            return crate::commands::EXIT_USAGE;
        }
    }

    let compressed = match compress(&tar_bytes, DEFAULT_COMPRESSION_LEVEL) {
        Ok(b) => b,
        Err(e) => {
            output::emit_failed_bare(Reason::Usage, Some(&format!("zstd: {e}")));
            return crate::commands::EXIT_USAGE;
        }
    };

    // Stage to a sibling temp of `cwd` so the existing upload
    // helper (which expects a path on disk) can read it.
    let staging_parent = cwd.parent().unwrap_or(Path::new("."));
    let tmp_path = staging_parent.join(format!(".zz-{bundle_leaf}.tmp"));
    if let Err(e) = std::fs::write(&tmp_path, &compressed) {
        output::emit_failed_bare(Reason::Usage, Some(&format!("stage bundle: {e}")));
        return crate::commands::EXIT_USAGE;
    }

    // Prefix the bundle leaf with the optional remote sub-path:
    // `zz sax . backup/` → `<remote_root>/backup/<dirname>.tar.zst`.
    let mut segments = remote_prefix_segments(dest_remote);
    segments.push(bundle_leaf.clone());

    let pct: u32 = if tar_bytes.is_empty() {
        0
    } else {
        let ratio = compressed.len() as f64 / tar_bytes.len() as f64;
        let saved = (1.0 - ratio) * 100.0;
        saved.round().clamp(0.0, 99.0) as u32
    };

    let mut summary = BatchSummary::default();
    perform_upload(
        remote,
        &tmp_path,
        &segments,
        profile,
        color,
        &mut summary,
        Some(pct),
    );
    let _ = std::fs::remove_file(&tmp_path);

    if crate::runtime::flags().output == crate::runtime::OutputMode::Text {
        output::line(&format!(
            "  · bundled {} files into {}",
            entries.len(),
            bundle_leaf,
        ));
    }

    summary.emit_and_exit_code()
}

fn upload_one<R: RemoteFs>(
    remote: &R,
    local: &Path,
    profile: &PlainProfile,
    color: &ColorPolicy,
    summary: &mut BatchSummary,
    compress_flag: bool,
    dest_remote: Option<&str>,
) {
    let display = local.display().to_string();
    let target = output::profile_target(profile);
    let scope = TargetLabel {
        alias: profile.alias.as_str(),
        target: &target,
    };

    // Remote name is the local file's basename (single segment).
    let basename = match local.file_name().and_then(|n| n.to_str()) {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => {
            output::emit_failed_file(&display, Reason::Usage, "invalid path", scope, color);
            summary.record_failure();
            return;
        }
    };

    if basename.starts_with('.') {
        output::emit_failed_file(&display, Reason::Usage, "dotfile", scope, color);
        summary.record_skip();
        return;
    }

    let metadata = match local.symlink_metadata() {
        Ok(m) => m,
        Err(e) => {
            let detail = if e.kind() == std::io::ErrorKind::NotFound {
                "not found"
            } else {
                "io error"
            };
            output::emit_failed_file(&display, Reason::Usage, detail, scope, color);
            summary.record_failure();
            return;
        }
    };

    let ft = metadata.file_type();
    if ft.is_symlink() {
        output::emit_failed_file(&display, Reason::Usage, "symlink", scope, color);
        summary.record_skip();
        return;
    }
    if ft.is_dir() {
        output::emit_failed_file(&display, Reason::Usage, "is a directory", scope, color);
        summary.record_skip();
        return;
    }
    if !ft.is_file() {
        output::emit_failed_file(&display, Reason::Usage, "not a regular file", scope, color);
        summary.record_skip();
        return;
    }

    let mut segments = remote_prefix_segments(dest_remote);
    if compress_flag && (metadata.len() as usize) >= COMPRESS_SKIP_THRESHOLD_BYTES {
        upload_one_compressed(remote, local, &basename, profile, color, summary, &segments);
    } else {
        // Either compression wasn't asked, or the file is below
        // the skip threshold (zstd's frame header would make a
        // tiny payload bigger). Upload as-is.
        segments.push(basename);
        perform_upload(remote, local, &segments, profile, color, summary, None);
    }
}

/// Read `local`, zstd-encode the bytes, write them to a sibling
/// temp file, and feed that to the regular per-file upload path
/// with the leaf renamed to `<basename>.zst`. Temp file is
/// removed when the call returns.
fn upload_one_compressed<R: RemoteFs>(
    remote: &R,
    local: &Path,
    basename: &str,
    profile: &PlainProfile,
    color: &ColorPolicy,
    summary: &mut BatchSummary,
    prefix_segments: &[String],
) {
    let display = local.display().to_string();
    let target = output::profile_target(profile);
    let scope = TargetLabel {
        alias: profile.alias.as_str(),
        target: &target,
    };

    let plaintext = match std::fs::read(local) {
        Ok(b) => b,
        Err(e) => {
            output::emit_failed_file(&display, Reason::Usage, &format!("read: {e}"), scope, color);
            summary.record_failure();
            return;
        }
    };

    let compressed = match compress(&plaintext, DEFAULT_COMPRESSION_LEVEL) {
        Ok(b) => b,
        Err(e) => {
            output::emit_failed_file(&display, Reason::Usage, &format!("zstd: {e}"), scope, color);
            summary.record_failure();
            return;
        }
    };

    // Stage the compressed bytes in the same parent dir so the
    // RemoteFs upload helper can `read` it like any other file.
    let parent = local.parent().unwrap_or(Path::new("."));
    let tmp_path = parent.join(format!(".zz-{basename}.zst.tmp"));
    if let Err(e) = std::fs::write(&tmp_path, &compressed) {
        output::emit_failed_file(&display, Reason::Usage, &format!("stage: {e}"), scope, color);
        summary.record_failure();
        return;
    }

    // "N% compressed" reads naturally as "saved N%". Compute it
    // as 100 − (compressed/original × 100), rounded, clamped to
    // 0..=99. Already-compressed inputs (PNG, zip, etc.) land
    // near 0%; highly redundant text near 99%. zstd inflation
    // (rare, payload > original) clamps to 0%.
    let pct: u32 = if plaintext.is_empty() {
        0
    } else {
        let ratio = compressed.len() as f64 / plaintext.len() as f64;
        let saved = (1.0 - ratio) * 100.0;
        saved.round().clamp(0.0, 99.0) as u32
    };

    let remote_leaf = format!("{basename}{ZSTD_SUFFIX}");
    let mut segments: Vec<String> = prefix_segments.to_vec();
    segments.push(remote_leaf);
    perform_upload(remote, &tmp_path, &segments, profile, color, summary, Some(pct));

    // Best-effort cleanup; a leftover `.tmp` is annoying but not
    // a correctness problem (it'll be overwritten on the next
    // run with the same basename).
    let _ = std::fs::remove_file(&tmp_path);
}

fn upload_walk_entry<R: RemoteFs>(
    remote: &R,
    entry: &LocalFile,
    profile: &PlainProfile,
    color: &ColorPolicy,
    summary: &mut BatchSummary,
    dest_remote: Option<&str>,
) {
    let mut segments = remote_prefix_segments(dest_remote);
    segments.extend_from_slice(&entry.relative_segments);
    perform_upload(
        remote,
        &entry.absolute,
        &segments,
        profile,
        color,
        summary,
        None,
    );
}

/// Convert an optional `<remote-prefix>` (a slash-separated
/// path such as `backup/snap`) into individual segments, ready
/// to prepend to the per-file segment list. Empty / `None` →
/// empty Vec; the caller's `.push(...)` then puts the file at
/// `<remote_root>/...` as before.
fn remote_prefix_segments(dest_remote: Option<&str>) -> Vec<String> {
    match dest_remote {
        None => Vec::new(),
        Some(p) => p
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect(),
    }
}

fn perform_upload<R: RemoteFs>(
    remote: &R,
    local: &Path,
    segments: &[String],
    profile: &PlainProfile,
    color: &ColorPolicy,
    summary: &mut BatchSummary,
    compression_pct: Option<u32>,
) {
    let segs: Vec<&str> = segments.iter().map(|s| s.as_str()).collect();
    let display = segments.join("/");
    let target = output::profile_target(profile);
    let scope = TargetLabel {
        alias: profile.alias.as_str(),
        target: &target,
    };

    let policy = collision_policy_for(profile);

    match remote.upload(local, &segs, policy) {
        Ok(outcome) => {
            output::emit_uploaded(
                &outcome.final_name,
                outcome.size,
                compression_pct,
                scope,
                color,
            );
            if outcome.renamed && crate::runtime::flags().output == crate::runtime::OutputMode::Text {
                output::line(&format!("  (renamed from {display})"));
            }
            summary.record_success();
        }
        Err(e) => {
            output::emit_failed_file(
                &display,
                Reason::ProviderError,
                &format!("{e}"),
                scope,
                color,
            );
            summary.record_failure();
        }
    }
}

fn collision_policy_for(profile: &PlainProfile) -> CollisionPolicy {
    let _ = profile;
    DEFAULT_POLICY
}
