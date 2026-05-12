//! Best-effort agent-notification helpers used by the TUI. The
//! agent lives in the `zz-drop` binary; we talk to it directly over
//! its Unix socket using the framing types in
//! `zz_drop_core::agent_proto`.
//!
//! Everything here is best-effort by design: if the socket isn't
//! there, the token file is missing, the handshake doesn't go
//! through, or the agent has already exited, we silently move on.
//! Disk state is the source of truth; these calls only keep the
//! agent's RAM cache in sync so a stale snapshot doesn't get
//! re-encrypted on top of a fresh container.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use zz_drop_core::ProfileSet;
use zz_drop_core::agent_proto::{AgentRequest, encode_request_body, read_frame, write_frame};

const TOKEN_LEN: usize = 32;
const SOCKET_TIMEOUT: Duration = Duration::from_secs(2);

/// Connect to the agent socket, perform the token handshake, and
/// send a single `Exit` request. Errors are swallowed — the caller
/// proceeds with the rest of the wipe regardless.
pub fn try_exit_agent(socket: &Path, token_file: &Path) {
    if !socket.exists() {
        return;
    }
    let _ = (|| -> std::io::Result<()> {
        let mut stream = UnixStream::connect(socket)?;
        stream.set_read_timeout(Some(SOCKET_TIMEOUT)).ok();
        stream.set_write_timeout(Some(SOCKET_TIMEOUT)).ok();

        let token = std::fs::read(token_file)?;
        if token.len() != TOKEN_LEN {
            // Wrong size = wrong shape; the agent would refuse anyway.
            return Ok(());
        }
        // Handshake: first frame is the raw token.
        if write_frame(&mut stream, &token).is_err() {
            return Ok(());
        }
        // Single request: Exit.
        if let Ok(body) = encode_request_body(&AgentRequest::Exit) {
            let _ = write_frame(&mut stream, &body);
            let _ = stream.flush();
        }
        Ok(())
    })();
}

/// Push the freshly re-encrypted container to a running agent so its
/// in-RAM `ProfileSet` doesn't fall out of sync with the disk file.
/// Without this, an agent unlocked before the TUI appended an inner
/// profile would re-encrypt its stale snapshot the next time something
/// triggers `update_profile` (e.g. an OAuth token refresh) and silently
/// overwrite the new container.
///
/// Best-effort: returns silently if the agent isn't reachable. The
/// disk file is already correct by the time this is called.
pub fn try_update_profile_set(socket: &Path, token_file: &Path, set: &ProfileSet) {
    if !socket.exists() {
        return;
    }
    let _ = (|| -> std::io::Result<()> {
        let mut stream = UnixStream::connect(socket)?;
        stream.set_read_timeout(Some(SOCKET_TIMEOUT)).ok();
        stream.set_write_timeout(Some(SOCKET_TIMEOUT)).ok();

        let token = std::fs::read(token_file)?;
        if token.len() != TOKEN_LEN {
            return Ok(());
        }
        if write_frame(&mut stream, &token).is_err() {
            return Ok(());
        }
        let req = AgentRequest::UpdateProfileSet {
            profile_set: set.clone(),
        };
        if let Ok(body) = encode_request_body(&req) {
            let _ = write_frame(&mut stream, &body);
            let _ = stream.flush();
            // Drain the response so the agent doesn't see a half-spoken
            // peer; we don't actually act on the result.
            let _ = read_frame(&mut stream);
        }
        Ok(())
    })();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_update_profile_set_is_silent_when_no_socket() {
        // Both functions are best-effort. When the socket file is
        // missing (no agent running) they must return without panic
        // or errors propagating up.
        let tmp = tempfile::tempdir().unwrap();
        let socket = tmp.path().join("missing.sock");
        let token = tmp.path().join("missing.token");
        try_update_profile_set(&socket, &token, &ProfileSet::new());
        try_exit_agent(&socket, &token);
    }
}
