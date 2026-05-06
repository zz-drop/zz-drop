use zz_drop_core::AgentResponse;
use zz_drop_core::config::Paths;

use crate::agent::{AgentClient, ClientError};
use crate::commands::{EXIT_AGENT_UNREACHABLE, EXIT_OK};
use crate::output;

pub fn run(paths: &Paths) -> i32 {
    if !paths.agent_socket.exists() {
        // Already locked (no agent running).
        output::line("already locked");
        return EXIT_OK;
    }

    let mut client = match AgentClient::connect(&paths.agent_socket, &paths.token_file) {
        Ok(c) => c,
        Err(ClientError::SocketUnreachable(_)) => {
            output::line("already locked");
            return EXIT_OK;
        }
        Err(e) => {
            output::err_line(&format!("agent error: {e}"));
            return EXIT_AGENT_UNREACHABLE;
        }
    };

    match client.lock() {
        Ok(AgentResponse::Locked) => {
            output::line("locked");
            EXIT_OK
        }
        Ok(other) => {
            output::err_line(&format!("unexpected agent response: {other:?}"));
            EXIT_AGENT_UNREACHABLE
        }
        Err(e) => {
            output::err_line(&format!("agent error: {e}"));
            EXIT_AGENT_UNREACHABLE
        }
    }
}
