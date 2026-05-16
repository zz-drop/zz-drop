use zz_drop_core::AgentResponse;
use zz_drop_core::config::Paths;
use zz_drop_core::scriptable::Reason;

use crate::agent::{AgentClient, ClientError};
use crate::commands::{EXIT_AGENT_UNREACHABLE, EXIT_OK};
use crate::output;

pub fn run(paths: &Paths) -> i32 {
    if !paths.agent_socket.exists() {
        // Already locked (no agent running). Idempotent success
        // — `zz q` is a "make sure I'm locked" verb, not a state
        // transition.
        output::emit_locked(true);
        return EXIT_OK;
    }

    let mut client = match AgentClient::connect(&paths.agent_socket, &paths.token_file) {
        Ok(c) => c,
        Err(ClientError::SocketUnreachable(_)) => {
            output::emit_locked(true);
            return EXIT_OK;
        }
        Err(e) => {
            output::emit_failed_bare(
                Reason::AgentUnreachable,
                Some(&format!("agent error: {e}")),
            );
            return EXIT_AGENT_UNREACHABLE;
        }
    };

    match client.lock() {
        Ok(AgentResponse::Locked) => {
            output::emit_locked(false);
            EXIT_OK
        }
        Ok(other) => {
            output::emit_failed_bare(
                Reason::AgentUnreachable,
                Some(&format!("unexpected agent response: {other:?}")),
            );
            EXIT_AGENT_UNREACHABLE
        }
        Err(e) => {
            output::emit_failed_bare(
                Reason::AgentUnreachable,
                Some(&format!("agent error: {e}")),
            );
            EXIT_AGENT_UNREACHABLE
        }
    }
}
