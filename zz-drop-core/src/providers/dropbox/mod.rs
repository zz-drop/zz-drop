//! Dropbox provider — types, OAuth constants, and a validated client
//! constructor.
//!
//! Authentication uses the OAuth 2.0 Authorization Code flow with
//! PKCE and **no `redirect_uri`** (the "paste-code" / out-of-band
//! variant). Dropbox does not implement the RFC 8628 Device
//! Authorization Grant that OneDrive and Google Drive use, so the
//! generic [`crate::providers::oauth::paste_code`] helper drives
//! setup instead. Dropbox public clients with PKCE do **not** use a
//! `client_secret` — only the `client_id` ships with the binary.
//!
//! Endpoints and parameters were verified against the official
//! Dropbox OAuth guide (developers.dropbox.com/oauth-guide) on
//! 2026-05-09. The token response carries `refresh_token` only when
//! the authorize URL includes `token_access_type=offline`, which is
//! enforced by the constants below.

pub mod errors;
pub mod rest;
pub mod types;

pub use errors::{DropboxError, diagnose};
pub use rest::DropboxClient;
pub use types::{DropboxAuth, DropboxProfile, EXPIRY_SKEW_SECS};

// `DROPBOX_CLIENT_ID` is defined once, alongside every other
// provider's identifier, in `crate::providers::oauth_clients`.
// Re-exported here so existing
// `use zz_drop_core::providers::dropbox::DROPBOX_CLIENT_ID` call
// sites stay valid. See `oauth_clients` for the build-time
// override env var contract and the App-folder + PKCE
// prerequisites for the registered Dropbox app.
pub use crate::providers::oauth_clients::DROPBOX_CLIENT_ID;

/// Authorize endpoint (browser-facing). The user opens this URL,
/// grants consent, and Dropbox displays a short authorization code
/// for the user to paste back into the TUI.
pub const DROPBOX_AUTHORIZE_ENDPOINT: &str = "https://www.dropbox.com/oauth2/authorize";

/// Token endpoint: used both for the initial code → tokens exchange
/// and for refresh.
pub const DROPBOX_TOKEN_ENDPOINT: &str = "https://www.dropbox.com/oauth2/token";

/// Default folder name created under the app's sandbox the first
/// time zz-drop uploads. The user sees it under
/// `Apps/zz-drop/zz-drop/`.
pub const DROPBOX_DEFAULT_ROOT: &str = "zz-drop";

/// Documentation-only marker for the four delegated scopes wired in
/// the App Console. The Dropbox authorize URL does **not** carry a
/// `scope` parameter for our case — Dropbox derives the granted
/// scopes from the app registration. The constant lives here so a
/// reviewer can `grep DROPBOX_SCOPE` and verify which permissions
/// the binary expects to see post-consent.
pub const DROPBOX_SCOPE: &[&str] = &[
    "files.content.write",
    "files.content.read",
    "files.metadata.read",
    "account_info.read",
];

/// Build a [`crate::providers::oauth::PasteCodeConfig`] populated
/// for Dropbox. The `token_access_type=offline` extra is mandatory
/// — without it Dropbox does not issue a `refresh_token`.
pub fn paste_code_config<'a>() -> crate::providers::oauth::PasteCodeConfig<'a> {
    crate::providers::oauth::PasteCodeConfig {
        authorize_endpoint: DROPBOX_AUTHORIZE_ENDPOINT,
        token_endpoint: DROPBOX_TOKEN_ENDPOINT,
        client_id: DROPBOX_CLIENT_ID,
        authorize_extra: &[("token_access_type", "offline")],
        scope: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_auth() -> DropboxAuth {
        DropboxAuth {
            access_token: "AT-CANARY".into(),
            refresh_token: "RT-CANARY".into(),
            token_type: "bearer".into(),
            expires_at: 9_999_999_999,
            scope: DROPBOX_SCOPE.join(" "),
        }
    }

    fn sample_profile() -> DropboxProfile {
        DropboxProfile {
            root_folder: DROPBOX_DEFAULT_ROOT.into(),
            user_email: "alice@example.com".into(),
            auth: sample_auth(),
        }
    }

    #[test]
    fn debug_redacts_profile_and_auth() {
        let p = sample_profile();
        let d = format!("{p:?}");
        assert!(!d.contains("AT-CANARY"));
        assert!(!d.contains("RT-CANARY"));
        assert!(!d.contains("alice@example.com"));
        assert!(d.contains("redacted"));

        let a = sample_auth();
        let d = format!("{a:?}");
        assert!(!d.contains("AT-CANARY"));
        assert!(!d.contains("RT-CANARY"));
        assert!(d.contains("redacted"));
    }

    #[test]
    fn paste_code_config_is_populated() {
        let cfg = paste_code_config();
        assert_eq!(cfg.client_id, DROPBOX_CLIENT_ID);
        assert!(cfg.scope.is_none());
        // Without `token_access_type=offline` Dropbox does not issue
        // refresh tokens — guard against accidental removal.
        assert!(cfg.authorize_extra
            .iter()
            .any(|(k, v)| *k == "token_access_type" && *v == "offline"));
        assert!(cfg.authorize_endpoint.starts_with("https://www.dropbox.com"));
        assert!(cfg.token_endpoint.starts_with("https://www.dropbox.com"));
    }

    #[test]
    fn diagnose_lines_are_short_and_static() {
        let cases: &[DropboxError] = &[
            DropboxError::BadRoot,
            DropboxError::TokenExpired,
            DropboxError::Unauthorized,
            DropboxError::NotFound,
            DropboxError::Conflict,
            DropboxError::RateLimited,
            DropboxError::ServerError { status: 500 },
            DropboxError::Network,
            DropboxError::LocalIo,
            DropboxError::Decode,
        ];
        for e in cases {
            let line = diagnose(e);
            assert!(!line.is_empty());
            assert!(line.len() < 30, "diagnose too long: {line}");
        }
    }

    #[test]
    fn scope_list_has_minimum_four_entries() {
        // Documentation guard: the four delegated scopes registered
        // in the Dropbox App Console must stay in sync with this
        // constant. A drift here means either the binary expects
        // permissions the app does not have, or the app has been
        // over-permissioned.
        assert_eq!(DROPBOX_SCOPE.len(), 4);
        assert!(DROPBOX_SCOPE.contains(&"files.content.write"));
        assert!(DROPBOX_SCOPE.contains(&"files.content.read"));
        assert!(DROPBOX_SCOPE.contains(&"files.metadata.read"));
        assert!(DROPBOX_SCOPE.contains(&"account_info.read"));
    }
}
