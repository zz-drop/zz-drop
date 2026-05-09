//! End-to-end checks for the OneDrive setup path.
//!
//! Covers the same surface as the existing `add_inner_profile_flow`
//! integration but for the OneDrive provider:
//!
//! - the provider picker cycles through the three real providers
//!   (Nextcloud → Google Drive → OneDrive → Nextcloud) on Up/Down;
//! - selecting OneDrive routes to `Screen::SetupOneDrive` and arms
//!   the device-flow `init` request flag the main loop consumes;
//! - `apply_onedrive_*` stage transitions land the wizard in
//!   `OneDriveSetupStage::Done` with tokens populated;
//! - `save_profile_with_alias` produces a `ProviderProfile::OneDrive`
//!   readable back from a freshly-decrypted container.
//!
//! No real network is touched; tokens are stubbed in-memory the same
//! way the Google Drive integration tests stub them.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tempfile::tempdir;
use zz_drop_core::profile::format::load_set_zz;
use zz_drop_core::{Argon2idConfig, ProviderProfile};
use zz_drop_tui::app::App;
use zz_drop_tui::screens::Screen;
use zz_drop_tui::upload_test::{SaveProfileOutcome, save_profile_with_alias};
use zz_drop_tui::wizard::{
    GoogleDriveSetupState, OneDriveSetupStage, OneDriveSetupState, ProviderKind, WizardState,
};

const PASS: &str = "onedrive-integration-test-pass";
const FAST_KDF: Argon2idConfig = Argon2idConfig {
    memory_kib: 8 * 1024,
    iterations: 1,
    parallelism: 1,
};

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

#[test]
fn provider_picker_cycles_through_real_providers() {
    let mut app = App::new();
    app.screen = Screen::Provider;
    // Default after `App::new` is `Nextcloud`.
    assert_eq!(app.state.provider_kind, ProviderKind::Nextcloud);
    app.on_key(k(KeyCode::Down));
    assert_eq!(app.state.provider_kind, ProviderKind::GoogleDrive);
    app.on_key(k(KeyCode::Down));
    assert_eq!(app.state.provider_kind, ProviderKind::OneDrive);
    app.on_key(k(KeyCode::Down));
    assert_eq!(app.state.provider_kind, ProviderKind::Dropbox);
    app.on_key(k(KeyCode::Down));
    assert_eq!(app.state.provider_kind, ProviderKind::Nextcloud);
    // Up/k cycles in reverse.
    app.on_key(k(KeyCode::Up));
    assert_eq!(app.state.provider_kind, ProviderKind::Dropbox);
}

#[test]
fn enter_on_onedrive_routes_to_setup_screen_and_arms_init() {
    let mut app = App::new();
    app.screen = Screen::Provider;
    app.state.provider_kind = ProviderKind::OneDrive;
    app.on_key(k(KeyCode::Enter));
    assert_eq!(app.screen, Screen::SetupOneDrive);
    assert!(
        app.onedrive_request_init,
        "selecting OneDrive must arm the device-flow init flag"
    );
    // The state slot is reset so a previous attempt can't bleed
    // into the next try.
    assert_eq!(
        app.onedrive_setup.stage,
        OneDriveSetupStage::NotStarted
    );
}

#[test]
fn esc_on_setup_onedrive_returns_to_provider_and_clears_flags() {
    let mut app = App::new();
    app.screen = Screen::SetupOneDrive;
    app.onedrive_setup.stage = OneDriveSetupStage::Polling;
    app.onedrive_request_init = true;
    app.onedrive_request_email = true;
    app.on_key(k(KeyCode::Esc));
    assert_eq!(app.screen, Screen::Provider);
    assert!(!app.onedrive_request_init);
    assert!(!app.onedrive_request_email);
    assert_eq!(
        app.onedrive_setup.stage,
        OneDriveSetupStage::NotStarted,
        "Esc must reset the setup state to default"
    );
}

#[test]
fn apply_onedrive_lifecycle_transitions_through_stages() {
    let mut app = App::new();
    // init: NotStarted → Polling, fields populated.
    app.apply_onedrive_init(
        "ABCD-EFGH".into(),
        "https://login.microsoft.com/device".into(),
        Some("https://login.microsoft.com/device?user_code=ABCD-EFGH".into()),
        "device-code-canary".into(),
        900,
        5,
    );
    assert_eq!(app.onedrive_setup.stage, OneDriveSetupStage::Polling);
    assert_eq!(app.onedrive_setup.user_code, "ABCD-EFGH");
    assert!(app.onedrive_setup.expires_at > 0);
    assert!(!app.onedrive_setup.device_code.is_empty());

    // tokens: Polling → Fetching, request_email armed.
    app.apply_onedrive_tokens(
        "access-token-canary".into(),
        Some("refresh-token-canary".into()),
        "Bearer".into(),
        3600,
        Some("Files.ReadWrite offline_access User.Read".into()),
    );
    assert_eq!(app.onedrive_setup.stage, OneDriveSetupStage::Fetching);
    assert!(app.onedrive_request_email);
    assert_eq!(app.onedrive_setup.access_token, "access-token-canary");
    assert_eq!(app.onedrive_setup.refresh_token, "refresh-token-canary");

    // email: Fetching → Done.
    app.apply_onedrive_email("alice@example.org".into());
    assert_eq!(app.onedrive_setup.stage, OneDriveSetupStage::Done);
    assert_eq!(app.onedrive_setup.user_email, "alice@example.org");
}

#[test]
fn enter_on_done_advances_to_collision() {
    let mut app = App::new();
    app.screen = Screen::SetupOneDrive;
    app.onedrive_setup.stage = OneDriveSetupStage::Done;
    app.on_key(k(KeyCode::Enter));
    assert_eq!(app.screen, Screen::Collision);
}

#[test]
fn save_profile_with_alias_writes_a_onedrive_provider_profile() {
    // Drive `save_profile_with_alias` directly with a fully-stubbed
    // OneDrive setup state and verify the encrypted blob round-trips
    // back into a `ProviderProfile::OneDrive` with the expected
    // payload. Mirrors what happens in `main.rs::run_save_profile`
    // after the device flow lands on Done.
    let dir = tempdir().unwrap();
    // The save path resolves under the user's config dir; for a
    // hermetic test we override it with tempdir-rooted env.
    unsafe {
        // SAFETY: single-threaded test, no other code touches these
        // env vars. Restored by `tempdir` going out of scope.
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
    }

    let mut state = WizardState::default();
    state.provider_kind = ProviderKind::OneDrive;

    let gdrive_setup = GoogleDriveSetupState::default();
    let mut onedrive_setup = OneDriveSetupState::default();
    onedrive_setup.access_token = "access-token-canary".into();
    onedrive_setup.refresh_token = "refresh-token-canary".into();
    onedrive_setup.token_type = "Bearer".into();
    onedrive_setup.access_expires_at = 1_700_000_000;
    onedrive_setup.scope = "Files.ReadWrite offline_access User.Read".into();
    onedrive_setup.user_email = "alice@example.org".into();
    onedrive_setup.root_folder = "zz-drop".into();

    let dropbox_setup = zz_drop_tui::wizard::DropboxSetupState::default();
    let outcome = save_profile_with_alias(
        &state,
        &gdrive_setup,
        &onedrive_setup,
        &dropbox_setup,
        PASS,
        "onedrive-canary",
    );
    let path = match outcome {
        SaveProfileOutcome::Ok { path } => path,
        SaveProfileOutcome::Failed(reason) => panic!("expected save success: {reason}"),
    };
    let path = std::path::PathBuf::from(path);
    assert!(path.exists(), "container blob not written at {path:?}");

    let (set, _kek) = load_set_zz(&path, PASS).unwrap();
    assert_eq!(set.profiles.len(), 1);
    let inner = &set.profiles[0];
    assert_eq!(inner.alias, "onedrive-canary");
    assert_eq!(inner.default_target, "onedrive");
    let od = match inner.providers.first() {
        Some(ProviderProfile::OneDrive(o)) => o,
        other => panic!("expected ProviderProfile::OneDrive, got {other:?}"),
    };
    assert_eq!(od.user_email, "alice@example.org");
    assert_eq!(od.root_folder, "zz-drop");
    assert_eq!(od.auth.access_token, "access-token-canary");
    assert_eq!(od.auth.refresh_token, "refresh-token-canary");
    assert_eq!(od.auth.scope, "Files.ReadWrite offline_access User.Read");

    let _ = FAST_KDF; // silence unused warning when we drop this in future cleanup
}

/// Locks the alias-generator surface for OneDrive — the picker UI
/// shows a generated suggestion with an `onedrive-` prefix when the
/// operator opens the inner-alias prompt. Regression guard: a
/// future refactor of `prepare_inner_alias_input` must keep this
/// arm in place.
#[test]
fn alias_suggestion_for_onedrive_uses_onedrive_prefix() {
    let suggestion = zz_drop_tui::alias_gen::suggest_alias_for(
        zz_drop_tui::alias_gen::ProviderPrefix::OneDrive,
    );
    assert!(
        suggestion.starts_with("onedrive-"),
        "expected onedrive- prefix, got {suggestion:?}"
    );
}

/// Regression guard for the alias-prompt routing fix: in a *first*
/// container setup with an OAuth provider (Google Drive / OneDrive),
/// pressing Enter on TestUpload must route to InnerAlias — not
/// straight to ProfilePassphrase — so the operator can pick a
/// readable alias instead of inheriting the local-part of the
/// authenticated email address.
#[test]
fn first_setup_oauth_provider_routes_through_inner_alias() {
    use zz_drop_tui::wizard::{TestOutcome, WizardMode};

    let mut app = App::new();
    app.wizard_mode = WizardMode::CreateLocal;
    app.state.provider_kind = ProviderKind::OneDrive;
    app.state.last_test_outcome = Some(TestOutcome::Ok);
    app.screen = Screen::TestUpload;
    app.on_key(k(KeyCode::Enter));
    assert_eq!(
        app.screen,
        Screen::InnerAlias,
        "OneDrive first setup must show the alias prompt before passphrase"
    );

    // Same routing for Google Drive — coherent with the OneDrive
    // arm and locks the OAuth pair.
    let mut app = App::new();
    app.wizard_mode = WizardMode::CreateLocal;
    app.state.provider_kind = ProviderKind::GoogleDrive;
    app.state.last_test_outcome = Some(TestOutcome::Ok);
    app.screen = Screen::TestUpload;
    app.on_key(k(KeyCode::Enter));
    assert_eq!(app.screen, Screen::InnerAlias);

    // Nextcloud keeps the original direct path: the username
    // typed during auth already serves as the alias, no extra
    // prompt needed.
    let mut app = App::new();
    app.wizard_mode = WizardMode::CreateLocal;
    app.state.provider_kind = ProviderKind::Nextcloud;
    app.state.last_test_outcome = Some(TestOutcome::Ok);
    app.screen = Screen::TestUpload;
    app.on_key(k(KeyCode::Enter));
    assert_eq!(
        app.screen,
        Screen::ProfilePassphrase,
        "Nextcloud first setup must not insert an extra alias prompt"
    );
}

/// Reproduce the user-reported "wrong passphrase" against a
/// single-character passphrase ("!"). If save+load round-trips
/// here but fails in the live TUI, the bug is somewhere between
/// the input event and the call to `run_save_profile`. If it
/// fails here too, the bug is in the core profile encrypt/decrypt
/// path.
/// Reproduce the EXACT TUI save path with passphrase `!`,
/// mirroring main.rs::save_request handling (1) build the alias
/// via `run_save_profile`, (2) which calls
/// `save_profile_with_alias`, (3) which calls `save_set_zz`. Then
/// load it back with the same passphrase. If the bug is between
/// the TUI input and the disk write, this test fails.
#[test]
fn full_tui_save_path_with_single_char_passphrase() {
    use zz_drop_core::profile::format::load_set_zz;
    use zz_drop_tui::upload_test::run_save_profile;
    use zz_drop_tui::wizard::WizardMode;

    let dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
    }

    let mut state = WizardState::default();
    state.provider_kind = ProviderKind::Nextcloud;
    state.username = "gibbio".into();
    state.server_url = "https://nextcloud.armtc.net".into();
    state.auth_secret = "app-pw".into();
    state.remote_folder = "/zz-drop".into();
    let _ = WizardMode::CreateLocal;

    let gdrive = GoogleDriveSetupState::default();
    let onedrive = OneDriveSetupState::default();

    // EXACT pass the TUI hands to `run_save_profile` after Enter
    // on the passphrase screen.
    let pass = "!";
    let dropbox = zz_drop_tui::wizard::DropboxSetupState::default();
    let outcome = run_save_profile(&state, &gdrive, &onedrive, &dropbox, pass);
    let path = match outcome {
        SaveProfileOutcome::Ok { path } => path,
        SaveProfileOutcome::Failed(reason) => panic!("save failed: {reason}"),
    };
    let path = std::path::PathBuf::from(path);

    let (set, _kek) = load_set_zz(&path, pass)
        .expect("decrypt with the same single-char passphrase");
    assert_eq!(set.profiles.len(), 1);
    assert_eq!(set.profiles[0].alias, "gibbio");
}

#[test]
fn round_trip_with_single_char_passphrase() {
    use zz_drop_core::profile::format::{decrypt_set, encrypt_set};

    let mut set = zz_drop_core::ProfileSet::new();
    set.profiles
        .push(zz_drop_core::PlainProfile {
            profile_version: 1,
            profile_id: "p-1".into(),
            alias: "test".into(),
            default_target: "nextcloud".into(),
            providers: vec![],
            collision_policy: zz_drop_core::CollisionPolicy::Rename,
            settings: zz_drop_core::ProfileSettings::default(),
            created_at: "epoch:0".into(),
            updated_at: "epoch:0".into(),
        });
    let (envelope, _kek) = encrypt_set(&set, "!").expect("encrypt");
    let (decoded, _kek2) = decrypt_set(&envelope, "!").expect("decrypt with same single-char pass");
    assert_eq!(decoded.profiles.len(), 1);
    assert_eq!(decoded.profiles[0].alias, "test");
}

/// Full save → load round-trip. Mirrors what the main loop does
/// when the wizard arms `save_request`: build the alias, encrypt
/// the container with the passphrase, persist to disk. Then re-open
/// it with the same passphrase and verify the inner profile and
/// alias survived. A regression here would surface to the operator
/// as "wrong passphrase or corrupted profile blob" right after
/// setup — exactly the failure mode worth locking.
#[test]
fn round_trip_save_then_load_with_alias_override() {
    use zz_drop_tui::upload_test::run_save_profile;
    use zz_drop_tui::wizard::WizardMode;

    let dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
    }

    let mut state = WizardState::default();
    state.provider_kind = ProviderKind::OneDrive;
    state.alias_override = Some("personal".into());

    let gdrive_setup = GoogleDriveSetupState::default();
    let mut onedrive_setup = OneDriveSetupState::default();
    onedrive_setup.access_token = "access-token".into();
    onedrive_setup.refresh_token = "refresh-token".into();
    onedrive_setup.token_type = "Bearer".into();
    onedrive_setup.access_expires_at = 1_700_000_000;
    onedrive_setup.scope = "Files.ReadWrite offline_access User.Read".into();
    onedrive_setup.user_email = "alice@example.org".into();
    onedrive_setup.root_folder = "zz-drop".into();
    let _ = (FAST_KDF, WizardMode::CreateLocal);

    let dropbox_setup = zz_drop_tui::wizard::DropboxSetupState::default();
    let outcome = run_save_profile(
        &state,
        &gdrive_setup,
        &onedrive_setup,
        &dropbox_setup,
        "round-trip-pass",
    );
    let path = match outcome {
        SaveProfileOutcome::Ok { path } => path,
        SaveProfileOutcome::Failed(reason) => panic!("save failed: {reason}"),
    };

    let (set, _kek) =
        load_set_zz(&std::path::PathBuf::from(&path), "round-trip-pass").unwrap();
    assert_eq!(set.profiles.len(), 1);
    assert_eq!(
        set.profiles[0].alias, "personal",
        "alias_override must be the on-disk alias, not the email local-part"
    );
    let inner = &set.profiles[0];
    assert!(matches!(
        inner.providers.first(),
        Some(ProviderProfile::OneDrive(_))
    ));

    // And confirm the failure mode the operator hits: a wrong
    // passphrase yields a decrypt error, not a successful load
    // with garbled bytes.
    let wrong = load_set_zz(&std::path::PathBuf::from(&path), "wrong-pass");
    assert!(wrong.is_err(), "wrong passphrase must fail");
}

#[test]
fn inner_alias_in_create_mode_stashes_alias_and_advances_to_passphrase() {
    use zz_drop_tui::wizard::WizardMode;

    let mut app = App::new();
    app.wizard_mode = WizardMode::CreateLocal;
    app.state.provider_kind = ProviderKind::OneDrive;
    app.screen = Screen::InnerAlias;
    app.inner_alias_input.set_value("personal");
    app.on_key(k(KeyCode::Enter));
    assert_eq!(app.screen, Screen::ProfilePassphrase);
    assert_eq!(app.state.alias_override.as_deref(), Some("personal"));
    assert!(
        !app.add_inner_request,
        "first-setup InnerAlias must NOT arm add_inner_request"
    );
}
