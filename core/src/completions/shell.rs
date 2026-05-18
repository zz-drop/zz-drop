//! Shell enum + detection from `$SHELL`.

use serde::Serialize;
use std::path::Path;

/// Target shell for completion install. v1 supports the three
/// SACS-bundled shells; PowerShell / nushell / elvish are out of
/// scope (see `docs/sacs.md`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

impl Shell {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
        }
    }

    /// Parse a shell name from a string. Accepts the bare names
    /// (`bash`, `zsh`, `fish`) and full paths (`/bin/zsh`,
    /// `/usr/local/bin/fish`). Case-sensitive.
    pub fn parse(s: &str) -> Option<Self> {
        let name = Path::new(s)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(s);
        match name {
            "bash" => Some(Self::Bash),
            "zsh" => Some(Self::Zsh),
            "fish" => Some(Self::Fish),
            _ => None,
        }
    }

    /// Detect from `$SHELL`. Returns `None` when the env var is
    /// unset or points at a shell SACS doesn't ship for. The
    /// caller can then ask the user, default to bash, or report
    /// an error — this function picks no policy.
    pub fn detect_from_env() -> Option<Self> {
        std::env::var("SHELL").ok().and_then(|s| Self::parse(&s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_names() {
        assert_eq!(Shell::parse("bash"), Some(Shell::Bash));
        assert_eq!(Shell::parse("zsh"), Some(Shell::Zsh));
        assert_eq!(Shell::parse("fish"), Some(Shell::Fish));
        assert_eq!(Shell::parse("powershell"), None);
        assert_eq!(Shell::parse(""), None);
    }

    #[test]
    fn parses_full_paths() {
        assert_eq!(Shell::parse("/bin/bash"), Some(Shell::Bash));
        assert_eq!(Shell::parse("/usr/local/bin/zsh"), Some(Shell::Zsh));
        assert_eq!(Shell::parse("/opt/homebrew/bin/fish"), Some(Shell::Fish));
    }

    #[test]
    fn serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Shell::Zsh).unwrap(), "\"zsh\"");
    }
}
