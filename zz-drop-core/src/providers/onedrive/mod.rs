//! Microsoft OneDrive provider — types, OAuth constants and a
//! validated client constructor.
//!
//! Authentication uses the OAuth 2.0 Device Authorization Grant
//! (RFC 8628), reusing the generic [`crate::providers::oauth::device_flow`]
//! client. Microsoft public clients (the "Allow public client flows"
//! setting on an app registration) do **not** require a
//! `client_secret` on device flow — only the `client_id` ships with
//! the binary.
//!
//! Endpoints and parameters were verified against Microsoft Learn
//! "OAuth 2.0 device authorization grant" (page last updated
//! 2025-10-02). The token response carries `refresh_token` only when
//! the request scope includes `offline_access`, which is enforced by
//! the scope constant below.

pub mod errors;
pub mod rest;
pub mod types;

pub use errors::{OneDriveError, diagnose};
pub use rest::OneDriveClient;
pub use types::{EXPIRY_SKEW_SECS, OneDriveAuth, OneDriveProfile};

// `ONEDRIVE_CLIENT_ID` is defined once, alongside every other
// provider's identifier, in `crate::providers::oauth_clients`.
// Re-exported here so existing
// `use zz_drop_core::providers::onedrive::ONEDRIVE_CLIENT_ID`
// call sites stay valid. See `oauth_clients` for the build-time
// override env var contract and the app-registration prerequisites
// (multi-tenant + personal accounts, "Allow public client flows"
// on, the three delegated scopes listed below).
pub use crate::providers::oauth_clients::ONEDRIVE_CLIENT_ID;

/// Tenant path. `/common` accepts both personal Microsoft accounts
/// and work/school accounts; `/consumers` would lock the app to
/// personal accounts only. zz-drop targets the broader audience.
pub const ONEDRIVE_TENANT: &str = "common";

/// `Files.ReadWrite`  — read/write access to the user's OneDrive.
/// `offline_access`   — required for the token endpoint to issue a
///                       refresh_token (Microsoft does NOT issue
///                       refresh tokens unless this scope is asked).
/// `User.Read`        — needed to fetch `mail` / `userPrincipalName`
///                       for the display label on the wizard / pill.
pub const ONEDRIVE_SCOPE: &str = "Files.ReadWrite offline_access User.Read";

/// RFC 8628 device authorization endpoint for Microsoft identity
/// platform v2.
pub const ONEDRIVE_DEVICE_CODE_ENDPOINT: &str =
    "https://login.microsoftonline.com/common/oauth2/v2.0/devicecode";

/// Token endpoint: used both for the polling exchange and for refresh.
pub const ONEDRIVE_TOKEN_ENDPOINT: &str =
    "https://login.microsoftonline.com/common/oauth2/v2.0/token";

/// Default folder name created under the user's OneDrive root the
/// first time zz-drop uploads.
pub const ONEDRIVE_DEFAULT_ROOT: &str = "zz-drop";

/// Build a [`crate::providers::oauth::DeviceFlowConfig`] populated
/// for Microsoft. Public clients (device flow on Microsoft) ship
/// without a `client_secret`.
pub fn device_flow_config<'a>() -> crate::providers::oauth::DeviceFlowConfig<'a> {
    crate::providers::oauth::DeviceFlowConfig {
        device_code_endpoint: ONEDRIVE_DEVICE_CODE_ENDPOINT,
        token_endpoint: ONEDRIVE_TOKEN_ENDPOINT,
        client_id: ONEDRIVE_CLIENT_ID,
        client_secret: None,
        scope: ONEDRIVE_SCOPE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_auth() -> OneDriveAuth {
        OneDriveAuth {
            access_token: "AT-CANARY".into(),
            refresh_token: "RT-CANARY".into(),
            token_type: "Bearer".into(),
            expires_at: 9_999_999_999,
            scope: ONEDRIVE_SCOPE.into(),
        }
    }

    fn sample_profile() -> OneDriveProfile {
        OneDriveProfile {
            root_folder: ONEDRIVE_DEFAULT_ROOT.into(),
            user_email: "alice@outlook.com".into(),
            root_folder_id: None,
            auth: sample_auth(),
        }
    }

    #[test]
    fn debug_redacts_profile_and_auth() {
        let p = sample_profile();
        let d = format!("{p:?}");
        assert!(!d.contains("AT-CANARY"));
        assert!(!d.contains("RT-CANARY"));
        assert!(!d.contains("alice@outlook.com"));
        assert!(d.contains("redacted"));

        let a = sample_auth();
        let d = format!("{a:?}");
        assert!(!d.contains("AT-CANARY"));
        assert!(!d.contains("RT-CANARY"));
        assert!(d.contains("redacted"));
    }

    #[test]
    fn device_flow_config_is_populated() {
        let cfg = device_flow_config();
        assert_eq!(cfg.client_id, ONEDRIVE_CLIENT_ID);
        // Microsoft public clients DO NOT use a client secret on
        // device flow — keep this assertion as a guard against
        // accidentally re-introducing one in the future.
        assert!(cfg.client_secret.is_none());
        assert!(cfg.scope.contains("offline_access"));
        assert!(cfg.scope.contains("Files.ReadWrite"));
        assert!(cfg.device_code_endpoint.starts_with("https://login.microsoftonline.com"));
        assert!(cfg.token_endpoint.starts_with("https://login.microsoftonline.com"));
    }

    #[test]
    fn diagnose_lines_are_short_and_static() {
        let cases: &[OneDriveError] = &[
            OneDriveError::BadRoot,
            OneDriveError::TokenExpired,
            OneDriveError::Unauthorized,
            OneDriveError::NotFound,
            OneDriveError::Conflict,
            OneDriveError::RateLimited,
            OneDriveError::ServerError { status: 500 },
            OneDriveError::Network,
            OneDriveError::LocalIo,
            OneDriveError::Decode,
        ];
        for e in cases {
            let line = diagnose(e);
            assert!(!line.is_empty());
            assert!(line.len() < 30, "diagnose too long: {line}");
        }
    }
}
