// Cross-checks the API DTOs against canonical JSON examples that mirror
// the OpenAPI spec at `core/docs/api/openapi.yaml`. If any of
// these tests fail, either the wire format or the OpenAPI spec drifted
// — fix both in the same change.

use zz_drop_core::api::{
    Alias, ApiErrorBody, ApiErrorCode, CreateProfileRequest, EmailPreferences,
    EmailPreferencesUpdate, Info, LoginRequest, LoginResponse, ProfileList, ProfileSummary,
    RegisterRequest, BASE_PATH,
};

#[test]
fn base_path_is_v1() {
    assert_eq!(BASE_PATH, "/api/v1");
}

#[test]
fn info_optional_fields_omitted_when_none() {
    let info = Info {
        api_version: "1".into(),
        implementation: None,
        notes: None,
    };
    let s = serde_json::to_string(&info).unwrap();
    assert_eq!(s, r#"{"api_version":"1"}"#);
}

#[test]
fn info_full_round_trip() {
    let info = Info {
        api_version: "1".into(),
        implementation: Some("zz-drop-server-minimal".into()),
        notes: Some("hosted at zz-drop.net".into()),
    };
    let s = serde_json::to_string(&info).unwrap();
    let back: Info = serde_json::from_str(&s).unwrap();
    assert_eq!(back, info);
}

#[test]
fn register_request_round_trip() {
    let req = RegisterRequest {
        email: "alice@example.org".into(),
        password: "very-strong-passphrase".into(),
    };
    let s = serde_json::to_string(&req).unwrap();
    let back: RegisterRequest = serde_json::from_str(&s).unwrap();
    assert_eq!(back, req);
}

#[test]
fn login_response_uses_snake_case_for_expires_in() {
    let r = LoginResponse {
        token: "opaque-token".into(),
        expires_in: 86_400,
    };
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains("\"expires_in\":86400"), "got `{s}`");
}

#[test]
fn login_request_round_trip() {
    let req = LoginRequest {
        email: "alice@example.org".into(),
        password: "p".into(),
    };
    let s = serde_json::to_string(&req).unwrap();
    let back: LoginRequest = serde_json::from_str(&s).unwrap();
    assert_eq!(back, req);
}

#[test]
fn profile_summary_field_order_matches_spec() {
    // The spec orders fields as alias, blob_size, blob_version, created_at, updated_at.
    // serde derive preserves field order; we just round-trip and check the JSON.
    let p = ProfileSummary {
        alias: Alias::new("casa-nc").unwrap(),
        blob_size: 1024,
        blob_version: 7,
        created_at: "2026-04-28T08:00:00Z".into(),
        updated_at: "2026-04-28T09:30:00Z".into(),
    };
    let s = serde_json::to_string(&p).unwrap();
    assert_eq!(
        s,
        r#"{"alias":"casa-nc","blob_size":1024,"blob_version":7,"created_at":"2026-04-28T08:00:00Z","updated_at":"2026-04-28T09:30:00Z"}"#
    );
    let back: ProfileSummary = serde_json::from_str(&s).unwrap();
    assert_eq!(back, p);
}

#[test]
fn profile_list_round_trip_with_two_entries() {
    let list = ProfileList {
        profiles: vec![
            ProfileSummary {
                alias: Alias::new("casa-nc").unwrap(),
                blob_size: 512,
                blob_version: 1,
                created_at: "2026-04-28T08:00:00Z".into(),
                updated_at: "2026-04-28T08:00:00Z".into(),
            },
            ProfileSummary {
                alias: Alias::new("work.nc").unwrap(),
                blob_size: 1024,
                blob_version: 3,
                created_at: "2026-04-28T08:01:00Z".into(),
                updated_at: "2026-04-28T08:30:00Z".into(),
            },
        ],
    };
    let s = serde_json::to_string(&list).unwrap();
    let back: ProfileList = serde_json::from_str(&s).unwrap();
    assert_eq!(back, list);
}

#[test]
fn create_profile_request_uses_validated_alias() {
    let s = r#"{"alias":"casa-nc"}"#;
    let req: CreateProfileRequest = serde_json::from_str(s).unwrap();
    assert_eq!(req.alias.as_str(), "casa-nc");

    // Invalid alias must be rejected at deserialization time, not silently.
    let bad = r#"{"alias":"NoUppercase"}"#;
    let r: Result<CreateProfileRequest, _> = serde_json::from_str(bad);
    assert!(r.is_err());
}

#[test]
fn email_preferences_security_events_always_true_via_constructor() {
    let p = EmailPreferences::new(true, false);
    assert!(p.security_events);
    let s = serde_json::to_string(&p).unwrap();
    assert!(s.contains("\"security_events\":true"));
    assert!(s.contains("\"profile_activity\":true"));
    assert!(s.contains("\"product_updates\":false"));
}

#[test]
fn email_preferences_update_omits_unset_fields() {
    let upd = EmailPreferencesUpdate {
        profile_activity: Some(false),
        product_updates: None,
    };
    let s = serde_json::to_string(&upd).unwrap();
    assert_eq!(s, r#"{"profile_activity":false}"#);
}

#[test]
fn empty_email_preferences_update_serializes_to_empty_object() {
    let upd = EmailPreferencesUpdate::default();
    assert_eq!(serde_json::to_string(&upd).unwrap(), "{}");
}

#[test]
fn api_error_body_canonical_examples() {
    for (code, wire) in [
        (ApiErrorCode::Unauthorized, "unauthorized"),
        (ApiErrorCode::VersionConflict, "version_conflict"),
        (ApiErrorCode::BlobTooLarge, "blob_too_large"),
        (ApiErrorCode::RateLimited, "rate_limited"),
    ] {
        let body = ApiErrorBody::new(code, "human readable");
        let s = serde_json::to_string(&body).unwrap();
        assert_eq!(
            s,
            format!(r#"{{"error":"{wire}","message":"human readable"}}"#)
        );
    }
}

#[test]
fn no_dto_field_carries_provider_metadata() {
    // Defensive sanity: the public DTOs must not have any field that
    // names a provider, an OAuth token, or a passphrase. This is a
    // grep-as-test; it catches accidental field names early.
    let snapshot = serde_json::to_string(&Info {
        api_version: "1".into(),
        implementation: Some("test".into()),
        notes: Some("test".into()),
    })
    .unwrap()
        + &serde_json::to_string(&LoginResponse {
            token: "t".into(),
            expires_in: 1,
        })
        .unwrap()
        + &serde_json::to_string(&ProfileSummary {
            alias: Alias::new("test").unwrap(),
            blob_size: 0,
            blob_version: 0,
            created_at: String::new(),
            updated_at: String::new(),
        })
        .unwrap();
    for forbidden in [
        "passphrase",
        "password_hash",
        "nextcloud",
        "webdav",
        "oauth",
        "provider",
        "secret",
    ] {
        assert!(
            !snapshot.contains(forbidden),
            "DTO leaked `{forbidden}` in `{snapshot}`"
        );
    }
}
