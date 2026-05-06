//! Production-side bridge from the SACS completer to the agent.
//!
//! Implements [`RemoteListSource`] by relaying calls over the
//! existing Unix socket protocol. Constructed on demand inside
//! `handle_complete` only when the agent socket exists; tests
//! plug in their own implementation directly without touching
//! this module.
//!
//! Failure is silent by design: every error variant maps to
//! [`RemoteListError`] and the completer renders fewer
//! candidates without surfacing anything to the operator.

use std::path::Path;

use zz_drop_core::AgentError;
use zz_drop_core::agent_proto::{AgentResponse, EntryKindFilter, RemoteListEntry};

use crate::agent::{AgentClient, ClientError};
use crate::sacs::complete::{RemoteListError, RemoteListSource};

const REMOTE_LIST_MAX: u32 = 200;

/// One-shot bridge over an `AgentClient`. Holds the connection
/// for the duration of a single `__complete` invocation and
/// surfaces a clean `unlocked` flag the state classifier reads
/// once.
pub struct AgentBridge {
    client: AgentClient,
    unlocked: bool,
}

impl AgentBridge {
    /// Connect, send a single `Status`, and return a ready-to-use
    /// bridge. Returns `None` if any step fails — the agent is
    /// effectively absent from the completion's point of view.
    pub fn probe(socket: &Path, token_file: &Path) -> Option<Self> {
        let mut client = AgentClient::connect(socket, token_file).ok()?;
        let unlocked = match client.status() {
            Ok(AgentResponse::Status { unlocked, .. }) => unlocked,
            _ => false,
        };
        Some(Self { client, unlocked })
    }

    pub fn unlocked(&self) -> bool {
        self.unlocked
    }
}

impl RemoteListSource for AgentBridge {
    fn list(
        &mut self,
        prefix: Option<&str>,
        kind_filter: EntryKindFilter,
    ) -> Result<Vec<RemoteListEntry>, RemoteListError> {
        match self.client.list_remote(prefix, kind_filter, REMOTE_LIST_MAX) {
            Ok(AgentResponse::RemoteList { entries, .. }) => Ok(entries),
            Ok(AgentResponse::Error(AgentError::NotUnlocked)) => Err(RemoteListError::NotUnlocked),
            Ok(AgentResponse::Error(_)) => Err(RemoteListError::Provider),
            Ok(_) => Err(RemoteListError::Provider),
            Err(ClientError::SocketUnreachable(_)) => Err(RemoteListError::Unreachable),
            Err(_) => Err(RemoteListError::Provider),
        }
    }
}
