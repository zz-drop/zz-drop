use base64::{Engine, engine::general_purpose::STANDARD as B64};
use zz_drop_core::profile::format::{
    load_profile_zz, save_profile_zz_with_config,
};
use zz_drop_core::{
    Argon2idConfig, CollisionPolicy, NextcloudAuth, NextcloudProfile, PlainProfile,
    ProfileCryptoError, ProfileSettings, ProviderProfile, decrypt_profile, encrypt_profile,
    encrypt_profile_with_config,
};

const TEST_PASSPHRASE: &str = "correct horse battery staple";
const WRONG_PASSPHRASE: &str = "Tr0ub4dor&3";

const FAST_KDF: Argon2idConfig = Argon2idConfig {
    memory_kib: 8 * 1024,
    iterations: 1,
    parallelism: 1,
};

fn sample_profile() -> PlainProfile {
    PlainProfile {
        profile_version: 1,
        profile_id: "p-0001".into(),
        alias: "casa-nc".into(),
        default_target: "nextcloud-1".into(),
        providers: vec![ProviderProfile::Nextcloud(NextcloudProfile {
            server_url: "https://example.org".into(),
            username: "user".into(),
            auth: NextcloudAuth::AppPassword {
                secret: "topsecret".into(),
            },
            remote_root: "/zz-drop".into(),
        })],
        collision_policy: CollisionPolicy::Rename,
        settings: ProfileSettings::default(),
        created_at: "2026-04-25T22:00:00Z".into(),
        updated_at: "2026-04-25T22:00:00Z".into(),
    }
}

#[test]
fn default_config_pinned() {
    assert_eq!(Argon2idConfig::DEFAULT.memory_kib, 194_560);
    assert_eq!(Argon2idConfig::DEFAULT.iterations, 3);
    assert_eq!(Argon2idConfig::DEFAULT.parallelism, 1);
}

#[test]
fn encrypt_then_decrypt_round_trip() {
    let profile = sample_profile();
    let envelope = encrypt_profile_with_config(&profile, TEST_PASSPHRASE, &FAST_KDF).unwrap();
    let restored = decrypt_profile(&envelope, TEST_PASSPHRASE).unwrap();
    assert!(profile == restored);
}

#[test]
fn decrypt_with_wrong_passphrase_fails() {
    let profile = sample_profile();
    let envelope = encrypt_profile_with_config(&profile, TEST_PASSPHRASE, &FAST_KDF).unwrap();
    let err = decrypt_profile(&envelope, WRONG_PASSPHRASE).unwrap_err();
    assert!(
        matches!(err, ProfileCryptoError::Aead),
        "expected Aead, got {err:?}"
    );
}

#[test]
fn envelope_uses_canonical_constants() {
    let profile = sample_profile();
    let envelope_json = encrypt_profile_with_config(&profile, TEST_PASSPHRASE, &FAST_KDF).unwrap();
    let v: serde_json::Value = serde_json::from_str(&envelope_json).unwrap();

    assert_eq!(v["version"], 1);
    assert_eq!(v["kdf"]["name"], "argon2id");
    assert_eq!(v["cipher"]["name"], "xchacha20poly1305");
    assert_eq!(v["payload"]["format"], "cbor");

    assert_eq!(v["kdf"]["memory_kib"], FAST_KDF.memory_kib);
    assert_eq!(v["kdf"]["iterations"], FAST_KDF.iterations);
    assert_eq!(v["kdf"]["parallelism"], FAST_KDF.parallelism);

    assert!(v["kdf"]["salt"].is_string());
    assert!(v["cipher"]["nonce"].is_string());
    assert!(v["payload"]["ciphertext"].is_string());
}

#[test]
fn unsupported_version_is_rejected() {
    let profile = sample_profile();
    let envelope_str =
        encrypt_profile_with_config(&profile, TEST_PASSPHRASE, &FAST_KDF).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&envelope_str).unwrap();
    v["version"] = serde_json::json!(99);
    let bad = serde_json::to_string(&v).unwrap();

    let err = decrypt_profile(&bad, TEST_PASSPHRASE).unwrap_err();
    assert!(
        matches!(
            err,
            ProfileCryptoError::UnsupportedVersion {
                got: 99,
                expected: 1,
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn unsupported_kdf_is_rejected() {
    let profile = sample_profile();
    let envelope_str =
        encrypt_profile_with_config(&profile, TEST_PASSPHRASE, &FAST_KDF).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&envelope_str).unwrap();
    v["kdf"]["name"] = serde_json::json!("bcrypt");
    let bad = serde_json::to_string(&v).unwrap();

    let err = decrypt_profile(&bad, TEST_PASSPHRASE).unwrap_err();
    assert!(
        matches!(err, ProfileCryptoError::UnsupportedKdf { ref name } if name == "bcrypt"),
        "got {err:?}"
    );
}

#[test]
fn unsupported_cipher_is_rejected() {
    let profile = sample_profile();
    let envelope_str =
        encrypt_profile_with_config(&profile, TEST_PASSPHRASE, &FAST_KDF).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&envelope_str).unwrap();
    v["cipher"]["name"] = serde_json::json!("aes-gcm");
    let bad = serde_json::to_string(&v).unwrap();

    let err = decrypt_profile(&bad, TEST_PASSPHRASE).unwrap_err();
    assert!(
        matches!(err, ProfileCryptoError::UnsupportedCipher { ref name } if name == "aes-gcm"),
        "got {err:?}"
    );
}

#[test]
fn unsupported_payload_format_is_rejected() {
    let profile = sample_profile();
    let envelope_str =
        encrypt_profile_with_config(&profile, TEST_PASSPHRASE, &FAST_KDF).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&envelope_str).unwrap();
    v["payload"]["format"] = serde_json::json!("json");
    let bad = serde_json::to_string(&v).unwrap();

    let err = decrypt_profile(&bad, TEST_PASSPHRASE).unwrap_err();
    assert!(
        matches!(err, ProfileCryptoError::UnsupportedPayloadFormat { ref name } if name == "json"),
        "got {err:?}"
    );
}

#[test]
fn corrupt_ciphertext_fails_with_aead_error() {
    let profile = sample_profile();
    let envelope_str =
        encrypt_profile_with_config(&profile, TEST_PASSPHRASE, &FAST_KDF).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&envelope_str).unwrap();

    let ct = v["payload"]["ciphertext"].as_str().unwrap();
    let mut bytes = B64.decode(ct).unwrap();
    *bytes.last_mut().unwrap() ^= 0x01;
    v["payload"]["ciphertext"] = serde_json::json!(B64.encode(&bytes));

    let bad = serde_json::to_string(&v).unwrap();
    let err = decrypt_profile(&bad, TEST_PASSPHRASE).unwrap_err();
    assert!(
        matches!(err, ProfileCryptoError::Aead),
        "expected Aead on corrupted ciphertext, got {err:?}"
    );
}

#[test]
fn malformed_envelope_is_rejected() {
    let err = decrypt_profile("not json", TEST_PASSPHRASE).unwrap_err();
    assert!(
        matches!(err, ProfileCryptoError::InvalidEnvelope),
        "got {err:?}"
    );
}

#[test]
fn error_display_does_not_leak_passphrase() {
    let profile = sample_profile();
    let envelope = encrypt_profile_with_config(&profile, TEST_PASSPHRASE, &FAST_KDF).unwrap();
    let leak_canary = "le4k-c4n4ry-pa55phra5e-DO-NOT-LEAK";
    let err = decrypt_profile(&envelope, leak_canary).unwrap_err();

    let display = format!("{err}");
    let debug = format!("{err:?}");

    assert!(
        !display.contains(leak_canary),
        "Display must not leak passphrase: {display}"
    );
    assert!(
        !debug.contains(leak_canary),
        "Debug must not leak passphrase: {debug}"
    );
}

#[test]
#[ignore = "full Argon2id default parameters take ~500ms-1s; run with --ignored"]
fn full_strength_round_trip() {
    let profile = sample_profile();
    let envelope = encrypt_profile(&profile, TEST_PASSPHRASE).unwrap();
    let restored = decrypt_profile(&envelope, TEST_PASSPHRASE).unwrap();
    assert!(profile == restored);
}

#[test]
fn save_profile_zz_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("profile.zz");
    let profile = sample_profile();
    save_profile_zz_with_config(&profile, TEST_PASSPHRASE, &path, &FAST_KDF).unwrap();
    let loaded = load_profile_zz(&path, TEST_PASSPHRASE).unwrap();
    assert!(profile == loaded);
}

#[test]
fn save_profile_zz_creates_parent_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("nested").join("dir").join("profile.zz");
    save_profile_zz_with_config(&sample_profile(), TEST_PASSPHRASE, &path, &FAST_KDF).unwrap();
    assert!(path.is_file());
}

#[cfg(unix)]
#[test]
fn save_profile_zz_sets_0600() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("profile.zz");
    save_profile_zz_with_config(&sample_profile(), TEST_PASSPHRASE, &path, &FAST_KDF).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn load_profile_zz_with_wrong_passphrase_fails_with_aead() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("profile.zz");
    save_profile_zz_with_config(&sample_profile(), TEST_PASSPHRASE, &path, &FAST_KDF).unwrap();
    let res = load_profile_zz(&path, "wrong-passphrase");
    assert!(matches!(res, Err(ProfileCryptoError::Aead)));
}

#[test]
fn load_profile_zz_missing_file_is_io_error() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("never-written.zz");
    let res = load_profile_zz(&path, TEST_PASSPHRASE);
    assert!(matches!(res, Err(ProfileCryptoError::Io)));
}
