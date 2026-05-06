pub mod parser;

pub use parser::{ParseError, parse_args};

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `compress = true` ↔ the `x` modifier appeared in the
    /// token (e.g. `zz sx file.md`). The upload pipeline wraps
    /// the bytes in a zstd frame and renames the leaf to
    /// `<name>.zst` on the cloud.
    ///
    /// `dest_remote = Some(prefix)` ↔ the operator typed a
    /// trailing-`/` argument as the last positional, e.g.
    /// `zz s a.md b.md docs/` → files land at
    /// `<remote_root>/docs/{a.md,b.md}`.
    Upload {
        files: Vec<PathBuf>,
        compress: bool,
        dest_remote: Option<String>,
    },
    /// `dir` is the local source directory (typed by the
    /// operator: `zz sa .` for cwd, or any path). All top-level
    /// regular files inside become uploads. Hidden files
    /// (`.bashrc`, `.git/`) are skipped per the existing rules.
    ///
    /// `dest_remote = Some(prefix)` ↔ optional 2nd positional
    /// `<remote-prefix>` (`zz sa /tmp/proj backup/`).
    SaveAll {
        compress: bool,
        dir: PathBuf,
        dest_remote: Option<String>,
    },
    /// Like [`SaveAll`] but recurses into subdirectories,
    /// preserving relative path segments on the remote side.
    SaveAllRecursive {
        compress: bool,
        dir: PathBuf,
        dest_remote: Option<String>,
    },
    /// `decompress = true` ↔ the `x` modifier appeared in the
    /// token (e.g. `zz dx file.md.zst`). The downloaded blob is
    /// kept on disk under its remote name AND the decompressed
    /// payload (or extracted tar tree) is produced alongside.
    ///
    /// `dest_local = Some(dir)` ↔ the operator typed a
    /// trailing-`/` argument as the last positional
    /// (`zz d api.md guide.md ./out/`); files land at
    /// `<dest_local>/{api.md,guide.md}` instead of cwd.
    Download {
        files: Vec<String>,
        decompress: bool,
        dest_local: Option<PathBuf>,
    },
    /// Walk the remote root (or a sub-prefix) and download each
    /// top-level file into the local destination directory.
    ///
    /// Both fields are optional positional arguments, in this
    /// order on the command line:
    /// 1. `dest_local` (1st arg, optional) — defaults to cwd.
    /// 2. `src_remote` (2nd arg, optional) — defaults to root.
    ///
    /// Example: `zz da backup docs/` → only `docs/` top-level
    /// files into `cwd/backup`.
    DownloadAll {
        decompress: bool,
        dest_local: Option<PathBuf>,
        src_remote: Option<String>,
    },
    DownloadAllRecursive {
        decompress: bool,
        dest_local: Option<PathBuf>,
        src_remote: Option<String>,
    },
    /// `zz z [local|remote]` — unlock a profile container, then pick
    /// the inner profile to use as the active default. `which =
    /// None` means "use the cached default if any, otherwise prompt
    /// local/remote".
    ContainerUnlock { which: Option<ContainerSource> },
    Lock,
    Wipe,
    OpenTui,
    Doctor,
}

/// Which container `zz z` should unlock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerSource {
    Local,
    Remote,
}
