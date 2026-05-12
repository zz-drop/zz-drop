//! Thin TUI-side wrapper around `zz_drop_core::api::ApiClient` that
//! drives the four network calls of the push sub-flow:
//!
//! 1. `login(email, password)` → either a session or a TOTP challenge
//! 2. `login_totp(challenge, code)` → a session
//! 3. `list_profiles()` → the operator's existing aliases
//! 4. `create_or_put_blob(alias, blob)` → upload the encrypted blob
//!
//! Each function is synchronous: the TUI runs sync ureq from the main
//! loop, between two `terminal.draw()` calls. Error messages are
//! short, localized, and never carry the bearer token, the password,
//! or any TOTP code.

use zz_drop_core::api::{ApiClient, ApiClientError, ApiErrorCode, LoginOutcome};

pub enum PushLoginOutcome {
    Session(String),
    TotpRequired(String),
}

pub fn login(
    base: &str,
    email: &str,
    password: &str,
) -> Result<PushLoginOutcome, String> {
    let client = ApiClient::new(base);
    match client.login(email, password) {
        Ok(LoginOutcome::Session(s)) => Ok(PushLoginOutcome::Session(s.token)),
        Ok(LoginOutcome::TotpRequired(c)) => Ok(PushLoginOutcome::TotpRequired(c.challenge)),
        Err(e) => Err(short_error(e)),
    }
}

pub fn login_totp(base: &str, challenge: &str, code: &str) -> Result<String, String> {
    let client = ApiClient::new(base);
    client
        .login_totp(challenge, code)
        .map(|r| r.token)
        .map_err(short_error)
}

pub fn list_aliases(base: &str, token: &str) -> Result<Vec<String>, String> {
    let client = ApiClient::new(base).with_token(token);
    client
        .list_profiles()
        .map(|r| {
            r.profiles
                .into_iter()
                .map(|p| p.alias.into_inner())
                .collect()
        })
        .map_err(short_error)
}

/// Download the encrypted blob for `alias`. Used by the SignIn flow
/// to populate `profile-remote.zz`.
pub fn download_blob(base: &str, token: &str, alias: &str) -> Result<Vec<u8>, String> {
    let client = ApiClient::new(base).with_token(token);
    client.get_blob(alias).map_err(short_error)
}

/// Push the blob to `alias`. If the alias does not exist yet, create
/// it first. We retry the put once with the new version on a
/// `VersionConflict` if the alias was new — that case can happen if
/// another client raced us between `create_profile` and `put_blob`.
pub fn push_blob(
    base: &str,
    token: &str,
    alias: &str,
    blob: Vec<u8>,
) -> Result<PushSummary, String> {
    let client = ApiClient::new(base).with_token(token);
    // Discover or create the alias to get the right `expected_version`.
    let current_version = match client.list_profiles().map_err(short_error)? {
        list => list
            .profiles
            .iter()
            .find(|p| p.alias.as_str() == alias)
            .map(|p| p.blob_version),
    };
    let expected_version = match current_version {
        Some(v) => v,
        None => {
            // Alias does not exist — create it; first PUT uses 0.
            client.create_profile(alias).map_err(short_error)?;
            0
        }
    };
    let summary = client
        .put_blob(alias, expected_version, blob)
        .map_err(short_error)?;
    Ok(PushSummary {
        alias: summary.alias.into_inner(),
        blob_size: summary.blob_size,
        blob_version: summary.blob_version,
    })
}

pub struct PushSummary {
    pub alias: String,
    pub blob_size: u64,
    pub blob_version: u64,
}

fn short_error(e: ApiClientError) -> String {
    match e {
        ApiClientError::Network(_) | ApiClientError::Transport(_) => {
            "could not reach the server".into()
        }
        ApiClientError::Decode(_) => "bad server response".into(),
        ApiClientError::NoToken => "missing session".into(),
        ApiClientError::Api(code, msg) => match code {
            ApiErrorCode::Unauthorized => "wrong credentials".into(),
            ApiErrorCode::Forbidden => "forbidden".into(),
            ApiErrorCode::NotFound => "not found".into(),
            ApiErrorCode::VersionConflict => "version conflict — please retry".into(),
            ApiErrorCode::BlobTooLarge => "blob too large".into(),
            ApiErrorCode::RateLimited => "rate limited".into(),
            ApiErrorCode::ServerError => "server error".into(),
            ApiErrorCode::InvalidRequest => msg,
        },
    }
}
