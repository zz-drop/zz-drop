use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Wire-format error body returned by every endpoint on a non-2xx
/// response. The `error` field is a stable machine-readable code; the
/// `message` is human-readable and free-form (must not leak credentials
/// or stack traces).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub error: ApiErrorCode,
    pub message: String,
}

impl ApiErrorBody {
    pub fn new(code: ApiErrorCode, message: impl Into<String>) -> Self {
        Self {
            error: code,
            message: message.into(),
        }
    }
}

/// Stable machine-readable error codes. Servers MUST return exactly one
/// of these in the `error` field; clients MUST treat unknown codes as
/// `server_error`. The set is closed by design — adding a code is a
/// breaking change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Error)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    /// 400 — request body malformed, missing required fields, or
    /// fields out of range.
    #[error("invalid request")]
    InvalidRequest,
    /// 401 — no session token, expired token, or wrong credentials.
    #[error("unauthorized")]
    Unauthorized,
    /// 403 — token is valid but does not own the targeted resource.
    #[error("forbidden")]
    Forbidden,
    /// 404 — resource (alias, account) does not exist.
    #[error("not found")]
    NotFound,
    /// 409 — `expected_version` did not match the current blob, or the
    /// alias is already taken.
    #[error("version conflict")]
    VersionConflict,
    /// 413 — blob exceeds the server's per-account size limit.
    #[error("blob too large")]
    BlobTooLarge,
    /// 429 — caller is being rate-limited.
    #[error("rate limited")]
    RateLimited,
    /// 500 — unexpected server-side error.
    #[error("server error")]
    ServerError,
}

impl ApiErrorCode {
    /// HTTP status the server is expected to attach to this code.
    /// Clients can rely on the wire `error` code rather than the
    /// status, but the mapping is fixed for tests and for servers
    /// that build their response status from the code.
    pub fn status_code(self) -> u16 {
        match self {
            Self::InvalidRequest => 400,
            Self::Unauthorized => 401,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::VersionConflict => 409,
            Self::BlobTooLarge => 413,
            Self::RateLimited => 429,
            Self::ServerError => 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_body_round_trip_json() {
        let body = ApiErrorBody::new(ApiErrorCode::Unauthorized, "missing token");
        let s = serde_json::to_string(&body).unwrap();
        assert_eq!(s, r#"{"error":"unauthorized","message":"missing token"}"#);
        let back: ApiErrorBody = serde_json::from_str(&s).unwrap();
        assert_eq!(back, body);
    }

    #[test]
    fn unknown_error_code_fails_to_deserialize() {
        let s = r#"{"error":"teapot","message":"i am a teapot"}"#;
        let r: Result<ApiErrorBody, _> = serde_json::from_str(s);
        assert!(r.is_err());
    }

    #[test]
    fn status_code_matches_spec_table() {
        assert_eq!(ApiErrorCode::InvalidRequest.status_code(), 400);
        assert_eq!(ApiErrorCode::Unauthorized.status_code(), 401);
        assert_eq!(ApiErrorCode::Forbidden.status_code(), 403);
        assert_eq!(ApiErrorCode::NotFound.status_code(), 404);
        assert_eq!(ApiErrorCode::VersionConflict.status_code(), 409);
        assert_eq!(ApiErrorCode::BlobTooLarge.status_code(), 413);
        assert_eq!(ApiErrorCode::RateLimited.status_code(), 429);
        assert_eq!(ApiErrorCode::ServerError.status_code(), 500);
    }

    #[test]
    fn snake_case_serialization() {
        for (code, wire) in [
            (ApiErrorCode::VersionConflict, "version_conflict"),
            (ApiErrorCode::BlobTooLarge, "blob_too_large"),
            (ApiErrorCode::RateLimited, "rate_limited"),
            (ApiErrorCode::ServerError, "server_error"),
        ] {
            assert_eq!(serde_json::to_string(&code).unwrap(), format!("\"{wire}\""));
        }
    }
}
