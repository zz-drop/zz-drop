//! Shell completion script templates inlined at compile time.
//!
//! The actual content lives in [`zz_drop_core::completions::scripts`]
//! so the TUI can install completions without duplicating the
//! payload. This module re-exports the three constants under their
//! historical names so existing call sites compile unchanged.

pub use zz_drop_core::completions::scripts::{BASH, FISH, ZSH};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_template_falls_back_when_neither_binary_is_on_path() {
        for (name, body) in [("bash", BASH), ("zsh", ZSH), ("fish", FISH)] {
            assert!(
                body.contains("zz-drop"),
                "{name} template forgot the zz-drop fallback"
            );
        }
    }

    #[test]
    fn each_template_registers_for_both_zz_and_zz_drop() {
        for (name, body) in [("bash", BASH), ("zsh", ZSH), ("fish", FISH)] {
            assert!(
                body.contains("zz-drop"),
                "{name}: missing zz-drop registration"
            );
            assert!(body.contains(" zz") || body.contains("zz "));
        }
    }

    #[test]
    fn no_secret_or_remote_host_in_templates() {
        for body in [BASH, ZSH, FISH] {
            assert!(!body.contains("zz-drop.net"));
            assert!(!body.contains("@example.org"));
            assert!(!body.contains("@example.com"));
            assert!(!body.contains("passphrase"));
            assert!(!body.contains("access_token"));
            assert!(!body.contains("refresh_token"));
            assert!(!body.to_lowercase().contains("bearer"));
        }
    }
}
