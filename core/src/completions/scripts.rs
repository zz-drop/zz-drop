//! Inlined shell completion script templates.
//!
//! Each script is a small relay (~50 lines) that calls back into
//! `zz __complete` and formats the NDJSON response for its native
//! shell. The brain lives in the binary; rebuilding `zz` updates
//! the suggestions without touching these files.
//!
//! Lives in `core` (not the root `zz-drop` crate) because both the
//! CLI binary and the TUI need to install them: the TUI invokes
//! [`crate::completions::install`] from the welcome screen.

pub const BASH: &str = include_str!("scripts/bash.sh");
pub const ZSH: &str = include_str!("scripts/zsh.sh");
pub const FISH: &str = include_str!("scripts/fish.fish");

/// Select the script for a given shell.
pub fn for_shell(shell: super::Shell) -> &'static str {
    match shell {
        super::Shell::Bash => BASH,
        super::Shell::Zsh => ZSH,
        super::Shell::Fish => FISH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_template_carries_its_marker() {
        assert!(BASH.contains("zz-drop:sacs-bash:v1"));
        assert!(ZSH.contains("zz-drop:sacs-zsh:v1"));
        assert!(FISH.contains("zz-drop:sacs-fish:v1"));
    }

    #[test]
    fn each_template_calls_back_into_zz_complete() {
        for (name, body) in [("bash", BASH), ("zsh", ZSH), ("fish", FISH)] {
            assert!(
                body.contains("__complete"),
                "{name} template no longer invokes `__complete`"
            );
        }
    }
}
