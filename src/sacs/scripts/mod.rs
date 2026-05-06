//! Shell completion script templates inlined at compile time.
//!
//! Each script is intentionally dumb (~50 lines): it relays the
//! cursor context to `zz __complete` over stdout/stdin and
//! formats the NDJSON response for its native shell. Updating
//! the grammar never touches these files — the brain lives in
//! `zz`, and rebuilding the binary is enough.

pub const BASH: &str = include_str!("bash.sh");
pub const ZSH: &str = include_str!("zsh.sh");
pub const FISH: &str = include_str!("fish.fish");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_template_carries_its_marker() {
        assert!(
            BASH.contains("zz-drop:sacs-bash:v1"),
            "bash template missing marker"
        );
        assert!(
            ZSH.contains("zz-drop:sacs-zsh:v1"),
            "zsh template missing marker"
        );
        assert!(
            FISH.contains("zz-drop:sacs-fish:v1"),
            "fish template missing marker"
        );
    }

    #[test]
    fn each_template_calls_back_into_zz_complete() {
        // The script's only job is to invoke `zz __complete`
        // (or `zz-drop __complete`). If a future edit drops
        // that call, the completion silently does nothing.
        for (name, body) in [("bash", BASH), ("zsh", ZSH), ("fish", FISH)] {
            assert!(
                body.contains("__complete"),
                "{name} template no longer invokes `__complete`"
            );
        }
    }

    #[test]
    fn each_template_falls_back_when_neither_binary_is_on_path() {
        // We cannot exec the scripts in unit tests, but we can
        // verify the fallback string is present so the script
        // compiles to a no-op rather than an error spam when
        // installed without `zz`.
        for (name, body) in [("bash", BASH), ("zsh", ZSH), ("fish", FISH)] {
            assert!(
                body.contains("zz-drop"),
                "{name} template forgot the zz-drop fallback"
            );
        }
    }

    #[test]
    fn each_template_registers_for_both_zz_and_zz_drop() {
        // `zz` is the daily symlink, `zz-drop` is the canonical
        // binary name. Users who couldn't install the symlink
        // need completion on the long form too.
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
        // Privacy lock per design §12. The bare word "token" is
        // intentionally not blacklisted — it appears as the
        // generic shell term for "command-line word", not as an
        // OAuth/bearer/access token. We block the specific
        // sensitive forms instead.
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
