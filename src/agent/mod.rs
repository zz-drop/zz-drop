pub mod client;
pub mod list_cache;
pub mod lock;
pub mod remote_client;
pub mod security;
pub mod server;
pub mod state;

pub use client::{AgentClient, ClientError};
pub use server::{
    DEFAULT_IDLE_EXIT_SECS, DEFAULT_TTL_SECS, POLL_INTERVAL_MS, ServerConfig, ServerError, run,
};
pub use state::AgentState;

pub const AGENT_MODE_ENV: &str = "ZZ_DROP_AGENT_MODE";

pub fn is_agent_mode() -> bool {
    std::env::var(AGENT_MODE_ENV)
        .map(|v| v == "1")
        .unwrap_or(false)
}
