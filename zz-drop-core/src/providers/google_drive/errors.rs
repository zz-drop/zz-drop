//! Error type and short stderr mapping for the Google Drive provider.
//!
//! Mirrors the Nextcloud `diagnose` style: typed errors stay rich for
//! the TUI, the CLI maps them to a single short, sanitised line.

use thiserror::Error;

use crate::providers::oauth::DeviceFlowError;

#[derive(Debug, Error)]
pub enum GoogleDriveError {
    #[error("oauth: {0}")]
    Oauth(#[from] DeviceFlowError),

    #[error("invalid root folder")]
    BadRoot,

    #[error("token expired and refresh failed")]
    TokenExpired,

    #[error("auth failed")]
    Unauthorized,

    #[error("not found")]
    NotFound,

    #[error("conflict")]
    Conflict,

    #[error("rate limited")]
    RateLimited,

    #[error("server error: {status}")]
    ServerError { status: u16 },

    #[error("network error")]
    Network,

    #[error("local io error")]
    LocalIo,

    #[error("malformed response")]
    Decode,
}

/// Single short, sanitised line for stderr / exit-code 9 path.
/// Mirrors `nextcloud::diagnose` — keep stable for scripts.
pub fn diagnose(err: &GoogleDriveError) -> &'static str {
    match err {
        GoogleDriveError::Oauth(_) => "oauth flow error",
        GoogleDriveError::BadRoot => "invalid root folder",
        GoogleDriveError::TokenExpired => "token expired",
        GoogleDriveError::Unauthorized => "auth failed",
        GoogleDriveError::NotFound => "not found",
        GoogleDriveError::Conflict => "conflict",
        GoogleDriveError::RateLimited => "rate limited",
        GoogleDriveError::ServerError { .. } => "server error",
        GoogleDriveError::Network => "network error",
        GoogleDriveError::LocalIo => "local file error",
        GoogleDriveError::Decode => "bad server response",
    }
}
