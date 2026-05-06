//! Single-slot cache for the active provider client inside the
//! agent. Built on demand from the unlocked `PlainProfile` and
//! kept around so subsequent `LIST_REMOTE` calls reuse the warm
//! HTTP connection. Dropped on every event that could change
//! "what the active provider is":
//!
//! - `Lock` (and TTL auto-lock)
//! - `UpdateProfile` matching the active alias
//! - `UpdateProfileSet`
//! - `SetActiveAlias`
//!
//! A single slot is enough because SACS only ever serves the
//! active inner profile; cross-alias listing isn't part of the
//! v1 surface (`cli-autosuggest.md` §3 "1 active profile per
//! unlocked container").

use std::sync::Mutex;

use zz_drop_core::PlainProfile;

use crate::commands::remote_fs::AnyRemote;

#[derive(Debug)]
pub enum BuildError {
    Diagnostic(&'static str),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostic(d) => f.write_str(d),
        }
    }
}

impl std::error::Error for BuildError {}

struct Slot {
    profile_id: String,
    /// Non-`Send`/`Sync`-friendly handle. Wrapped in `Mutex` at
    /// the [`RemoteClientCache`] level so the rest of the agent
    /// can hold an `Arc<AgentState>` without inheriting the
    /// `RemoteFs` trait's relaxed thread bounds.
    client: AnyRemote,
}

pub struct RemoteClientCache {
    slot: Mutex<Option<Slot>>,
}

impl Default for RemoteClientCache {
    fn default() -> Self {
        Self {
            slot: Mutex::new(None),
        }
    }
}

impl std::fmt::Debug for RemoteClientCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cached = self
            .slot
            .lock()
            .map(|g| g.is_some())
            .unwrap_or(false);
        f.debug_struct("RemoteClientCache")
            .field("populated", &cached)
            .finish()
    }
}

impl RemoteClientCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run `f` with the active provider client. Builds it from
    /// `profile` on first call (and on any subsequent call where
    /// the cached `profile_id` differs); caches the resulting
    /// `AnyRemote` for the next call.
    ///
    /// The closure runs while the internal `Mutex` is held, so
    /// callers must keep it short. `LIST_REMOTE` over WebDAV /
    /// Drive / Graph is what blocks the agent's accept loop —
    /// see `docs/sacs.md` "Latency" for the documented caveat.
    pub fn with_client<R>(
        &self,
        profile: &PlainProfile,
        f: impl FnOnce(&AnyRemote) -> R,
    ) -> Result<R, BuildError> {
        let mut guard = self
            .slot
            .lock()
            .map_err(|_| BuildError::Diagnostic("agent client cache poisoned"))?;
        let needs_rebuild = match &*guard {
            Some(s) => s.profile_id != profile.profile_id,
            None => true,
        };
        if needs_rebuild {
            let client =
                crate::commands::build_remote(profile).map_err(BuildError::Diagnostic)?;
            *guard = Some(Slot {
                profile_id: profile.profile_id.clone(),
                client,
            });
        }
        let slot = guard.as_ref().unwrap();
        Ok(f(&slot.client))
    }

    /// Drop the cached client. Called by `AgentState` on every
    /// transition that could change the active provider.
    pub fn clear(&self) {
        if let Ok(mut g) = self.slot.lock() {
            *g = None;
        }
    }

    #[cfg(test)]
    pub fn is_populated(&self) -> bool {
        self.slot.lock().map(|g| g.is_some()).unwrap_or(false)
    }
}
