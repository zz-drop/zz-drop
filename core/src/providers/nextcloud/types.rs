use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct NextcloudProfile {
    pub server_url: String,
    pub username: String,
    pub auth: NextcloudAuth,
    pub remote_root: String,
}

impl fmt::Debug for NextcloudProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NextcloudProfile { <redacted> }")
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextcloudAuth {
    AppPassword { secret: String },
    LoginFlowToken { secret: String },
}

impl fmt::Debug for NextcloudAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppPassword { .. } => f.write_str("AppPassword { secret: <redacted> }"),
            Self::LoginFlowToken { .. } => f.write_str("LoginFlowToken { secret: <redacted> }"),
        }
    }
}
