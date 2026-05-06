use std::fmt;

use serde::{Deserialize, Serialize};

use crate::providers::{CollisionPolicy, ProviderProfile};

pub const PROFILE_VERSION_V1: u32 = 1;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct PlainProfile {
    pub profile_version: u32,
    pub profile_id: String,
    pub alias: String,
    pub default_target: String,
    pub providers: Vec<ProviderProfile>,
    pub collision_policy: CollisionPolicy,
    #[serde(default)]
    pub settings: ProfileSettings,
    pub created_at: String,
    pub updated_at: String,
}

impl fmt::Debug for PlainProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PlainProfile { <redacted> }")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSettings {
    #[serde(default = "default_unlock_ttl_secs")]
    pub unlock_ttl_secs: u64,
    #[serde(default = "default_agent_idle_exit_secs")]
    pub agent_idle_exit_secs: u64,
}

fn default_unlock_ttl_secs() -> u64 {
    600
}

fn default_agent_idle_exit_secs() -> u64 {
    300
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            unlock_ttl_secs: default_unlock_ttl_secs(),
            agent_idle_exit_secs: default_agent_idle_exit_secs(),
        }
    }
}
