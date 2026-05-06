use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::Argon2idConfig;
use crate::profile::{PlainProfile, ProfileKek, ProfileSet};

pub const PROTOCOL_VERSION: u16 = 1;

/// Wire-format counterpart of `ProfileKek`. The agent reconstructs a
/// `ProfileKek` from this on receive, with the bytes immediately
/// landing in a zeroizing buffer. Custom `Debug` redacts the key.
///
/// The `key` field is plain `[u8; 32]` here, which means it briefly
/// lives on the stack during decode before [`ProfileKek::from_parts`]
/// moves it into a zeroizing buffer. Decoding loops keep the
/// intermediate `KekPayload` short-lived to minimise the window.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KekPayload {
    pub key: [u8; 32],
    pub salt: [u8; 16],
    pub kdf: Argon2idConfig,
}

impl KekPayload {
    pub fn from_kek(kek: &ProfileKek) -> Self {
        Self {
            key: *kek.key_bytes(),
            salt: *kek.salt(),
            kdf: kek.kdf_config().clone(),
        }
    }

    pub fn into_kek(self) -> ProfileKek {
        ProfileKek::from_parts(self.key, self.salt, self.kdf)
    }
}

impl fmt::Debug for KekPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KekPayload { <redacted> }")
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRequest {
    Ping,
    /// Decrypt-then-unlock: the CLI hands the agent the decoded
    /// container and the KEK derived from the passphrase. The agent
    /// stores both in RAM (KEK in zeroizing storage) so it can
    /// re-encrypt the container on subsequent inner mutations
    /// without prompting the operator.
    Unlock {
        profile_set: ProfileSet,
        kek: KekPayload,
        active_alias: String,
        ttl_secs: Option<u64>,
    },
    GetProfile,
    /// Replace one inner profile inside the unlocked container,
    /// matching by `alias`. Used by the CLI after a provider mutates
    /// auth or cached lookups (OAuth refresh, root-folder-id
    /// resolution). The agent re-encrypts the container with the
    /// cached KEK and writes it back to disk.
    UpdateProfile {
        profile: PlainProfile,
    },
    /// Replace the entire in-RAM container. Used by the TUI when an
    /// inner profile is added or removed. Agent re-encrypts and
    /// persists.
    UpdateProfileSet {
        profile_set: ProfileSet,
    },
    /// Switch which inner profile is the "active" one for subsequent
    /// `GetProfile` calls. The container is not rewritten — the
    /// active alias lives only in agent RAM and in the `last-default`
    /// sidecar (written by the CLI), per resolution #4 of the
    /// container design.
    SetActiveAlias {
        alias: String,
    },
    Lock,
    Exit,
    Status,
    // ── Additive variants live below this line ──────────────────
    // Postcard encodes enum discriminants as a varint of the
    // declaration index. Inserting new variants above an existing
    // one shifts the indices of every variant after it — older
    // agents (or older clients) then mis-decode the wire form.
    // Any future variant must be appended at the bottom.
    /// SACS contextual completion: list a single remote directory so
    /// the shell completion script can offer remote names without
    /// the CLI hitting the network on every TAB. The agent serves
    /// from a 60-second cache and only contacts the provider on
    /// miss. `prefix = None` means the remote root.
    ListRemote {
        prefix: Option<String>,
        kind_filter: EntryKindFilter,
        max_results: u32,
    },
    /// Drop the cached `ListRemote` entries for a prefix (and every
    /// parent up to the root). Called by the CLI after a successful
    /// upload so the next completion reflects the mutation.
    InvalidateRemote {
        prefix: Option<String>,
    },
}

impl fmt::Debug for AgentRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ping => f.write_str("Ping"),
            Self::Unlock { ttl_secs, .. } => write!(
                f,
                "Unlock {{ profile_set: <redacted>, kek: <redacted>, active_alias: <redacted>, ttl_secs: {ttl_secs:?} }}"
            ),
            Self::GetProfile => f.write_str("GetProfile"),
            Self::UpdateProfile { .. } => f.write_str("UpdateProfile { profile: <redacted> }"),
            Self::UpdateProfileSet { .. } => {
                f.write_str("UpdateProfileSet { profile_set: <redacted> }")
            }
            Self::SetActiveAlias { .. } => {
                f.write_str("SetActiveAlias { alias: <redacted> }")
            }
            Self::ListRemote { kind_filter, max_results, .. } => write!(
                f,
                "ListRemote {{ prefix: <redacted>, kind_filter: {kind_filter:?}, max_results: {max_results} }}"
            ),
            Self::InvalidateRemote { .. } => {
                f.write_str("InvalidateRemote { prefix: <redacted> }")
            }
            Self::Lock => f.write_str("Lock"),
            Self::Exit => f.write_str("Exit"),
            Self::Status => f.write_str("Status"),
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentResponse {
    Pong,
    Unlocked,
    Profile(PlainProfile),
    /// Confirms the agent stored a new profile after `UpdateProfile`.
    Updated,
    Locked,
    Exited,
    Status {
        unlocked: bool,
        ttl_remaining_secs: Option<u64>,
    },
    Error(AgentError),
    // ── Additive variants live below this line ──────────────────
    // Same rule as `AgentRequest`: new variants append at the
    // bottom, never insert between existing ones.
    /// SACS list response. `cached_at_secs` is unix epoch seconds at
    /// the moment the agent populated this entry; clients can use
    /// it to display "fetched 8 s ago" if they want, but most don't.
    /// `truncated = true` means the underlying provider returned
    /// more entries than `max_results`; the agent kept the first N.
    RemoteList {
        entries: Vec<RemoteListEntry>,
        cached_at_secs: u64,
        truncated: bool,
    },
}

impl fmt::Debug for AgentResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pong => f.write_str("Pong"),
            Self::Unlocked => f.write_str("Unlocked"),
            Self::Profile(_) => f.write_str("Profile(<redacted>)"),
            Self::Updated => f.write_str("Updated"),
            Self::Locked => f.write_str("Locked"),
            Self::Exited => f.write_str("Exited"),
            Self::Status {
                unlocked,
                ttl_remaining_secs,
            } => write!(
                f,
                "Status {{ unlocked: {unlocked}, ttl_remaining_secs: {ttl_remaining_secs:?} }}"
            ),
            Self::RemoteList {
                entries,
                cached_at_secs,
                truncated,
            } => write!(
                f,
                "RemoteList {{ entries: <{} redacted>, cached_at_secs: {cached_at_secs}, truncated: {truncated} }}",
                entries.len()
            ),
            Self::Error(e) => write!(f, "Error({e:?})"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentError {
    #[error("protocol mismatch: got {got}, expected {expected}")]
    ProtocolMismatch { got: u16, expected: u16 },

    #[error("agent is not unlocked")]
    NotUnlocked,

    #[error("frame too large: {size} bytes (limit {limit})")]
    FrameTooLarge { size: u64, limit: u64 },

    #[error("invalid token")]
    InvalidToken,

    #[error("io error: {message}")]
    Io { message: String },

    #[error("decode error: {message}")]
    Decode { message: String },

    #[error("alias not found in unlocked container")]
    AliasNotFound,
}

/// Filter applied to a [`AgentRequest::ListRemote`] call.
/// `Both` is the default the SACS shell scripts use; the more
/// targeted variants exist so chunk E can prefer one or the
/// other when the cursor context is unambiguous (a 2nd
/// positional under `sa` is always a directory; a 1st positional
/// under `d` is always a file).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKindFilter {
    File,
    Directory,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteKind {
    File,
    Directory,
}

/// One row inside a [`AgentResponse::RemoteList`]. `mtime_secs` is
/// optional because not every backend exposes a stable last-modified
/// timestamp at list time (Google Drive does, Nextcloud WebDAV
/// does, but we keep the field optional so future provider
/// integrations don't need to pretend).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteListEntry {
    pub name: String,
    pub size: Option<u64>,
    pub kind: RemoteKind,
    pub mtime_secs: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VersionedRequest {
    pub version: u16,
    pub request: AgentRequest,
}

impl VersionedRequest {
    pub fn current(request: AgentRequest) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            request,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VersionedResponse {
    pub version: u16,
    pub response: AgentResponse,
}

impl VersionedResponse {
    pub fn current(response: AgentResponse) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            response,
        }
    }
}
