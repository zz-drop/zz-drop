//! Error type and short stderr mapping for the OneDrive provider.
//!
//! Mirrors `google_drive::errors`: typed errors stay rich for the
//! TUI, the CLI maps them to a single short, sanitised line.

use thiserror::Error;

use crate::providers::oauth::DeviceFlowError;

#[derive(Debug, Error)]
pub enum OneDriveError {
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
/// Mirrors `google_drive::diagnose` — keep stable for scripts.
pub fn diagnose(err: &OneDriveError) -> &'static str {
    match err {
        OneDriveError::Oauth(_) => "oauth flow error",
        OneDriveError::BadRoot => "invalid root folder",
        OneDriveError::TokenExpired => "token expired",
        OneDriveError::Unauthorized => "auth failed",
        OneDriveError::NotFound => "not found",
        OneDriveError::Conflict => "conflict",
        OneDriveError::RateLimited => "rate limited",
        OneDriveError::ServerError { .. } => "server error",
        OneDriveError::Network => "network error",
        OneDriveError::LocalIo => "local file error",
        OneDriveError::Decode => "bad server response",
    }
}
