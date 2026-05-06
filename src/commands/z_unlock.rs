//! `zz z [local|remote]` — orchestrate container unlock then picker.
//!
//! Flow for `zz z local`:
//!   1. read sidecar `last-default-local` if present (defensive
//!      parsing per `sidecars` rules)
//!   2. read `profiles-local.zz` from disk
//!   3. prompt passphrase (the prompt label tells the operator which
//!      file is being unlocked)
//!   4. decrypt → `(ProfileSet, ProfileKek)`
//!   5. picker: numbered list, default = sidecar alias if it still
//!      resolves to an inner profile
//!   6. spawn agent if not running, connect
//!   7. send `Unlock { profile_set, kek, active_alias, ttl }`
//!   8. write sidecar with the chosen alias (best-effort)
//!
//! `zz z` (no args) reads the local sidecar first; if a default
//! resolves, the picker honours it. If neither container exists, the
//! operator is told to run `zz c`.
//!
//! `zz z remote` is gated by the `remote` feature flag (TASK 46) and
//! is not built into the default v1 binary. Today it surfaces a
//! clear "remote not available in this build" message.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use zz_drop_core::AgentResponse;
use zz_drop_core::config::Paths;
use zz_drop_core::diag_log;
use zz_drop_core::{ProfileCryptoError, decrypt_set};

use crate::agent::{AGENT_MODE_ENV, AgentClient, ClientError};
use crate::cli::ContainerSource;
use crate::commands::{
    EXIT_AGENT_UNREACHABLE, EXIT_DECRYPT_FAILED, EXIT_NOT_IMPLEMENTED, EXIT_OK,
    EXIT_PROFILE_MISSING, EXIT_USAGE,
};
use crate::output;
use crate::picker::{PickError, pick_alias};
use zz_drop_core::sidecars;

const UNLOCK_TTL_SECS: u64 = 600;

pub fn run(paths: &Paths, which: Option<ContainerSource>) -> i32 {
    let resolved = match which {
        Some(s) => s,
        None => match resolve_default_source(paths) {
            Ok(s) => s,
            Err(code) => return code,
        },
    };

    match resolved {
        ContainerSource::Local => unlock_local(paths),
        ContainerSource::Remote => {
            // The remote container flow lives behind the `remote`
            // feature flag (TASK 46). The default v1 binary does not
            // ship it; we surface a clear message rather than
            // silently failing.
            output::err_line(
                "remote container not available in this build (v1 ships local-only)",
            );
            EXIT_NOT_IMPLEMENTED
        }
    }
}

/// Pick local vs remote when the operator ran `zz z` with no args.
fn resolve_default_source(paths: &Paths) -> Result<ContainerSource, i32> {
    let local_present = paths.profiles_local_file.exists();
    let remote_present = paths.profiles_remote_file.exists();

    match (local_present, remote_present) {
        (false, false) => {
            output::err_line(&format!(
                "no profile container at {} or {}; run `zz c` to configure one",
                paths.profiles_local_file.display(),
                paths.profiles_remote_file.display()
            ));
            Err(EXIT_PROFILE_MISSING)
        }
        (true, false) => Ok(ContainerSource::Local),
        (false, true) => Ok(ContainerSource::Remote),
        (true, true) => {
            // Both exist. Pick the one that has a cached default in
            // its sidecar; if neither does, default to local.
            let local_has_default =
                sidecars::read_local_default(&paths.last_default_local_file).is_ok();
            let remote_has_default =
                sidecars::read_remote_default(&paths.last_default_remote_file).is_ok();
            match (local_has_default, remote_has_default) {
                (true, false) => Ok(ContainerSource::Local),
                (false, true) => Ok(ContainerSource::Remote),
                _ => Ok(ContainerSource::Local),
            }
        }
    }
}

fn unlock_local(paths: &Paths) -> i32 {
    if !paths.profiles_local_file.exists() {
        output::err_line(&format!(
            "no local container at {}; run `zz c` to configure one",
            paths.profiles_local_file.display()
        ));
        return EXIT_PROFILE_MISSING;
    }

    let envelope = match std::fs::read_to_string(&paths.profiles_local_file) {
        Ok(s) => s,
        Err(e) => {
            output::err_line(&format!("could not read profile container: {e}"));
            diag_log::log(&format!(
                "unlock_local read_err path={} err={e}",
                paths.profiles_local_file.display()
            ));
            return EXIT_PROFILE_MISSING;
        }
    };
    let envelope_fnv = diag_log::fnv64(envelope.as_bytes());
    let envelope_mtime_secs = std::fs::metadata(&paths.profiles_local_file)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(-1);
    diag_log::log(&format!(
        "unlock_local read path={} envelope_len={} envelope_fnv={:016x} mtime_unix={}",
        paths.profiles_local_file.display(),
        envelope.len(),
        envelope_fnv,
        envelope_mtime_secs,
    ));

    let label = paths
        .profiles_local_file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("profiles-local.zz")
        .to_string();
    let passphrase = match prompt_passphrase(&label) {
        Ok(p) => p,
        Err(e) => {
            output::err_line(&format!("could not read passphrase: {e}"));
            diag_log::log(&format!("unlock_local prompt_err err={e}"));
            return EXIT_USAGE;
        }
    };
    diag_log::log(&format!("unlock_local prompt pass_len={}", passphrase.len()));

    let (profile_set, kek) = match decrypt_set(&envelope, &passphrase) {
        Ok(pair) => pair,
        Err(ProfileCryptoError::Aead) => {
            output::err_line("decryption failed (wrong passphrase or corrupted container)");
            diag_log::log(&format!(
                "unlock_local decrypt_fail kind=Aead envelope_fnv={envelope_fnv:016x} pass_len={}",
                passphrase.len()
            ));
            return EXIT_DECRYPT_FAILED;
        }
        Err(ProfileCryptoError::LegacyFormat) => {
            output::err_line(
                "legacy single-profile blob detected (no migration in dev): run `zz w` and re-set up",
            );
            diag_log::log("unlock_local decrypt_fail kind=Legacy");
            return EXIT_DECRYPT_FAILED;
        }
        Err(e) => {
            output::err_line(&format!("decryption failed: {e}"));
            diag_log::log(&format!("unlock_local decrypt_fail kind=other detail={e}"));
            return EXIT_DECRYPT_FAILED;
        }
    };
    // KEK fingerprint is gated behind `ZZ_DROP_DECRYPT_DEBUG` —
    // FNV of a 32-byte secret isn't reversible, but the spirit of
    // the no-secret rule is "don't even partially exfiltrate the
    // KEK". Logging the salt fingerprint + KDF params is enough
    // for the everyday case ("did the on-disk file change?").
    let key_dbg = if std::env::var("ZZ_DROP_DECRYPT_DEBUG").is_ok() {
        format!(" key_fnv={:016x}", diag_log::fnv64(kek.key_bytes()))
    } else {
        String::new()
    };
    diag_log::log(&format!(
        "unlock_local decrypt_ok profiles={} kdf_m={} kdf_t={} kdf_p={} salt_fnv={:016x}{key_dbg}",
        profile_set.profiles.len(),
        kek.kdf_config().memory_kib,
        kek.kdf_config().iterations,
        kek.kdf_config().parallelism,
        diag_log::fnv64(kek.salt()),
    ));

    if profile_set.is_empty() {
        output::err_line("profile container is empty; run `zz c` to add a profile");
        return EXIT_DECRYPT_FAILED;
    }

    // Picker — honour the sidecar default if it still resolves to an
    // inner profile in the just-decrypted container.
    let cached = sidecars::read_local_default(&paths.last_default_local_file).ok();
    let cached_alias = cached
        .as_ref()
        .map(|d| d.alias.as_str())
        .filter(|a| profile_set.contains_alias(a));
    let aliases: Vec<&str> = profile_set.aliases();

    let active_alias = match pick_alias(&aliases, cached_alias) {
        Ok(a) => a,
        Err(PickError::EmptyList) => {
            output::err_line("profile container is empty; run `zz c` to add a profile");
            return EXIT_DECRYPT_FAILED;
        }
        Err(PickError::NotInteractive) => {
            output::err_line(
                "no cached default and stdin is not a terminal; run `zz z local` interactively first",
            );
            return EXIT_USAGE;
        }
        Err(PickError::InvalidIndex) => {
            output::err_line("invalid selection");
            return EXIT_USAGE;
        }
        Err(PickError::Stdin) => {
            output::err_line("could not read selection");
            return EXIT_USAGE;
        }
    };

    let active_profile = profile_set
        .find_by_alias(&active_alias)
        .expect("alias was just chosen from the set");
    let target = output::profile_target(active_profile);

    // Stale-agent eviction: if a process from an older build is
    // still listening, SIGTERM it (the lock module already
    // wipes socket/token/lock) so the spawn below brings up a
    // fresh agent from the current binary.
    let stale = crate::agent::lock::check_for_stale_agent(
        &paths.runtime_dir,
        &paths.agent_socket,
        &paths.token_file,
    );
    diag_log::log(&format!("unlock_local stale_check={stale:?}"));

    if !paths.agent_socket.exists() {
        diag_log::log("unlock_local agent spawn");
        if let Err(e) = spawn_agent() {
            output::err_line(&format!("could not start agent: {e}"));
            diag_log::log(&format!("unlock_local agent spawn_err err={e}"));
            return EXIT_AGENT_UNREACHABLE;
        }
        if !wait_for_socket(&paths.agent_socket, Duration::from_secs(2)) {
            output::err_line("agent did not come up in time");
            diag_log::log("unlock_local agent wait_socket_timeout");
            return EXIT_AGENT_UNREACHABLE;
        }
    }

    let mut client = match AgentClient::connect(&paths.agent_socket, &paths.token_file) {
        Ok(c) => c,
        Err(e) => {
            output::err_line(&format!("could not connect to agent: {e}"));
            diag_log::log(&format!("unlock_local agent connect_err err={e}"));
            return EXIT_AGENT_UNREACHABLE;
        }
    };

    match client.unlock(profile_set, &kek, &active_alias, Some(UNLOCK_TTL_SECS)) {
        Ok(AgentResponse::Unlocked) => {
            // Best-effort: persist the chosen alias so a future
            // `zz z` no-args bypasses the picker.
            let _ = sidecars::write_local_default(
                &paths.last_default_local_file,
                &active_alias,
            );
            output::line(&format!("unlocked · {active_alias} · {target}"));
            diag_log::log(&format!(
                "unlock_local agent_ok alias={active_alias}"
            ));
            EXIT_OK
        }
        Ok(other) => {
            output::err_line(&format!("unexpected agent response: {other:?}"));
            diag_log::log(&format!("unlock_local agent_unexpected resp={other:?}"));
            EXIT_AGENT_UNREACHABLE
        }
        Err(ClientError::HandshakeFailed) => {
            output::err_line("agent handshake failed (token mismatch)");
            diag_log::log("unlock_local agent_handshake_fail");
            EXIT_AGENT_UNREACHABLE
        }
        Err(e) => {
            output::err_line(&format!("agent error: {e}"));
            diag_log::log(&format!("unlock_local agent_err {e}"));
            EXIT_AGENT_UNREACHABLE
        }
    }
}

fn prompt_passphrase(label: &str) -> std::io::Result<String> {
    rpassword::prompt_password(&format!("profile passphrase ({label}): "))
}

fn spawn_agent() -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    Command::new(exe)
        .env(AGENT_MODE_ENV, "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

fn wait_for_socket(path: &Path, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if path.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    path.exists()
}
