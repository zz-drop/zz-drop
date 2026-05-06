//! `zz z <email>` / `zz z <alias>` — pull the encrypted profile
//! container from the configured zz-drop server, persist it as
//! `profiles-remote.zz`, then chain into the same unlock dance as
//! `zz z remote`.
//!
//! Gated behind the `remote` Cargo feature (TASK 46, default-off
//! in v1). The default v1 binary parses the form so completion
//! and help stay consistent across builds, and surfaces a clear
//! "remote not enabled in this build" message at runtime.

use zz_drop_core::config::Paths;

use crate::cli::RemoteSelector;
use crate::commands::EXIT_NOT_IMPLEMENTED;
use crate::output;

pub fn run(paths: &Paths, selector: &RemoteSelector) -> i32 {
    #[cfg(not(feature = "remote"))]
    {
        let _ = (paths, selector);
        output::err_line(
            "remote container login is not enabled in this build (rebuild with `--features remote`)",
        );
        EXIT_NOT_IMPLEMENTED
    }
    #[cfg(feature = "remote")]
    {
        remote_impl::run(paths, selector)
    }
}

#[cfg(feature = "remote")]
mod remote_impl {
    use super::*;
    use crate::cli::ContainerSource;
    use crate::commands::{
        EXIT_AGENT_UNREACHABLE, EXIT_OK, EXIT_PROFILE_MISSING, EXIT_PROVIDER_ERROR,
        EXIT_USAGE, z_unlock,
    };
    use std::io::{self, Write};

    use zz_drop_core::api::{ApiClient, ApiClientError, LoginOutcome, ProfileSummary};

    pub fn run(paths: &Paths, selector: &RemoteSelector) -> i32 {
        // Server URL is read from the environment for v1. Persistence
        // (per-account stored URL) is a v1.1 follow-up.
        let server_url = match std::env::var("ZZ_SERVER_URL") {
            Ok(s) if !s.is_empty() => s,
            _ => {
                output::err_line(
                    "set ZZ_SERVER_URL to your server (e.g. https://zz-drop.net)",
                );
                return EXIT_USAGE;
            }
        };

        // v1 supports email-based login only. `zz z <alias>` would
        // need a saved session keyed by alias — out of scope for v1.
        let email = match selector {
            RemoteSelector::Email(e) => e.clone(),
            RemoteSelector::Alias(a) => {
                output::err_line(&format!(
                    "alias-only login (`zz z {a}`) requires a saved session — not yet supported in v1; use `zz z <email>` to authenticate"
                ));
                return EXIT_NOT_IMPLEMENTED;
            }
        };

        let password = match rpassword::prompt_password(&format!("password for {email}: ")) {
            Ok(p) => p,
            Err(e) => {
                output::err_line(&format!("could not read password: {e}"));
                return EXIT_USAGE;
            }
        };

        let client = ApiClient::new(server_url.clone());
        let session = match client.login(&email, &password) {
            Ok(LoginOutcome::Session(r)) => r,
            Ok(LoginOutcome::TotpRequired(c)) => match prompt_totp() {
                Err(e) => {
                    output::err_line(&format!("could not read TOTP code: {e}"));
                    return EXIT_USAGE;
                }
                Ok(code) => match client.login_totp(&c.challenge, &code) {
                    Ok(r) => r,
                    Err(e) => {
                        output::err_line(&format!("TOTP verification failed: {}", short(&e)));
                        return EXIT_PROVIDER_ERROR;
                    }
                },
            },
            Err(e) => {
                output::err_line(&format!("login failed: {}", short(&e)));
                return EXIT_PROVIDER_ERROR;
            }
        };
        // Best-effort: drop the in-RAM password as soon as login is past.
        drop(password);

        let client = client.with_token(session.token);

        let profiles = match client.list_profiles() {
            Ok(p) => p,
            Err(e) => {
                output::err_line(&format!("could not list aliases: {}", short(&e)));
                return EXIT_PROVIDER_ERROR;
            }
        };
        if profiles.profiles.is_empty() {
            output::err_line(
                "no profile aliases on this account; push one from the configuration TUI (`zz c`) first",
            );
            return EXIT_PROFILE_MISSING;
        }

        let alias = if profiles.profiles.len() == 1 {
            profiles.profiles[0].alias.as_str().to_string()
        } else {
            match pick_alias(&profiles.profiles) {
                Some(a) => a,
                None => {
                    output::err_line("no selection; aborting");
                    return EXIT_USAGE;
                }
            }
        };

        let blob = match client.get_blob(&alias) {
            Ok(b) => b,
            Err(e) => {
                output::err_line(&format!("could not download blob `{alias}`: {}", short(&e)));
                return EXIT_PROVIDER_ERROR;
            }
        };

        if let Some(parent) = paths.profiles_remote_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&paths.profiles_remote_file, &blob) {
            output::err_line(&format!(
                "could not write {}: {e}",
                paths.profiles_remote_file.display()
            ));
            return EXIT_PROFILE_MISSING;
        }
        output::line(&format!(
            "downloaded `{alias}` from {server_url} → {}",
            paths.profiles_remote_file.display()
        ));

        // Chain into the existing unlock dance for the remote slot.
        // Returns its own exit code; whatever `unlock_remote` says is
        // what the operator gets.
        let code = z_unlock::run(paths, Some(ContainerSource::Remote));
        if code == EXIT_OK {
            // Persist the email + chosen alias so a future `zz z`
            // no-args resolves directly without re-prompting.
            let _ = zz_drop_core::sidecars::write_remote_default(
                &paths.last_default_remote_file,
                &email,
                Some(&alias),
            );
        }
        // Mirror common agent failures back as the right code.
        if code == EXIT_AGENT_UNREACHABLE {
            return EXIT_AGENT_UNREACHABLE;
        }
        code
    }

    fn prompt_totp() -> io::Result<String> {
        // TOTP codes are short and not secrets in the same sense as
        // passphrases; printing them would be unusual but we still
        // route through stdin (no echo suppression — RFC 6238 codes
        // expire in 30 s, the operator's terminal scrollback is not
        // a meaningful exfiltration channel).
        print!("totp 6-digit code (or recovery code): ");
        io::stdout().flush()?;
        let mut s = String::new();
        io::stdin().read_line(&mut s)?;
        Ok(s.trim().to_string())
    }

    fn pick_alias(profiles: &[ProfileSummary]) -> Option<String> {
        output::line("aliases on this account:");
        for (i, p) in profiles.iter().enumerate() {
            output::line(&format!("  [{}] {}", i + 1, p.alias.as_str()));
        }
        print!("pick [1-{}]: ", profiles.len());
        io::stdout().flush().ok()?;
        let mut s = String::new();
        io::stdin().read_line(&mut s).ok()?;
        let idx: usize = s.trim().parse().ok()?;
        if idx < 1 || idx > profiles.len() {
            return None;
        }
        Some(profiles[idx - 1].alias.as_str().to_string())
    }

    /// Surface a short, operator-friendly form of `ApiClientError`
    /// — the underlying `Display` already redacts the bearer; we
    /// just trim the verbose `kind` prefix.
    fn short(e: &ApiClientError) -> String {
        match e {
            ApiClientError::Network(m) => format!("network: {m}"),
            ApiClientError::Transport(m) => format!("transport: {m}"),
            ApiClientError::Decode(m) => format!("decode: {m}"),
            ApiClientError::Api(_, m) => m.clone(),
            ApiClientError::NoToken => "missing session token".to_string(),
        }
    }
}
