use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Alias ───────────────────────────────────────────────────────────

/// Profile alias. Globally unique, lowercase, charset `a-z0-9._-`,
/// length 4–32 characters. Construction via `Alias::new` (or
/// `TryFrom<&str>` / `TryFrom<String>`) is the only way to mint one;
/// deserialization runs the same validator.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Alias(String);

impl Alias {
    pub const MIN_LEN: usize = 4;
    pub const MAX_LEN: usize = 32;

    pub fn new(s: impl Into<String>) -> Result<Self, AliasError> {
        let s = s.into();
        validate_alias(&s)?;
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for Alias {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for Alias {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<&str> for Alias {
    type Error = AliasError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Alias::new(s)
    }
}

impl TryFrom<String> for Alias {
    type Error = AliasError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Alias::new(s)
    }
}

impl Serialize for Alias {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Alias {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        Alias::new(s).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum AliasError {
    #[error("alias too short (min {min})", min = Alias::MIN_LEN)]
    TooShort,
    #[error("alias too long (max {max})", max = Alias::MAX_LEN)]
    TooLong,
    #[error("alias contains an illegal character")]
    BadCharacter,
}

fn validate_alias(s: &str) -> Result<(), AliasError> {
    let len = s.chars().count();
    if len < Alias::MIN_LEN {
        return Err(AliasError::TooShort);
    }
    if len > Alias::MAX_LEN {
        return Err(AliasError::TooLong);
    }
    for c in s.chars() {
        let ok = c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-');
        if !ok {
            return Err(AliasError::BadCharacter);
        }
    }
    Ok(())
}

// ─── /info ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    pub api_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implementation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

// ─── /auth ───────────────────────────────────────────────────────────

/// Plausibility check for an email address. Shared between the server
/// (which rejects malformed registrations / logins) and the CLI/TUI
/// (which want to fail fast before a network round-trip), so the three
/// surfaces agree on what "looks like an email" means.
///
/// We are deliberately not in the business of validating RFC 5322. The
/// real validation step is the email-confirmation flow. The check here
/// only filters out obviously broken input: whitespace, missing parts,
/// multiple `@`, or a domain without an interior dot.
pub fn is_plausible_email(s: &str) -> bool {
    if s.is_empty() || s.len() > 254 {
        return false;
    }
    if s.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    let mut split = s.splitn(2, '@');
    let local = split.next().unwrap_or("");
    let domain = split.next().unwrap_or("");
    if local.is_empty() || domain.is_empty() || domain.contains('@') {
        return false;
    }
    match domain.find('.') {
        Some(i) => i > 0 && i < domain.len() - 1,
        None => false,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_in: u64,
}

// ─── /profiles ───────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub alias: Alias,
    pub blob_size: u64,
    pub blob_version: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileList {
    pub profiles: Vec<ProfileSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateProfileRequest {
    pub alias: Alias,
}

// ─── /auth/totp ──────────────────────────────────────────────────────

/// Returned by `POST /auth/login` when the account has TOTP enabled.
/// The client must follow up with `POST /auth/totp/login` carrying the
/// `challenge` plus a 6-digit code (or a recovery code) within
/// `expires_in` seconds. `totp_required` is always `true` and exists
/// purely as a discriminator vs. the no-TOTP `LoginResponse`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginTotpChallenge {
    pub totp_required: bool,
    pub challenge: String,
    pub expires_in: u64,
}

/// Returned by `POST /auth/totp/enroll`. The `secret_base32` is the
/// shared seed encoded for manual entry in apps that don't scan QRs;
/// `otpauth_uri` is the `otpauth://totp/...` URI typically rendered as
/// a QR. `recovery_codes` are shown to the user **once** — they are
/// not retrievable later.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotpEnrollResponse {
    pub otpauth_uri: String,
    pub secret_base32: String,
    pub recovery_codes: Vec<String>,
}

/// `POST /auth/totp/verify` body. Exchanges a 6-digit code against the
/// pending enrollment to activate it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotpVerifyRequest {
    pub code: String,
}

/// `POST /auth/totp/login` body. Step 2 of the two-step login.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotpLoginRequest {
    pub challenge: String,
    pub code: String,
}

/// `POST /auth/totp/disable` body. Requires the account password plus
/// either a current TOTP code or one recovery code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotpDisableRequest {
    pub password: String,
    pub code: String,
}

// ─── /account/email-preferences ──────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailPreferences {
    pub security_events: bool,
    pub profile_activity: bool,
    pub product_updates: bool,
}

impl EmailPreferences {
    /// `security_events` is non-disableable: the server must always
    /// report `true`. Use this constructor on the server side to
    /// guarantee the invariant.
    pub fn new(profile_activity: bool, product_updates: bool) -> Self {
        Self {
            security_events: true,
            profile_activity,
            product_updates,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EmailPreferencesUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_activity: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_updates: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Alias ────────────────────────────────────────────────────

    #[test]
    fn alias_accepts_lowercase_alnum_and_separators() {
        for s in ["alice", "user.42", "a-b_c-9", "abcd", "a".repeat(32).as_str()] {
            assert!(Alias::new(s).is_ok(), "rejected `{s}`");
        }
    }

    #[test]
    fn alias_rejects_too_short() {
        assert_eq!(Alias::new("abc").unwrap_err(), AliasError::TooShort);
    }

    #[test]
    fn alias_rejects_too_long() {
        assert_eq!(
            Alias::new("a".repeat(33)).unwrap_err(),
            AliasError::TooLong
        );
    }

    #[test]
    fn alias_rejects_uppercase_and_non_ascii() {
        for s in ["Alice", "user@host", "ñiño", "user 42", "a/b/c"] {
            assert_eq!(
                Alias::new(s).unwrap_err(),
                AliasError::BadCharacter,
                "should reject `{s}`"
            );
        }
    }

    #[test]
    fn alias_round_trips_json() {
        let a = Alias::new("alice").unwrap();
        let s = serde_json::to_string(&a).unwrap();
        assert_eq!(s, "\"alice\"");
        let back: Alias = serde_json::from_str(&s).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn alias_deserialization_runs_validator() {
        let r: Result<Alias, _> = serde_json::from_str("\"BadAlias\"");
        assert!(r.is_err(), "uppercase should fail through serde too");
    }

    // ── is_plausible_email ──────────────────────────────────────

    #[test]
    fn email_accepts_basic_shapes() {
        for s in [
            "alice@example.org",
            "a@b.c",
            "user.name+tag@sub.example.co.uk",
            "x@y.z",
        ] {
            assert!(is_plausible_email(s), "should accept `{s}`");
        }
    }

    #[test]
    fn email_rejects_obviously_broken() {
        for s in [
            "",
            "noatsign",
            "@nolocal.org",
            "missing-domain@",
            "missing-dot@example",
            "two@@signs.org",
            "spaces in@example.org",
        ] {
            assert!(!is_plausible_email(s), "should reject `{s}`");
        }
    }
}
