//! Plaintext sidecar files that hint `zz z` no-args at the previously
//! selected default profile.
//!
//! These files are written by zz-drop after a successful unlock-with-
//! picker, and read on cold start. They are deliberately plaintext —
//! aliases are operator-chosen mnemonics (decision log 2026-05-02
//! container close-out, resolution #4) — but every read path treats
//! them as untrusted input and falls back silently to the interactive
//! picker on any failure mode.
//!
//! Format:
//! - `last-default-local`: one line, `<alias>\n`
//! - `last-default-remote`: two lines, `<email>\n<alias>\n`. Email-only
//!   (one line) is also acceptable on read; alias-only is not (the
//!   email is needed to authenticate before the alias is meaningful).
//!
//! Constraints:
//! - max 256 bytes total
//! - chmod 0600 on Unix
//! - alias: printable ASCII, no NUL, no `/`, no `..`, 1–64 chars
//! - email: contains `@`, no whitespace/control chars, length ≤ 254

use std::path::Path;

/// Hard cap on a sidecar file's total size; anything larger is
/// treated as garbage and the picker takes over.
pub const MAX_SIDECAR_BYTES: usize = 256;

/// Maximum length of an operator-chosen alias.
pub const MAX_ALIAS_LEN: usize = 64;

/// Per RFC 5321: the local part is up to 64, the domain up to 253, so
/// the full address is at most 254 ASCII chars.
pub const MAX_EMAIL_LEN: usize = 254;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalDefault {
    pub alias: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteDefault {
    pub email: String,
    /// `None` when the sidecar carries only the email (the alias was
    /// never selected, or got dropped on a previous read because the
    /// container no longer contained it).
    pub alias: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidecarReadError {
    /// File doesn't exist on disk; cold start.
    NotFound,
    /// File exists but is malformed (oversized, control chars, wrong
    /// line count, charset rejection, etc.). On this branch the
    /// caller falls back to the interactive picker; a warning to
    /// stderr is appropriate.
    Invalid,
    /// I/O failure (permissions, etc.).
    Io,
}

/// Read `last-default-local`, returning the cached alias if the file
/// is well-formed.
pub fn read_local_default(path: &Path) -> Result<LocalDefault, SidecarReadError> {
    let raw = read_capped(path)?;
    let mut lines = raw.split('\n');
    let alias = lines.next().unwrap_or("");
    let trailing = lines.next().unwrap_or("");
    // Allow at most a single trailing empty line (the `\n` terminator).
    if !trailing.is_empty() || lines.next().is_some() {
        return Err(SidecarReadError::Invalid);
    }
    if !validate_alias(alias) {
        return Err(SidecarReadError::Invalid);
    }
    Ok(LocalDefault {
        alias: alias.to_string(),
    })
}

/// Read `last-default-remote`. Both lines must validate; if only the
/// first (email) is present, returns the email with `alias = None`
/// so the caller can prompt only the picker.
pub fn read_remote_default(path: &Path) -> Result<RemoteDefault, SidecarReadError> {
    let raw = read_capped(path)?;
    let mut lines = raw.split('\n');
    let email = lines.next().unwrap_or("");
    let alias = lines.next().unwrap_or("");
    let trailing = lines.next().unwrap_or("");
    // Up to one trailing empty line.
    if !trailing.is_empty() || lines.next().is_some() {
        return Err(SidecarReadError::Invalid);
    }
    if !validate_email(email) {
        return Err(SidecarReadError::Invalid);
    }
    let alias = if alias.is_empty() {
        None
    } else if validate_alias(alias) {
        Some(alias.to_string())
    } else {
        return Err(SidecarReadError::Invalid);
    };
    Ok(RemoteDefault {
        email: email.to_string(),
        alias,
    })
}

/// Write `last-default-local`. Creates parent dirs, sets mode 0600
/// on Unix. Truncates any previous content.
pub fn write_local_default(path: &Path, alias: &str) -> std::io::Result<()> {
    if !validate_alias(alias) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "alias rejected",
        ));
    }
    write_atomic(path, format!("{alias}\n").as_bytes())
}

/// Write `last-default-remote`. `alias` is optional — passing `None`
/// stores email-only (useful between sign-in and first picker
/// confirmation).
pub fn write_remote_default(
    path: &Path,
    email: &str,
    alias: Option<&str>,
) -> std::io::Result<()> {
    if !validate_email(email) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "email rejected",
        ));
    }
    let body = match alias {
        Some(a) => {
            if !validate_alias(a) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "alias rejected",
                ));
            }
            format!("{email}\n{a}\n")
        }
        None => format!("{email}\n"),
    };
    write_atomic(path, body.as_bytes())
}

/// Charset rules for an alias mnemonic. Printable ASCII, no NUL, no
/// `/`, no `..`, 1..=`MAX_ALIAS_LEN`.
pub fn validate_alias(s: &str) -> bool {
    if s.is_empty() || s.len() > MAX_ALIAS_LEN {
        return false;
    }
    if s == "." || s == ".." || s.contains("..") {
        return false;
    }
    if s.contains('/') || s.contains('\0') || s.contains('\\') {
        return false;
    }
    s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
}

/// Email shape rules. RFC 5321 length only — full RFC 5322 grammar is
/// out of scope; the server's auth flow is the real validator.
pub fn validate_email(s: &str) -> bool {
    if s.is_empty() || s.len() > MAX_EMAIL_LEN {
        return false;
    }
    if !s.contains('@') {
        return false;
    }
    // No whitespace, no control chars, no NUL.
    s.chars().all(|c| !c.is_whitespace() && !c.is_control() && c != '\0')
}

fn read_capped(path: &Path) -> Result<String, SidecarReadError> {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(SidecarReadError::NotFound);
        }
        Err(_) => return Err(SidecarReadError::Io),
    };
    if metadata.len() as usize > MAX_SIDECAR_BYTES {
        return Err(SidecarReadError::Invalid);
    }
    let bytes = std::fs::read(path).map_err(|_| SidecarReadError::Io)?;
    if bytes.len() > MAX_SIDECAR_BYTES {
        return Err(SidecarReadError::Invalid);
    }
    // Reject any control char other than `\n` outright.
    for &b in &bytes {
        if b == b'\n' {
            continue;
        }
        if b < 0x20 || b == 0x7f {
            return Err(SidecarReadError::Invalid);
        }
    }
    String::from_utf8(bytes).map_err(|_| SidecarReadError::Invalid)
}

fn write_atomic(path: &Path, body: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Write to a tmp sibling and rename, so a crash can't leave a
    // truncated sidecar — readers always see either the old content
    // or the new content, never a partial write.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&tmp, perms)?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // ── alias validation ─────────────────────────────────────────

    #[test]
    fn alias_accepts_normal_mnemonics() {
        assert!(validate_alias("nc-casa"));
        assert!(validate_alias("gdrive-work"));
        assert!(validate_alias("a"));
        assert!(validate_alias("Halvdan_was_here"));
        assert!(validate_alias("alpha.beta")); // single dot is ok
    }

    #[test]
    fn alias_rejects_pathy_strings() {
        assert!(!validate_alias("a/b"));
        assert!(!validate_alias("a\\b"));
        assert!(!validate_alias(".."));
        assert!(!validate_alias("."));
        assert!(!validate_alias("a..b"));
        assert!(!validate_alias("../b"));
    }

    #[test]
    fn alias_rejects_control_and_nul() {
        assert!(!validate_alias("a\0b"));
        assert!(!validate_alias("a\nb"));
        assert!(!validate_alias("a\tb"));
    }

    #[test]
    fn alias_rejects_empty_and_overlong() {
        assert!(!validate_alias(""));
        let long = "a".repeat(MAX_ALIAS_LEN + 1);
        assert!(!validate_alias(&long));
        let max = "a".repeat(MAX_ALIAS_LEN);
        assert!(validate_alias(&max));
    }

    // ── email validation ─────────────────────────────────────────

    #[test]
    fn email_accepts_basic_shapes() {
        assert!(validate_email("user@example.com"));
        assert!(validate_email("a@b"));
        assert!(validate_email("user+tag@sub.example.org"));
    }

    #[test]
    fn email_rejects_missing_at() {
        assert!(!validate_email(""));
        assert!(!validate_email("noatsign"));
    }

    #[test]
    fn email_rejects_whitespace_and_control() {
        assert!(!validate_email("a@b c"));
        assert!(!validate_email("a@b\nc"));
        assert!(!validate_email("a@b\tc"));
        assert!(!validate_email("a@b\0c"));
    }

    #[test]
    fn email_rejects_overlong() {
        let local = "a".repeat(64);
        let domain = "b".repeat(MAX_EMAIL_LEN); // overshoot
        let bad = format!("{local}@{domain}");
        assert!(!validate_email(&bad));
    }

    // ── round-trip ──────────────────────────────────────────────

    #[test]
    fn local_round_trip_one_line() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last-default-local");
        write_local_default(&path, "nc-casa").unwrap();
        let got = read_local_default(&path).unwrap();
        assert_eq!(got.alias, "nc-casa");
    }

    #[test]
    fn remote_round_trip_two_lines() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last-default-remote");
        write_remote_default(&path, "user@example.org", Some("gdrive-work")).unwrap();
        let got = read_remote_default(&path).unwrap();
        assert_eq!(got.email, "user@example.org");
        assert_eq!(got.alias.as_deref(), Some("gdrive-work"));
    }

    #[test]
    fn remote_round_trip_email_only() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last-default-remote");
        write_remote_default(&path, "user@example.org", None).unwrap();
        let got = read_remote_default(&path).unwrap();
        assert_eq!(got.email, "user@example.org");
        assert!(got.alias.is_none());
    }

    // ── read failure modes ──────────────────────────────────────

    #[test]
    fn read_missing_file_returns_not_found() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nope");
        assert_eq!(
            read_local_default(&path),
            Err(SidecarReadError::NotFound)
        );
    }

    #[test]
    fn read_oversized_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last-default-local");
        std::fs::write(&path, "a".repeat(MAX_SIDECAR_BYTES + 1)).unwrap();
        assert_eq!(
            read_local_default(&path),
            Err(SidecarReadError::Invalid)
        );
    }

    #[test]
    fn read_with_control_chars_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last-default-local");
        std::fs::write(&path, "valid-prefix\x01garbage\n").unwrap();
        assert_eq!(
            read_local_default(&path),
            Err(SidecarReadError::Invalid)
        );
    }

    #[test]
    fn read_with_extra_lines_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last-default-local");
        std::fs::write(&path, "alias\nextra-junk\n").unwrap();
        assert_eq!(
            read_local_default(&path),
            Err(SidecarReadError::Invalid)
        );
    }

    #[test]
    fn read_with_path_separator_in_alias_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last-default-local");
        std::fs::write(&path, "../etc/passwd\n").unwrap();
        assert_eq!(
            read_local_default(&path),
            Err(SidecarReadError::Invalid)
        );
    }

    #[test]
    fn remote_with_invalid_email_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last-default-remote");
        std::fs::write(&path, "no-at-sign\nalias\n").unwrap();
        assert_eq!(
            read_remote_default(&path),
            Err(SidecarReadError::Invalid)
        );
    }

    #[test]
    fn remote_with_invalid_alias_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last-default-remote");
        std::fs::write(&path, "user@x.org\n../etc\n").unwrap();
        assert_eq!(
            read_remote_default(&path),
            Err(SidecarReadError::Invalid)
        );
    }

    // ── write rejects + permissions ─────────────────────────────

    #[test]
    fn write_local_rejects_invalid_alias() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last-default-local");
        let err = write_local_default(&path, "../bad").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!path.exists());
    }

    #[test]
    fn write_remote_rejects_invalid_email() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last-default-remote");
        let err = write_remote_default(&path, "no-at", Some("alias")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn write_sets_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("last-default-local");
        write_local_default(&path, "alias").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
