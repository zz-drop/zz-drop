//! Public schema constants and shared types for the `--json`
//! NDJSON event stream.
//!
//! See `docs/scriptable.md` for the user-facing spec and
//! `docs/scriptable/zz-drop-output.v1.json` for the JSON Schema.
//! The version string lives here so consumers compile against it
//! via `pub use`.

use serde::Serialize;

/// Schema version emitted as `"v": "<SCHEMA_V>"` on every record.
///
/// Stable once 1.0 ships. Additive field changes keep this at
/// `"1"`; a breaking change (rename, removal, type swap) bumps to
/// `"2"` and counts as a major-version event for downstream
/// scripts.
pub const SCHEMA_V: &str = "1";

/// Closed set of reason codes carried by `failed` events.
///
/// Each variant maps 1:1 to a `pub const EXIT_*` in
/// `zz-drop::commands`. Adding a variant means adding (or reusing)
/// an exit code; removing one is breaking and requires bumping
/// [`SCHEMA_V`] to `"2"`.
///
/// Serializes as snake_case bare strings (`"agent_locked"` etc.)
/// so consumers can match on `.reason == "agent_locked"` without
/// allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Flag / arg parse error, or an unsupported combination
    /// (e.g. `--quiet --json`). Maps to `EXIT_USAGE`.
    Usage,
    /// Recognised verb that has no implementation in the current
    /// build (e.g. `zz dax` placeholder). Maps to
    /// `EXIT_NOT_IMPLEMENTED`.
    NotImplemented,
    /// Agent socket missing or refused. Maps to
    /// `EXIT_AGENT_UNREACHABLE`.
    AgentUnreachable,
    /// `profiles-local.zz` or the requested container is absent.
    /// Maps to `EXIT_PROFILE_MISSING`.
    ProfileMissing,
    /// Passphrase wrong or container corrupt. Maps to
    /// `EXIT_DECRYPT_FAILED`.
    DecryptFailed,
    /// `zz w` cancelled by the operator (or by absent `--yes` in
    /// non-interactive mode — see [`Self::InteractiveRequired`]).
    /// Maps to `EXIT_WIPE_CANCELLED`.
    WipeCancelled,
    /// Upstream provider failed (HTTP error, network, etc.). Maps
    /// to `EXIT_PROVIDER_ERROR`.
    ProviderError,
    /// Agent is up but the profile is locked, and `--json` forbids
    /// auto-unlock. Caller must run `zz x --json --passphrase-file
    /// …` first. Maps to `EXIT_AGENT_LOCKED`.
    AgentLocked,
    /// Passphrase file mode > 0600 or owner mismatch. Maps to
    /// `EXIT_PASSPHRASE_FILE_INSECURE`.
    PassphraseFilePermissions,
    /// Command would prompt for confirmation; caller must pass
    /// `--yes` or an env override. Maps to `EXIT_USAGE`.
    InteractiveRequired,
    /// Command is a TUI / wizard and has no scriptable surface
    /// (e.g. `zz c`). Maps to `EXIT_USAGE`.
    InteractiveOnly,
    /// Multiple aliases present and no `--alias` / `ZZ_ALIAS` /
    /// cached default. The `candidates` array on the event lists
    /// the valid options. Maps to `EXIT_USAGE`.
    AliasAmbiguous,
    /// Both `local` and `remote` containers exist and neither
    /// `--local` / `--remote` / `ZZ_CONTAINER` was supplied. Maps
    /// to `EXIT_USAGE`.
    ContainerAmbiguous,
}

impl Reason {
    /// Bare snake_case form, useful in plain-text error hints and
    /// for symmetry with the serialized payload.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::NotImplemented => "not_implemented",
            Self::AgentUnreachable => "agent_unreachable",
            Self::ProfileMissing => "profile_missing",
            Self::DecryptFailed => "decrypt_failed",
            Self::WipeCancelled => "wipe_cancelled",
            Self::ProviderError => "provider_error",
            Self::AgentLocked => "agent_locked",
            Self::PassphraseFilePermissions => "passphrase_file_permissions",
            Self::InteractiveRequired => "interactive_required",
            Self::InteractiveOnly => "interactive_only",
            Self::AliasAmbiguous => "alias_ambiguous",
            Self::ContainerAmbiguous => "container_ambiguous",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SCHEMA_V, "1");
    }

    #[test]
    fn reason_serializes_as_snake_case_bare_string() {
        let cases = [
            (Reason::Usage, "\"usage\""),
            (Reason::AgentLocked, "\"agent_locked\""),
            (Reason::PassphraseFilePermissions, "\"passphrase_file_permissions\""),
            (Reason::AliasAmbiguous, "\"alias_ambiguous\""),
        ];
        for (r, expected) in cases {
            let got = serde_json::to_string(&r).unwrap();
            assert_eq!(got, expected, "reason={r:?}");
        }
    }

    #[test]
    fn reason_as_str_matches_serde_form() {
        for r in [
            Reason::Usage,
            Reason::NotImplemented,
            Reason::AgentUnreachable,
            Reason::ProfileMissing,
            Reason::DecryptFailed,
            Reason::WipeCancelled,
            Reason::ProviderError,
            Reason::AgentLocked,
            Reason::PassphraseFilePermissions,
            Reason::InteractiveRequired,
            Reason::InteractiveOnly,
            Reason::AliasAmbiguous,
            Reason::ContainerAmbiguous,
        ] {
            let quoted = format!("\"{}\"", r.as_str());
            assert_eq!(serde_json::to_string(&r).unwrap(), quoted, "reason={r:?}");
        }
    }
}
