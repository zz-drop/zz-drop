//! State detector for SACS. Cheap on every TAB:
//! filesystem stats + a single non-blocking probe of the agent
//! socket. The expensive `Status` round-trip is reserved for
//! when the caller needs to disambiguate S2 (locked) from S3
//! (ready) — most code paths can short-circuit before then.
//!
//! Design reference: `cli-autosuggest.md` §5.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SacsState {
    /// No container on disk (or only `profiles-remote.zz` in a
    /// build without the `remote` feature — same effective state
    /// because the binary cannot decrypt it).
    S0Fresh,
    /// Container exists, agent socket absent.
    S1Down,
    /// Agent reachable but locked (resolved by the caller via
    /// `Status`; this detector leaves S2 vs S3 to the caller).
    S2Locked,
    /// Agent reachable and unlocked.
    S3Ready,
    /// Same as S3 but two containers exist on disk; the first-arg
    /// ranker should surface `z local` / `z remote`.
    S4ReadyDual,
}

/// Filesystem signals the detector relies on. Pulled out into a
/// struct so tests can construct synthetic fixtures.
#[derive(Debug, Clone)]
pub struct Signals {
    pub profiles_local_exists: bool,
    pub profiles_remote_exists: bool,
    pub remote_feature_compiled_in: bool,
    pub agent_socket_exists: bool,
    /// `None` if the caller skipped the agent probe; the detector
    /// then collapses S2/S3/S4 into S2Locked for safety. Pass
    /// `Some(true)` only after a successful `Status { unlocked: true }`.
    pub agent_unlocked: Option<bool>,
}

impl Signals {
    /// Read live signals from disk (cheap stat calls only — no
    /// agent round-trip yet). The `unlocked` field starts `None`
    /// and is filled by the caller after a `Status` probe if
    /// needed.
    pub fn detect(
        profiles_local: &Path,
        profiles_remote: &Path,
        agent_socket: &Path,
        remote_feature_compiled_in: bool,
    ) -> Self {
        Self {
            profiles_local_exists: profiles_local.exists(),
            profiles_remote_exists: profiles_remote.exists(),
            remote_feature_compiled_in,
            agent_socket_exists: agent_socket.exists(),
            agent_unlocked: None,
        }
    }
}

/// Resolve the state from filesystem + agent signals.
pub fn classify(s: &Signals) -> SacsState {
    // `profiles-remote.zz` only counts when the binary can
    // actually decrypt it. In a default v1 build (no `remote`
    // feature) the file is dead weight, so we treat the binary
    // as "container-less" if local is missing too.
    let remote_usable = s.profiles_remote_exists && s.remote_feature_compiled_in;
    let local_usable = s.profiles_local_exists;

    let any_container = local_usable || remote_usable;
    if !any_container {
        return SacsState::S0Fresh;
    }
    if !s.agent_socket_exists {
        return SacsState::S1Down;
    }
    match s.agent_unlocked {
        Some(true) => {
            if local_usable && remote_usable {
                SacsState::S4ReadyDual
            } else {
                SacsState::S3Ready
            }
        }
        // `None` (caller skipped the probe) and `Some(false)`
        // both fall back to S2: a locked agent and a "we don't
        // know yet" should produce the same conservative ranking
        // (no remote candidates, no `z local|remote` split).
        _ => SacsState::S2Locked,
    }
}

/// Hard-coded knowledge of which Cargo features are compiled
/// into this binary. SACS reads it via `cfg!` so the answer
/// stays static at build time and there is no env var to
/// spoof at runtime.
pub const fn remote_feature_compiled_in() -> bool {
    cfg!(feature = "remote")
}

/// Convenience wrapper for the production call site: `Paths`
/// from the discovery layer + the const above.
pub fn detect_signals_from_paths(
    profiles_local: &Path,
    profiles_remote: &Path,
    agent_socket: &Path,
) -> Signals {
    Signals::detect(
        profiles_local,
        profiles_remote,
        agent_socket,
        remote_feature_compiled_in(),
    )
}

/// Owned-path variant for the rare callers that don't already
/// hold borrowed paths in scope. Currently unused but
/// inexpensive to keep around — it documents the intended API
/// shape for chunk E.
#[allow(dead_code)]
pub fn detect_signals_owned(
    profiles_local: PathBuf,
    profiles_remote: PathBuf,
    agent_socket: PathBuf,
) -> Signals {
    detect_signals_from_paths(&profiles_local, &profiles_remote, &agent_socket)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(local: bool, remote: bool, remote_feature: bool, sock: bool, unlocked: Option<bool>) -> Signals {
        Signals {
            profiles_local_exists: local,
            profiles_remote_exists: remote,
            remote_feature_compiled_in: remote_feature,
            agent_socket_exists: sock,
            agent_unlocked: unlocked,
        }
    }

    #[test]
    fn s0_when_no_container() {
        assert_eq!(classify(&s(false, false, false, false, None)), SacsState::S0Fresh);
        assert_eq!(classify(&s(false, false, true, false, None)), SacsState::S0Fresh);
        // Even with a running agent, no container means S0 — a
        // stale socket without files is fresh state.
        assert_eq!(classify(&s(false, false, false, true, Some(true))), SacsState::S0Fresh);
    }

    #[test]
    fn remote_only_in_default_build_is_s0() {
        // `profiles-remote.zz` present, but the binary lacks the
        // `remote` feature → cannot decrypt → effectively S0
        // degraded. Documented in the plan.
        assert_eq!(classify(&s(false, true, false, false, None)), SacsState::S0Fresh);
        assert_eq!(classify(&s(false, true, false, true, Some(true))), SacsState::S0Fresh);
    }

    #[test]
    fn remote_only_with_feature_promotes_state() {
        assert_eq!(classify(&s(false, true, true, false, None)), SacsState::S1Down);
        assert_eq!(classify(&s(false, true, true, true, Some(true))), SacsState::S3Ready);
    }

    #[test]
    fn s1_when_container_but_no_socket() {
        assert_eq!(classify(&s(true, false, false, false, None)), SacsState::S1Down);
        assert_eq!(classify(&s(true, true, true, false, None)), SacsState::S1Down);
    }

    #[test]
    fn s2_when_socket_exists_and_unlock_unknown_or_locked() {
        // Detector left `unlocked = None` — treat as locked.
        assert_eq!(classify(&s(true, false, false, true, None)), SacsState::S2Locked);
        // Caller probed and got `unlocked = false`.
        assert_eq!(classify(&s(true, false, false, true, Some(false))), SacsState::S2Locked);
    }

    #[test]
    fn s3_when_unlocked_and_only_one_container() {
        assert_eq!(classify(&s(true, false, false, true, Some(true))), SacsState::S3Ready);
        assert_eq!(classify(&s(false, true, true, true, Some(true))), SacsState::S3Ready);
    }

    #[test]
    fn s4_when_unlocked_and_two_usable_containers() {
        // Both files present AND remote feature compiled → dual
        // container ranking.
        assert_eq!(classify(&s(true, true, true, true, Some(true))), SacsState::S4ReadyDual);
    }

    #[test]
    fn s4_demotes_to_s3_when_remote_feature_off() {
        // Both files on disk but binary cannot decrypt remote →
        // ranker should NOT offer `z remote`, so S3.
        assert_eq!(classify(&s(true, true, false, true, Some(true))), SacsState::S3Ready);
    }

    #[test]
    fn signals_detect_reads_filesystem_lazily() {
        // Constructing Signals must not panic on missing paths.
        let nope = Path::new("/nonexistent/a");
        let sig = Signals::detect(nope, nope, nope, false);
        assert!(!sig.profiles_local_exists);
        assert!(!sig.profiles_remote_exists);
        assert!(!sig.agent_socket_exists);
        assert_eq!(sig.agent_unlocked, None);
    }
}
