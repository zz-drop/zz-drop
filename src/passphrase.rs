//! Read a profile passphrase from a file with strict permission
//! and content checks. Used by `--passphrase-file` /
//! `ZZ_PASSPHRASE_FILE` in scriptable mode.
//!
//! The format is intentionally minimal: read the file verbatim,
//! strip exactly one trailing `\n` if present, return the rest.
//! No whitespace trimming, no comment lines, no quoting — what
//! you write is the passphrase.
//!
//! ## Threat model
//!
//! - The file must be a **regular file** (not a symlink, fifo,
//!   socket, device, or directory). We check with
//!   `symlink_metadata` so the path itself can't redirect us
//!   through a symlink to e.g. `/etc/shadow`.
//! - The file must be owned by the **current UID** — a passphrase
//!   file owned by root or by another user is rejected even when
//!   the current process can read it.
//! - The file mode must be `≤ 0600` (group + others = 0).
//! - We cap the read at 4 KiB; a passphrase file larger than
//!   that is almost certainly a configuration mistake, and the
//!   cap prevents accidentally reading a multi-MB file into RAM.
//! - The content must not embed a NUL byte — passphrases are
//!   plain UTF-8 in v1 and a NUL is almost certainly garbage.
//!
//! TOCTOU note: there is a small window between the metadata
//! check and the read; in v1 the attacker is assumed to control
//! their own home directory, so this window is not exploitable
//! by a separate user. A future hardening pass can switch to
//! `openat` with `O_NOFOLLOW` for race-free behavior.

use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

/// 4 KiB is a generous cap for a passphrase: the longest
/// realistic passphrase (a BIP39 24-word mnemonic with spaces)
/// stays under 200 bytes; anything beyond ~1 KiB is almost
/// certainly a misuse (e.g. the operator pointed `--passphrase-file`
/// at a `.zz` envelope or a binary).
pub const MAX_PASSPHRASE_FILE_BYTES: u64 = 4 * 1024;

#[derive(Debug)]
pub enum PassphraseFileError {
    /// `ENOENT` on the metadata read.
    NotFound { path: PathBuf },
    /// Path resolves to a non-regular file (symlink, fifo, dir,
    /// socket, device).
    NotRegular { path: PathBuf },
    /// `mode & 0o077 != 0` — at least one bit visible to group
    /// or others is set.
    InsecurePermissions { path: PathBuf, mode: u32 },
    /// File owner UID differs from the current process UID.
    OwnerMismatch {
        path: PathBuf,
        file_uid: u32,
        current_uid: u32,
    },
    /// File length exceeds [`MAX_PASSPHRASE_FILE_BYTES`].
    TooLarge { path: PathBuf, actual: u64 },
    /// File content contains an embedded NUL byte.
    EmbeddedNul { path: PathBuf },
    /// File is empty (or contains only a single trailing `\n`,
    /// which leaves nothing after stripping).
    Empty { path: PathBuf },
    /// Any other IO error from the metadata or read syscalls.
    Io { path: PathBuf, error: String },
}

impl std::fmt::Display for PassphraseFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { path } => write!(f, "passphrase file not found: {}", path.display()),
            Self::NotRegular { path } => write!(
                f,
                "passphrase file is not a regular file: {}",
                path.display()
            ),
            Self::InsecurePermissions { path, mode } => write!(
                f,
                "passphrase file {} has mode 0{:o} (must be 0600 or stricter)",
                path.display(),
                mode & 0o777
            ),
            Self::OwnerMismatch {
                path,
                file_uid,
                current_uid,
            } => write!(
                f,
                "passphrase file {} is owned by uid {} (current uid is {})",
                path.display(),
                file_uid,
                current_uid
            ),
            Self::TooLarge { path, actual } => write!(
                f,
                "passphrase file {} is {} bytes (max {})",
                path.display(),
                actual,
                MAX_PASSPHRASE_FILE_BYTES
            ),
            Self::EmbeddedNul { path } => write!(
                f,
                "passphrase file {} contains a NUL byte",
                path.display()
            ),
            Self::Empty { path } => {
                write!(f, "passphrase file {} is empty", path.display())
            }
            Self::Io { path, error } => {
                write!(f, "passphrase file {}: {error}", path.display())
            }
        }
    }
}

impl std::error::Error for PassphraseFileError {}

impl PassphraseFileError {
    /// True when the error is a permission / ownership violation
    /// (maps to `EXIT_PASSPHRASE_FILE_INSECURE` = 11). Other
    /// failures map to `EXIT_USAGE`.
    pub fn is_insecure(&self) -> bool {
        matches!(
            self,
            Self::InsecurePermissions { .. } | Self::OwnerMismatch { .. }
        )
    }
}

/// Read and validate a passphrase file. `current_uid` is injected
/// so unit tests can simulate owner-mismatch cases without
/// chowning anything (which requires root).
///
/// On success returns the file content with **at most one**
/// trailing `\n` stripped. The returned string is otherwise
/// verbatim — leading/trailing spaces, internal newlines, and
/// embedded UTF-8 are preserved.
#[cfg(unix)]
pub fn read_passphrase_file(
    path: &Path,
    current_uid: u32,
) -> Result<String, PassphraseFileError> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(PassphraseFileError::NotFound {
                path: path.to_path_buf(),
            });
        }
        Err(e) => {
            return Err(PassphraseFileError::Io {
                path: path.to_path_buf(),
                error: e.to_string(),
            });
        }
    };

    let ft = meta.file_type();
    if !ft.is_file() {
        return Err(PassphraseFileError::NotRegular {
            path: path.to_path_buf(),
        });
    }

    let mode = meta.mode();
    if mode & 0o077 != 0 {
        return Err(PassphraseFileError::InsecurePermissions {
            path: path.to_path_buf(),
            mode,
        });
    }

    let file_uid = meta.uid();
    if file_uid != current_uid {
        return Err(PassphraseFileError::OwnerMismatch {
            path: path.to_path_buf(),
            file_uid,
            current_uid,
        });
    }

    let len = meta.len();
    if len > MAX_PASSPHRASE_FILE_BYTES {
        return Err(PassphraseFileError::TooLarge {
            path: path.to_path_buf(),
            actual: len,
        });
    }

    // Read with a hard cap. `take` would also work but
    // `read_to_string` lets us reject non-UTF-8 cheaply.
    let raw_bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            return Err(PassphraseFileError::Io {
                path: path.to_path_buf(),
                error: e.to_string(),
            });
        }
    };
    // Defensive: a race could have grown the file between the
    // metadata check and the read. Re-cap.
    if raw_bytes.len() as u64 > MAX_PASSPHRASE_FILE_BYTES {
        return Err(PassphraseFileError::TooLarge {
            path: path.to_path_buf(),
            actual: raw_bytes.len() as u64,
        });
    }
    if raw_bytes.contains(&0u8) {
        return Err(PassphraseFileError::EmbeddedNul {
            path: path.to_path_buf(),
        });
    }

    let mut s = match String::from_utf8(raw_bytes) {
        Ok(s) => s,
        Err(_) => {
            return Err(PassphraseFileError::Io {
                path: path.to_path_buf(),
                error: "content is not valid UTF-8".to_string(),
            });
        }
    };

    // Strip exactly one trailing `\n` per the contract; do not
    // collapse multiple newlines.
    if s.ends_with('\n') {
        s.pop();
    }

    if s.is_empty() {
        return Err(PassphraseFileError::Empty {
            path: path.to_path_buf(),
        });
    }

    Ok(s)
}

/// Non-Unix stub. zz-drop is Unix-only in v1 (Cargo.toml has
/// macOS- and Linux-only target deps); this branch exists to keep
/// `cargo check` happy when contributors edit on Windows.
#[cfg(not(unix))]
pub fn read_passphrase_file(
    path: &Path,
    _current_uid: u32,
) -> Result<String, PassphraseFileError> {
    Err(PassphraseFileError::Io {
        path: path.to_path_buf(),
        error: "passphrase-file is only supported on Unix targets in v1".to_string(),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    /// Build a `tempdir`-rooted file with the given content and
    /// mode. Returns the path. Owner is whoever runs the test.
    fn write_with_mode(content: &[u8], mode: u32) -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pp");
        std::fs::write(&path, content).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        (dir, path)
    }

    fn current_uid() -> u32 {
        rustix::process::getuid().as_raw()
    }

    #[test]
    fn happy_path_strips_one_trailing_newline() {
        let (_dir, path) = write_with_mode(b"my-secret\n", 0o600);
        let got = read_passphrase_file(&path, current_uid()).unwrap();
        assert_eq!(got, "my-secret");
    }

    #[test]
    fn missing_trailing_newline_is_fine() {
        let (_dir, path) = write_with_mode(b"my-secret", 0o600);
        let got = read_passphrase_file(&path, current_uid()).unwrap();
        assert_eq!(got, "my-secret");
    }

    #[test]
    fn only_one_trailing_newline_is_stripped() {
        // Two trailing newlines: strip exactly one, the other
        // stays in the passphrase verbatim.
        let (_dir, path) = write_with_mode(b"my-secret\n\n", 0o600);
        let got = read_passphrase_file(&path, current_uid()).unwrap();
        assert_eq!(got, "my-secret\n");
    }

    #[test]
    fn internal_whitespace_preserved() {
        let (_dir, path) = write_with_mode(b"  spaces in passphrase  \n", 0o600);
        let got = read_passphrase_file(&path, current_uid()).unwrap();
        assert_eq!(got, "  spaces in passphrase  ");
    }

    #[test]
    fn empty_file_is_rejected() {
        let (_dir, path) = write_with_mode(b"", 0o600);
        match read_passphrase_file(&path, current_uid()).unwrap_err() {
            PassphraseFileError::Empty { .. } => {}
            other => panic!("expected Empty, got {other:?}"),
        }
    }

    #[test]
    fn file_with_only_newline_is_rejected_as_empty() {
        let (_dir, path) = write_with_mode(b"\n", 0o600);
        match read_passphrase_file(&path, current_uid()).unwrap_err() {
            PassphraseFileError::Empty { .. } => {}
            other => panic!("expected Empty, got {other:?}"),
        }
    }

    #[test]
    fn mode_0644_is_rejected_as_insecure() {
        let (_dir, path) = write_with_mode(b"secret", 0o644);
        match read_passphrase_file(&path, current_uid()).unwrap_err() {
            PassphraseFileError::InsecurePermissions { mode, .. } => {
                assert_eq!(mode & 0o777, 0o644);
            }
            other => panic!("expected InsecurePermissions, got {other:?}"),
        }
    }

    #[test]
    fn mode_0640_is_rejected_as_insecure() {
        // Group-readable counts too.
        let (_dir, path) = write_with_mode(b"secret", 0o640);
        match read_passphrase_file(&path, current_uid()).unwrap_err() {
            PassphraseFileError::InsecurePermissions { .. } => {}
            other => panic!("expected InsecurePermissions, got {other:?}"),
        }
    }

    #[test]
    fn mode_0400_is_accepted() {
        // Stricter than 0600 is fine.
        let (_dir, path) = write_with_mode(b"secret", 0o400);
        let got = read_passphrase_file(&path, current_uid()).unwrap();
        assert_eq!(got, "secret");
    }

    #[test]
    fn wrong_owner_is_rejected() {
        let (_dir, path) = write_with_mode(b"secret", 0o600);
        // Pretend a different uid is invoking us.
        let fake_uid = current_uid().wrapping_add(1);
        match read_passphrase_file(&path, fake_uid).unwrap_err() {
            PassphraseFileError::OwnerMismatch { file_uid, current_uid: cur, .. } => {
                assert_eq!(file_uid, current_uid());
                assert_eq!(cur, fake_uid);
            }
            other => panic!("expected OwnerMismatch, got {other:?}"),
        }
    }

    #[test]
    fn missing_file_is_not_found() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist");
        match read_passphrase_file(&path, current_uid()).unwrap_err() {
            PassphraseFileError::NotFound { .. } => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn symlink_target_is_rejected_even_when_target_is_safe() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::write(&real, b"secret").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        match read_passphrase_file(&link, current_uid()).unwrap_err() {
            PassphraseFileError::NotRegular { .. } => {}
            other => panic!("expected NotRegular for symlink, got {other:?}"),
        }
    }

    #[test]
    fn directory_is_rejected() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("subdir");
        std::fs::create_dir(&sub).unwrap();
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o700)).unwrap();
        match read_passphrase_file(&sub, current_uid()).unwrap_err() {
            PassphraseFileError::NotRegular { .. } => {}
            other => panic!("expected NotRegular for directory, got {other:?}"),
        }
    }

    #[test]
    fn too_large_file_is_rejected() {
        let big = vec![b'x'; (MAX_PASSPHRASE_FILE_BYTES + 1) as usize];
        let (_dir, path) = write_with_mode(&big, 0o600);
        match read_passphrase_file(&path, current_uid()).unwrap_err() {
            PassphraseFileError::TooLarge { actual, .. } => {
                assert_eq!(actual, MAX_PASSPHRASE_FILE_BYTES + 1);
            }
            other => panic!("expected TooLarge, got {other:?}"),
        }
    }

    #[test]
    fn embedded_nul_is_rejected() {
        let (_dir, path) = write_with_mode(b"head\0tail", 0o600);
        match read_passphrase_file(&path, current_uid()).unwrap_err() {
            PassphraseFileError::EmbeddedNul { .. } => {}
            other => panic!("expected EmbeddedNul, got {other:?}"),
        }
    }

    #[test]
    fn non_utf8_is_rejected_as_io() {
        let (_dir, path) = write_with_mode(&[0xff, 0xfe, 0xfd], 0o600);
        match read_passphrase_file(&path, current_uid()).unwrap_err() {
            PassphraseFileError::Io { error, .. } => {
                assert!(error.contains("UTF-8"), "got: {error}");
            }
            other => panic!("expected Io(UTF-8), got {other:?}"),
        }
    }

    #[test]
    fn is_insecure_classifies_permission_errors_only() {
        let perm = PassphraseFileError::InsecurePermissions {
            path: PathBuf::from("/x"),
            mode: 0o644,
        };
        assert!(perm.is_insecure());

        let owner = PassphraseFileError::OwnerMismatch {
            path: PathBuf::from("/x"),
            file_uid: 0,
            current_uid: 1000,
        };
        assert!(owner.is_insecure());

        let missing = PassphraseFileError::NotFound {
            path: PathBuf::from("/x"),
        };
        assert!(!missing.is_insecure());
    }

    #[test]
    fn display_does_not_include_file_content() {
        // Defensive: error formatting must never leak file
        // content. Build every variant that *could* know
        // anything content-shaped and check.
        let cases: [Box<PassphraseFileError>; 5] = [
            Box::new(PassphraseFileError::NotFound { path: "/p".into() }),
            Box::new(PassphraseFileError::NotRegular { path: "/p".into() }),
            Box::new(PassphraseFileError::InsecurePermissions {
                path: "/p".into(),
                mode: 0o644,
            }),
            Box::new(PassphraseFileError::TooLarge {
                path: "/p".into(),
                actual: 5000,
            }),
            Box::new(PassphraseFileError::Empty { path: "/p".into() }),
        ];
        for err in cases {
            let s = format!("{err}");
            assert!(!s.contains("\0"), "{s}");
            assert!(!s.to_lowercase().contains("secret"), "{s}");
        }
    }
}
