//! Error type and short stderr mapping for the Dropbox provider.
//!
//! Mirrors `onedrive::errors`: typed errors stay rich for the TUI,
//! the CLI maps them to a single short, sanitised line.

use thiserror::Error;

use crate::providers::oauth::PasteCodeError;

#[derive(Debug, Error)]
pub enum DropboxError {
    #[error("oauth: {0}")]
    Oauth(#[from] PasteCodeError),

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
/// Mirrors `onedrive::diagnose` — keep stable for scripts.
pub fn diagnose(err: &DropboxError) -> &'static str {
    match err {
        DropboxError::Oauth(_) => "oauth flow error",
        DropboxError::BadRoot => "invalid root folder",
        DropboxError::TokenExpired => "token expired",
        DropboxError::Unauthorized => "auth failed",
        DropboxError::NotFound => "not found",
        DropboxError::Conflict => "conflict",
        DropboxError::RateLimited => "rate limited",
        DropboxError::ServerError { .. } => "server error",
        DropboxError::Network => "network error",
        DropboxError::LocalIo => "local file error",
        DropboxError::Decode => "bad server response",
    }
}
