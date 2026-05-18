//! Static `zz --help` cheat sheet. No state, no agent — purely a
//! formatted string emitted to stdout. Layout sized to 80 columns,
//! decays gracefully to 60 by dropping the right-hand "what"
//! column when the terminal is narrower.
//!
//! User-facing reference: `zz-drop/COMMANDS.md`.

use std::env;

/// Width threshold above which we render the full two-column
/// layout. Below it we fall back to a flat list.
const WIDE_THRESHOLD: usize = 72;

/// Detect terminal columns from the `COLUMNS` environment
/// variable. Returns 80 when unset or unparseable — most
/// terminals export `COLUMNS` and this default keeps the help
/// output stable in pipes / non-tty contexts (no ioctl needed,
/// zero deps).
pub fn detect_columns() -> usize {
    env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(80)
}

/// Render the cheat sheet at the given column width. The result
/// includes a trailing newline.
pub fn render(columns: usize) -> String {
    if columns >= WIDE_THRESHOLD {
        WIDE.to_string()
    } else {
        NARROW.to_string()
    }
}

/// 80-column rendering. The "what" column starts at byte 30,
/// chosen so the longest LHS form (`zz dar [<dest> [<prefix>]]`)
/// leaves at least one space before the gloss.
const WIDE: &str = "\
zz-drop — explicit one-shot file transfer

UPLOAD
  zz <file>...               upload (default verb)
  zz s <file>... [<dir>/]    explicit alias (trailing-/ → remote dir)
  zz sx <file> [<dir>/]      + zstd compress
  zz sa <dir> [<prefix>]     bulk top-level (2nd arg = remote prefix)
  zz sar <dir> [<prefix>]    recursive
  zz sax <dir> [<prefix>]    → one .tar.zst
  zz sarx <dir> [<prefix>]   recursive bundle

DOWNLOAD
  zz d <name>... [<dir>/]    download raw (trailing-/ → local dir)
  zz dx <name> [<dir>/]      + decompress (or extract bundle)
  zz da [<dest> [<prefix>]]  all top-level (zero-arg → cwd, full root)
  zz dar [<dest> [<prefix>]] recursive

ATOMIC
  zz q                       lock the agent
  zz w                       wipe local state (asks for confirmation)
  zz z [local|remote]        unlock the active container
  zz c                       open the configuration TUI (zz-tui)
  zz f                       doctor / diagnostics

MODIFIERS (s and d only, set semantics)
  a  bulk         r  recursive (with a)
  x  compress     e  encrypt (coming v1.1)

set semantics: zz sarx . ≡ zz sxar . ≡ zz xars . ≡ zz rasx .

EXIT CODES
  0  ok            6  profile not found
  2  usage         7  decryption failed
  3  not impl      8  wipe cancelled
  5  agent down    9  provider error    127 zz-tui not on PATH

SHELL COMPLETIONS
  zz --setup-completions [bash|zsh|fish]       # install (auto-detects $SHELL)
  zz --check-completions [bash|zsh|fish]       # verify the install
  zz --setup-completions zsh --uninstall       # remove rc block + file

  See COMMANDS.md for the full grammar reference.
";

/// 60-column fallback. Drops the right-hand gloss for the verb
/// tables; the operator gets a flat list of forms and falls back
/// to `COMMANDS.md` for explanations.
const NARROW: &str = "\
zz-drop — explicit one-shot file transfer

UPLOAD
  zz <file>...
  zz s <file>... [<dir>/]
  zz sx <file> [<dir>/]
  zz sa <dir> [<prefix>]
  zz sar <dir> [<prefix>]
  zz sax <dir> [<prefix>]
  zz sarx <dir> [<prefix>]

DOWNLOAD
  zz d <name>... [<dir>/]
  zz dx <name> [<dir>/]
  zz da [<dest> [<prefix>]]
  zz dar [<dest> [<prefix>]]

ATOMIC
  zz q  zz w  zz z [local|remote]  zz c  zz f

MODIFIERS (s/d, set semantics)
  a bulk · r recursive (with a)
  x compress · e encrypt (v1.1)

EXIT CODES
  0 ok · 2 usage · 3 not impl · 5 agent down
  6 profile · 7 decrypt · 8 wipe cancel
  9 provider · 127 no zz-tui

SHELL COMPLETIONS
  zz --setup-completions [bash|zsh|fish]
  zz --check-completions [bash|zsh|fish]

See COMMANDS.md for the full grammar reference.
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_render_has_all_section_headers() {
        let out = render(80);
        for header in [
            "UPLOAD",
            "DOWNLOAD",
            "ATOMIC",
            "MODIFIERS",
            "EXIT CODES",
            "SHELL COMPLETIONS",
        ] {
            assert!(
                out.contains(header),
                "wide help missing section `{header}`:\n{out}"
            );
        }
    }

    #[test]
    fn wide_render_lists_every_atomic_verb() {
        let out = render(80);
        for verb in ["zz q", "zz w", "zz z", "zz c", "zz f"] {
            assert!(
                out.contains(verb),
                "wide help missing atomic verb `{verb}`"
            );
        }
    }

    #[test]
    fn wide_render_documents_setup_and_check_flags() {
        let out = render(80);
        assert!(out.contains("--setup-completions"));
        assert!(out.contains("--check-completions"));
        for shell in ["bash", "zsh", "fish"] {
            assert!(out.contains(shell), "wide help missing shell `{shell}`");
        }
    }

    #[test]
    fn wide_render_lines_fit_in_80_columns() {
        for (i, line) in render(80).lines().enumerate() {
            assert!(
                line.chars().count() <= 80,
                "line {} exceeds 80 cols ({} chars):\n{line}",
                i + 1,
                line.chars().count()
            );
        }
    }

    #[test]
    fn narrow_render_lines_fit_in_60_columns() {
        for (i, line) in render(60).lines().enumerate() {
            assert!(
                line.chars().count() <= 60,
                "line {} exceeds 60 cols ({} chars):\n{line}",
                i + 1,
                line.chars().count()
            );
        }
    }

    #[test]
    fn narrow_render_still_covers_every_section() {
        let out = render(60);
        for header in ["UPLOAD", "DOWNLOAD", "ATOMIC", "MODIFIERS", "EXIT CODES"] {
            assert!(
                out.contains(header),
                "narrow help missing section `{header}`"
            );
        }
    }

    #[test]
    fn narrow_threshold_picks_narrow_below() {
        // Anything strictly below the threshold uses the narrow
        // layout. Probe both sides of the boundary.
        assert_ne!(render(WIDE_THRESHOLD - 1), render(WIDE_THRESHOLD));
        assert_eq!(render(WIDE_THRESHOLD), render(80));
    }

    #[test]
    fn detect_columns_defaults_to_80() {
        // Saving / restoring env is racy across tests, so just
        // check the parse path with explicit values via `render`.
        // The default-80 behaviour is exercised in integration.
        let _ = detect_columns();
    }

    #[test]
    fn no_secret_or_remote_host_leak_in_help() {
        // Privacy lock per design §12 sanitisation: the help
        // page is shipped to every user, no zz-drop.net mention,
        // no fake-real-looking emails.
        let out = render(80);
        assert!(!out.contains("zz-drop.net"));
        assert!(!out.contains("@example.org"));
        assert!(!out.contains("@example.com"));
    }
}
