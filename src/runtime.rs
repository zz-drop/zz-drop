//! Process-wide global state for scriptable mode.
//!
//! The CLI accepts a small set of *global* flags (`--json`,
//! `--quiet`, `--passphrase-file`, `--alias`, `--local`, `--remote`,
//! `--yes`) and *environment overrides* (`ZZ_OUTPUT`,
//! `ZZ_PASSPHRASE_FILE`, `ZZ_ALIAS`, `ZZ_CONTAINER`). Both are
//! parsed once at startup and read by command code through
//! [`flags`].
//!
//! Precedence (highest first):
//!
//! 1. explicit command-line flag (`--json`, `--alias foo`, …)
//! 2. environment variable (`ZZ_OUTPUT=json`, …)
//! 3. default (text output, no passphrase file, no alias preset)
//!
//! Global flags are accepted only **before** the first verb /
//! positional argument, optionally terminated by `--`. This is
//! consistent and grep-friendly; scripts pipe-friendly defaults
//! by setting `ZZ_OUTPUT=json` once and writing terse verbs after.

use std::path::PathBuf;
use std::sync::OnceLock;

use crate::cli::ContainerSource;

/// What the CLI's stdout looks like for this invocation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OutputMode {
    /// Human-friendly text with ANSI color when a TTY is detected.
    /// The default — preserves the "mano sinistra" experience.
    #[default]
    Text,
    /// NDJSON, one record per line, stable schema. Stderr carries
    /// only fatal pre-output errors (panic, init failure).
    Json,
    /// Single minimal text line per result (no banners, no ANSI).
    Quiet,
}

impl OutputMode {
    pub fn is_json(self) -> bool {
        matches!(self, Self::Json)
    }
    pub fn is_quiet(self) -> bool {
        matches!(self, Self::Quiet)
    }
    /// True when output is intended for a script: no prompts, no
    /// auto-unlock, no picker.
    pub fn is_non_interactive(self) -> bool {
        matches!(self, Self::Json | Self::Quiet)
    }
}

/// Aggregated global state for one CLI run. Constructed once in
/// `main` and frozen via [`init`].
#[derive(Clone, Debug, Default)]
pub struct GlobalFlags {
    pub output: OutputMode,
    /// Path to a file the unlock flow reads instead of prompting.
    /// Format / permission rules are enforced by the reader, not
    /// here.
    pub passphrase_file: Option<PathBuf>,
    /// Pre-selected alias for the active session. Empty strings
    /// are rejected at parse time; the value is otherwise treated
    /// as opaque and matched verbatim against the alias list from
    /// the container.
    pub alias: Option<String>,
    /// Force the unlock flow to operate on the local or remote
    /// container instead of the default discovery.
    pub container: Option<ContainerSource>,
    /// Auto-confirm prompts (currently `zz w` only). In scriptable
    /// mode the wipe rejects without `--yes` regardless.
    pub yes: bool,
}

static GLOBAL: OnceLock<GlobalFlags> = OnceLock::new();

/// Install the parsed global state. Idempotent on the first call;
/// subsequent calls are ignored (the runtime is one-shot per
/// process).
pub fn init(flags: GlobalFlags) {
    let _ = GLOBAL.set(flags);
}

/// Read-only access to the installed global state. Returns the
/// default (Text mode, no overrides) if [`init`] was never called
/// — useful for unit tests that exercise command code without a
/// `main` boot.
pub fn flags() -> &'static GlobalFlags {
    GLOBAL.get_or_init(GlobalFlags::default)
}

// ---------------------------------------------------------------
// argv pre-pass: extract global flags from the front of argv.
// ---------------------------------------------------------------

/// Errors specific to the global-flag pre-pass. Distinct from
/// [`crate::cli::ParseError`] so the verb parser stays untouched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlagError {
    /// `--quiet` and `--json` are mutually exclusive.
    QuietJsonConflict,
    /// `--passphrase-file` / `--alias` need a value.
    MissingValue { flag: &'static str },
    /// Empty value (e.g. `--alias=` or `--alias ""`).
    EmptyValue { flag: &'static str },
    /// `--local` / `--remote` both supplied, or both supplied with
    /// `ZZ_CONTAINER`.
    ContainerConflict,
    /// Unrecognised `--name` at the global-flag position.
    UnknownFlag { token: String },
    /// `ZZ_OUTPUT` value other than `text` / `json`.
    BadEnvValue { var: &'static str, value: String },
}

impl std::fmt::Display for FlagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QuietJsonConflict => {
                f.write_str("--quiet and --json cannot be combined")
            }
            Self::MissingValue { flag } => {
                write!(f, "{flag} requires a value")
            }
            Self::EmptyValue { flag } => {
                write!(f, "{flag} value cannot be empty")
            }
            Self::ContainerConflict => f.write_str(
                "--local and --remote are mutually exclusive (also conflicts with ZZ_CONTAINER)",
            ),
            Self::UnknownFlag { token } => {
                write!(f, "unknown global flag `{token}` (use `--` to terminate flag parsing)")
            }
            Self::BadEnvValue { var, value } => {
                write!(f, "{var}=`{value}` not recognised (expected one of the documented values)")
            }
        }
    }
}

impl std::error::Error for FlagError {}

/// Walk the front of `argv` consuming recognised global flags.
/// Stops at the first non-flag token (verb / path) or at the
/// `--` terminator (which is consumed). Returns the parsed flags
/// plus the residual argv that the verb parser sees.
///
/// Does NOT consult env vars. Use [`merge_env`] on the result to
/// apply the precedence chain.
pub fn extract_flags(argv: Vec<String>) -> Result<(GlobalFlags, Vec<String>), FlagError> {
    let mut flags = GlobalFlags::default();
    let mut iter = argv.into_iter();
    let mut residual: Vec<String> = Vec::new();

    while let Some(tok) = iter.next() {
        // Bare `--` terminates global-flag parsing; everything
        // after it is verbatim positional.
        if tok == "--" {
            residual.extend(iter.by_ref());
            break;
        }
        // First non-flag token is the start of the verb / path
        // sequence. Push it back and bail.
        if !tok.starts_with("--") {
            residual.push(tok);
            residual.extend(iter.by_ref());
            break;
        }

        // From here on, tok is `--name` or `--name=value`. We
        // split on `=` against a local owned copy so that the
        // owned `tok` stays free for moving into the error path
        // if the name isn't recognised.
        let head = tok.clone();
        let (name, inline_value) = split_eq(&head);

        match name {
            "--json" => {
                reject_inline("--json", inline_value)?;
                if flags.output == OutputMode::Quiet {
                    return Err(FlagError::QuietJsonConflict);
                }
                flags.output = OutputMode::Json;
            }
            "--quiet" => {
                reject_inline("--quiet", inline_value)?;
                if flags.output == OutputMode::Json {
                    return Err(FlagError::QuietJsonConflict);
                }
                flags.output = OutputMode::Quiet;
            }
            "--passphrase-file" => {
                let v = take_value("--passphrase-file", inline_value, &mut iter)?;
                flags.passphrase_file = Some(PathBuf::from(v));
            }
            "--alias" => {
                let v = take_value("--alias", inline_value, &mut iter)?;
                flags.alias = Some(v);
            }
            "--local" => {
                reject_inline("--local", inline_value)?;
                if matches!(flags.container, Some(ContainerSource::Remote)) {
                    return Err(FlagError::ContainerConflict);
                }
                flags.container = Some(ContainerSource::Local);
            }
            "--remote" => {
                reject_inline("--remote", inline_value)?;
                if matches!(flags.container, Some(ContainerSource::Local)) {
                    return Err(FlagError::ContainerConflict);
                }
                flags.container = Some(ContainerSource::Remote);
            }
            "--yes" => {
                reject_inline("--yes", inline_value)?;
                flags.yes = true;
            }
            _ => {
                return Err(FlagError::UnknownFlag { token: tok });
            }
        }
    }

    Ok((flags, residual))
}

/// Apply env-var overrides to flags that were not set explicitly
/// on the command line. Precedence: flag > env > default.
///
/// `lookup` returns the string value for an env var name, or
/// `None` if unset / empty. Production code passes
/// [`env_lookup_os`] (which reads `std::env`); tests pass a
/// deterministic in-memory closure, so unit tests never touch the
/// process env (which is global and racy under multi-threaded
/// test runs).
pub fn merge_env<F: Fn(&str) -> Option<String>>(
    mut flags: GlobalFlags,
    explicit_output: bool,
    explicit_passphrase: bool,
    explicit_alias: bool,
    explicit_container: bool,
    lookup: F,
) -> Result<GlobalFlags, FlagError> {
    if !explicit_output {
        if let Some(s) = lookup("ZZ_OUTPUT") {
            flags.output = match s.as_str() {
                "text" => OutputMode::Text,
                "json" => OutputMode::Json,
                // `quiet` is intentionally NOT honored via env —
                // it's a flag-only opt-in to avoid surprising CI
                // scripts that set `ZZ_OUTPUT` for JSON purposes.
                other => {
                    return Err(FlagError::BadEnvValue {
                        var: "ZZ_OUTPUT",
                        value: other.to_string(),
                    });
                }
            };
        }
    }
    if !explicit_passphrase {
        if let Some(s) = lookup("ZZ_PASSPHRASE_FILE") {
            if !s.is_empty() {
                flags.passphrase_file = Some(PathBuf::from(s));
            }
        }
    }
    if !explicit_alias {
        if let Some(s) = lookup("ZZ_ALIAS") {
            if !s.is_empty() {
                flags.alias = Some(s);
            }
        }
    }
    if !explicit_container {
        if let Some(s) = lookup("ZZ_CONTAINER") {
            flags.container = Some(match s.as_str() {
                "local" => ContainerSource::Local,
                "remote" => ContainerSource::Remote,
                other => {
                    return Err(FlagError::BadEnvValue {
                        var: "ZZ_CONTAINER",
                        value: other.to_string(),
                    });
                }
            });
        }
    }
    Ok(flags)
}

/// Production env lookup — reads from the OS process environment
/// via `std::env::var_os`. Returns `None` for unset OR for values
/// containing non-UTF-8 (handled like empty for the strict env
/// vars; `ZZ_*` are documented to be ASCII).
pub fn env_lookup_os(key: &str) -> Option<String> {
    std::env::var_os(key).and_then(|v| v.into_string().ok())
}

/// Drive the full pre-pass: extract flags from argv, then layer
/// env-var overrides. Returns the residual argv that the verb
/// parser consumes plus the merged flag set.
pub fn parse_global(argv: Vec<String>) -> Result<(GlobalFlags, Vec<String>), FlagError> {
    let (flags, residual) = extract_flags(argv)?;
    let explicit_output = flags.output != OutputMode::Text;
    let explicit_pp = flags.passphrase_file.is_some();
    let explicit_alias = flags.alias.is_some();
    let explicit_container = flags.container.is_some();

    let merged = merge_env(
        flags,
        explicit_output,
        explicit_pp,
        explicit_alias,
        explicit_container,
        env_lookup_os,
    )?;
    Ok((merged, residual))
}

// ---------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------

fn split_eq(tok: &str) -> (&str, Option<&str>) {
    match tok.find('=') {
        Some(i) => (&tok[..i], Some(&tok[i + 1..])),
        None => (tok, None),
    }
}

fn reject_inline(name: &'static str, inline: Option<&str>) -> Result<(), FlagError> {
    if let Some(v) = inline {
        if !v.is_empty() {
            // `--json=foo` makes no semantic sense; reject loudly.
            return Err(FlagError::UnknownFlag {
                token: format!("{name}={v}"),
            });
        }
    }
    Ok(())
}

fn take_value<I: Iterator<Item = String>>(
    name: &'static str,
    inline: Option<&str>,
    iter: &mut I,
) -> Result<String, FlagError> {
    let raw = match inline {
        Some(v) => v.to_string(),
        None => iter.next().ok_or(FlagError::MissingValue { flag: name })?,
    };
    if raw.is_empty() {
        return Err(FlagError::EmptyValue { flag: name });
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(args: &[&str]) -> Result<(GlobalFlags, Vec<String>), FlagError> {
        extract_flags(args.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn no_flags_leaves_argv_unchanged() {
        let (f, r) = extract(&["s", "file.md"]).unwrap();
        assert_eq!(f.output, OutputMode::Text);
        assert!(f.passphrase_file.is_none());
        assert_eq!(r, vec!["s", "file.md"]);
    }

    #[test]
    fn json_flag_sets_mode_and_strips_token() {
        let (f, r) = extract(&["--json", "s", "file.md"]).unwrap();
        assert_eq!(f.output, OutputMode::Json);
        assert_eq!(r, vec!["s", "file.md"]);
    }

    #[test]
    fn quiet_flag_sets_mode() {
        let (f, _) = extract(&["--quiet", "f"]).unwrap();
        assert_eq!(f.output, OutputMode::Quiet);
    }

    #[test]
    fn quiet_json_conflict_is_rejected() {
        assert_eq!(
            extract(&["--quiet", "--json", "f"]).unwrap_err(),
            FlagError::QuietJsonConflict
        );
        assert_eq!(
            extract(&["--json", "--quiet", "f"]).unwrap_err(),
            FlagError::QuietJsonConflict
        );
    }

    #[test]
    fn passphrase_file_accepts_space_or_eq_form() {
        let (f, _) = extract(&["--passphrase-file", "/tmp/p", "z"]).unwrap();
        assert_eq!(f.passphrase_file.as_deref(), Some(PathBuf::from("/tmp/p").as_path()));
        let (f, _) = extract(&["--passphrase-file=/tmp/p", "z"]).unwrap();
        assert_eq!(f.passphrase_file.as_deref(), Some(PathBuf::from("/tmp/p").as_path()));
    }

    #[test]
    fn passphrase_file_requires_value() {
        assert_eq!(
            extract(&["--passphrase-file"]).unwrap_err(),
            FlagError::MissingValue { flag: "--passphrase-file" }
        );
        assert_eq!(
            extract(&["--passphrase-file="]).unwrap_err(),
            FlagError::EmptyValue { flag: "--passphrase-file" }
        );
    }

    #[test]
    fn alias_flag_sets_value() {
        let (f, r) = extract(&["--alias", "work-nc", "f"]).unwrap();
        assert_eq!(f.alias.as_deref(), Some("work-nc"));
        assert_eq!(r, vec!["f"]);
    }

    #[test]
    fn local_and_remote_are_exclusive() {
        let (f, _) = extract(&["--local", "z"]).unwrap();
        assert_eq!(f.container, Some(ContainerSource::Local));
        let (f, _) = extract(&["--remote", "z"]).unwrap();
        assert_eq!(f.container, Some(ContainerSource::Remote));
        assert_eq!(
            extract(&["--local", "--remote", "z"]).unwrap_err(),
            FlagError::ContainerConflict
        );
    }

    #[test]
    fn yes_flag_is_a_boolean_switch() {
        let (f, r) = extract(&["--yes", "w"]).unwrap();
        assert!(f.yes);
        assert_eq!(r, vec!["w"]);
    }

    #[test]
    fn unknown_flag_is_rejected() {
        match extract(&["--unknown", "s", "f"]).unwrap_err() {
            FlagError::UnknownFlag { token } => assert_eq!(token, "--unknown"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn double_dash_terminates_flag_parsing() {
        let (f, r) = extract(&["--json", "--", "--weird-name"]).unwrap();
        assert_eq!(f.output, OutputMode::Json);
        assert_eq!(r, vec!["--weird-name"]);
    }

    #[test]
    fn flag_after_positional_is_left_for_verb_parser() {
        // Global flags accepted only before the first non-flag
        // token; trailing tokens are verbatim.
        let (f, r) = extract(&["s", "file.md", "--json"]).unwrap();
        assert_eq!(f.output, OutputMode::Text);
        assert_eq!(r, vec!["s", "file.md", "--json"]);
    }

    // ---- merge_env -----------------------------------------------------
    //
    // Closure-based lookup keeps these tests off the global process
    // env, which is racy under multi-threaded test runs and (since
    // Rust 2024) requires `unsafe` to mutate. The lookup matches the
    // production signature: `Fn(&str) -> Option<String>`.

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |k| owned.iter().find(|(name, _)| name == k).map(|(_, v)| v.clone())
    }

    #[test]
    fn env_output_json_honored_when_no_flag() {
        let merged = merge_env(
            GlobalFlags::default(),
            false,
            false,
            false,
            false,
            env(&[("ZZ_OUTPUT", "json")]),
        )
        .unwrap();
        assert_eq!(merged.output, OutputMode::Json);
    }

    #[test]
    fn env_output_quiet_is_rejected() {
        let err = merge_env(
            GlobalFlags::default(),
            false,
            false,
            false,
            false,
            env(&[("ZZ_OUTPUT", "quiet")]),
        )
        .unwrap_err();
        assert!(matches!(err, FlagError::BadEnvValue { var: "ZZ_OUTPUT", .. }));
    }

    #[test]
    fn env_output_ignored_when_flag_explicit() {
        let mut flags = GlobalFlags::default();
        flags.output = OutputMode::Quiet; // simulate --quiet
        let merged = merge_env(
            flags,
            true,
            false,
            false,
            false,
            env(&[("ZZ_OUTPUT", "json")]),
        )
        .unwrap();
        assert_eq!(merged.output, OutputMode::Quiet);
    }

    #[test]
    fn env_passphrase_file_honored_when_no_flag() {
        let merged = merge_env(
            GlobalFlags::default(),
            false,
            false,
            false,
            false,
            env(&[("ZZ_PASSPHRASE_FILE", "/tmp/pp")]),
        )
        .unwrap();
        assert_eq!(
            merged.passphrase_file.as_deref(),
            Some(PathBuf::from("/tmp/pp").as_path())
        );
    }

    #[test]
    fn env_alias_honored_when_no_flag() {
        let merged = merge_env(
            GlobalFlags::default(),
            false,
            false,
            false,
            false,
            env(&[("ZZ_ALIAS", "home-gdrive")]),
        )
        .unwrap();
        assert_eq!(merged.alias.as_deref(), Some("home-gdrive"));
    }

    #[test]
    fn env_container_local_remote_honored_and_rejected() {
        let merged = merge_env(
            GlobalFlags::default(),
            false,
            false,
            false,
            false,
            env(&[("ZZ_CONTAINER", "remote")]),
        )
        .unwrap();
        assert_eq!(merged.container, Some(ContainerSource::Remote));

        let err = merge_env(
            GlobalFlags::default(),
            false,
            false,
            false,
            false,
            env(&[("ZZ_CONTAINER", "garbage")]),
        )
        .unwrap_err();
        assert!(matches!(err, FlagError::BadEnvValue { var: "ZZ_CONTAINER", .. }));
    }

    #[test]
    fn empty_env_passphrase_is_ignored() {
        let merged = merge_env(
            GlobalFlags::default(),
            false,
            false,
            false,
            false,
            env(&[("ZZ_PASSPHRASE_FILE", "")]),
        )
        .unwrap();
        assert!(
            merged.passphrase_file.is_none(),
            "empty ZZ_PASSPHRASE_FILE must not set the field"
        );
    }

    #[test]
    fn explicit_flag_pp_wins_over_env() {
        // `--passphrase-file /a` already set; env tries `/b`; flag
        // wins by passing explicit_passphrase=true.
        let mut flags = GlobalFlags::default();
        flags.passphrase_file = Some(PathBuf::from("/a"));
        let merged = merge_env(
            flags,
            false,
            true,
            false,
            false,
            env(&[("ZZ_PASSPHRASE_FILE", "/b")]),
        )
        .unwrap();
        assert_eq!(merged.passphrase_file.as_deref(), Some(PathBuf::from("/a").as_path()));
    }
}
