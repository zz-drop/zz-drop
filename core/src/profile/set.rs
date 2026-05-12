//! Container for multiple `PlainProfile`s, encrypted as a single
//! envelope on disk.
//!
//! `profiles-{local,remote}.zz` wraps a `ProfileSet`, not a
//! `PlainProfile`. The single-blob single-profile model from
//! `format.rs` is preserved for backward inspection only — every new
//! file written goes through `encrypt_set` / `decrypt_set`.

use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::crypto::aead::{KEY_LEN, SALT_LEN};
use crate::crypto::kdf::Argon2idConfig;
use crate::profile::types::PlainProfile;

/// Schema version for the `ProfileSet` payload. v1 was the implicit
/// "container is a single `PlainProfile`" of the legacy on-disk
/// format; v2 is this struct.
pub const PROFILE_SET_SCHEMA_V2: u32 = 2;

/// Encrypted container: holds `0..=N` inner profiles. The active
/// default alias does **not** live here — it is persisted in the
/// `last-default-{local,remote}` plaintext sidecar (decision log
/// 2026-05-02 close-out, resolution #4).
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileSet {
    pub schema_version: u32,
    pub profiles: Vec<PlainProfile>,
}

impl ProfileSet {
    /// Empty container at the current schema version.
    pub fn new() -> Self {
        Self {
            schema_version: PROFILE_SET_SCHEMA_V2,
            profiles: Vec::new(),
        }
    }

    /// Container with a single inner profile. Convenient for the
    /// first-setup path.
    pub fn with_profile(profile: PlainProfile) -> Self {
        Self {
            schema_version: PROFILE_SET_SCHEMA_V2,
            profiles: vec![profile],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn contains_alias(&self, alias: &str) -> bool {
        self.profiles.iter().any(|p| p.alias == alias)
    }

    pub fn find_by_alias(&self, alias: &str) -> Option<&PlainProfile> {
        self.profiles.iter().find(|p| p.alias == alias)
    }

    pub fn find_by_alias_mut(&mut self, alias: &str) -> Option<&mut PlainProfile> {
        self.profiles.iter_mut().find(|p| p.alias == alias)
    }

    pub fn aliases(&self) -> Vec<&str> {
        self.profiles.iter().map(|p| p.alias.as_str()).collect()
    }
}

impl Default for ProfileSet {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ProfileSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProfileSet")
            .field("schema_version", &self.schema_version)
            .field("profiles", &format_args!("<{} redacted>", self.profiles.len()))
            .finish()
    }
}

/// Key encryption key + the derivation context needed to re-encrypt
/// the container without re-running Argon2id. Held in agent RAM after
/// unlock so the TUI can append/remove inner profiles and so the CLI
/// can persist refreshed OAuth tokens, all without re-prompting the
/// passphrase (decision log 2026-05-02 close-out, resolution #1).
#[derive(Clone)]
pub struct ProfileKek {
    pub(crate) key: Zeroizing<[u8; KEY_LEN]>,
    pub(crate) salt: [u8; SALT_LEN],
    pub(crate) kdf_config: Argon2idConfig,
}

impl ProfileKek {
    pub(crate) fn new(
        key: Zeroizing<[u8; KEY_LEN]>,
        salt: [u8; SALT_LEN],
        kdf_config: Argon2idConfig,
    ) -> Self {
        Self {
            key,
            salt,
            kdf_config,
        }
    }

    /// Reconstruct a `ProfileKek` from its constituent parts. Used by
    /// the agent server when it receives the KEK over the wire from
    /// the unlocking CLI.
    ///
    /// The caller is expected to have transported `key` carefully —
    /// the bytes go straight into a `Zeroizing` buffer here.
    pub fn from_parts(
        key: [u8; KEY_LEN],
        salt: [u8; SALT_LEN],
        kdf_config: Argon2idConfig,
    ) -> Self {
        Self {
            key: Zeroizing::new(key),
            salt,
            kdf_config,
        }
    }

    /// Read access to the raw 32-byte key. The caller is responsible
    /// for handling the bytes in zeroizing storage of its own.
    pub fn key_bytes(&self) -> &[u8; KEY_LEN] {
        &self.key
    }

    pub fn salt(&self) -> &[u8; SALT_LEN] {
        &self.salt
    }

    pub fn kdf_config(&self) -> &Argon2idConfig {
        &self.kdf_config
    }
}

impl fmt::Debug for ProfileKek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProfileKek { <redacted> }")
    }
}
