//! Public HTTP API v1 surface.
//!
//! These types are wire DTOs. They mirror
//! `../docs/api/openapi.yaml` 1:1 — every field name, every
//! enum variant, and every error code must match. Server and client
//! implementations both depend on this module: it is the single source
//! of truth for the API in the Rust workspace.
//!
//! The module deliberately contains **no** transport (HTTP) logic —
//! that lives in `zz-drop-server-minimal` (server) and the consumer
//! crates that talk to the server (CLI / TUI). This split keeps the
//! DTOs reusable from inside generated stubs, tests, and the CLI's
//! API client without dragging in a web framework dependency.
//!
//! Privacy: the API types must never carry decrypted profile data,
//! profile passphrases, provider credentials, or provider metadata. If
//! you find yourself adding such a field, stop — it belongs in the
//! encrypted `profile.zz` blob, not on the wire.

pub mod client;
pub mod dto;
pub mod errors;
pub mod version;

pub use client::{ApiClient, ApiClientError, LoginOutcome};
pub use dto::{
    Alias, AliasError, CreateProfileRequest, EmailPreferences, EmailPreferencesUpdate, Info,
    LoginRequest, LoginResponse, LoginTotpChallenge, ProfileList, ProfileSummary, RegisterRequest,
    TotpDisableRequest, TotpEnrollResponse, TotpLoginRequest, TotpVerifyRequest,
    is_plausible_email,
};
pub use errors::{ApiErrorBody, ApiErrorCode};
pub use version::{API_VERSION, BASE_PATH};
