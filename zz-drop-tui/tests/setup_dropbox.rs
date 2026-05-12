//! End-to-end checks for the Dropbox setup path.
//!
//! Mirrors the OneDrive integration tests, with two structural
//! differences forced by Dropbox's paste-code flow:
//!
//! - the picker now cycles through *four* real providers (Nextcloud
//!   → Google Drive → OneDrive → Dropbox → Nextcloud), and selecting
//!   Dropbox lands on `Screen::SetupDropbox` with
//!   `dropbox_request_init` armed (no device-code endpoint, the
//!   init step is local URL building);
//! - the success path goes
//!   `NotStarted` → `AwaitingPaste` → `Exchanging` → `Fetching` →
//!   `Done` instead of the device-flow's `Polling` cycle. The tests
//!   stub `apply_dropbox_*` with the same shapes the main loop
//!   would feed in after a real exchange.
//!
//! No real network is touched.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tempfile::tempdir;
use zz_drop_core::profile::format::load_set_zz;
use zz_drop_core::ProviderProfile;
use zz_drop_tui::app::App;
use zz_drop_tui::screens::Screen;
use zz_drop_tui::upload_test::{SaveProfileOutcome, save_profile_with_alias_at};
use zz_drop_tui::wizard::{
    DropboxSetupStage, DropboxSetupState, GoogleDriveSetupState, OneDriveSetupState,
    ProviderKind, WizardState,
};

const PASS: &str = "dropbox-integration-test-pass";

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

#[test]
fn enter_on_dropbox_routes_to_setup_screen_and_arms_init() {
    let mut app = App::new();
    app.screen = Screen::Provider;
    app.state.provider_kind = ProviderKind::Dropbox;
    app.on_key(k(KeyCode::Enter));
    assert_eq!(app.screen, Screen::SetupDropbox);
    assert!(
        app.dropbox_request_init,
        "selecting Dropbox must arm the paste-code init flag"
    );
    // The state slot is reset so a previous attempt can't bleed
    // into the next try.
    assert_eq!(app.dropbox_setup.stage, DropboxSetupStage::NotStarted);
    assert!(app.dropbox_setup.authorize_url.is_empty());
    assert!(app.dropbox_setup.code_verifier.is_empty());
}

#[test]
fn esc_on_setup_dropbox_clears_flags_and_returns_to_picker() {
    let mut app = App::new();
    app.screen = Screen::SetupDropbox;
    app.dropbox_setup.stage = DropboxSetupStage::AwaitingPaste;
    app.dropbox_setup.authorize_url =
        "https://www.dropbox.com/oauth2/authorize?…".into();
    app.dropbox_setup.code_verifier = "verifier-canary".into();
    app.dropbox_setup.pasted_code.set_value("PARTIAL");
    app.dropbox_request_init = true;
    app.dropbox_request_exchange = true;
    app.dropbox_request_email = true;

    app.on_key(k(KeyCode::Esc));
    assert_eq!(app.screen, Screen::Provider);
    assert_eq!(app.dropbox_setup.stage, DropboxSetupStage::NotStarted);
    assert!(!app.dropbox_request_init);
    assert!(!app.dropbox_request_exchange);
    assert!(!app.dropbox_request_email);
    // Secrets cleared.
    assert!(app.dropbox_setup.authorize_url.is_empty());
    assert!(app.dropbox_setup.code_verifier.is_empty());
    assert!(app.dropbox_setup.pasted_code.is_empty());
}

#[test]
fn apply_dropbox_init_lands_in_awaiting_paste() {
    let mut app = App::new();
    app.screen = Screen::SetupDropbox;
    app.apply_dropbox_init(
        "https://www.dropbox.com/oauth2/authorize?abc".into(),
        "VERIFIER-CANARY".into(),
    );
    assert!(matches!(
        app.dropbox_setup.stage,
        DropboxSetupStage::AwaitingPaste
    ));
    assert!(
        app.dropbox_setup
            .authorize_url
            .starts_with("https://www.dropbox.com/oauth2/authorize")
    );
    assert_eq!(app.dropbox_setup.code_verifier, "VERIFIER-CANARY");
}

#[test]
fn typing_pasted_code_and_pressing_enter_arms_exchange() {
    let mut app = App::new();
    app.screen = Screen::SetupDropbox;
    app.dropbox_setup.stage = DropboxSetupStage::AwaitingPaste;
    app.dropbox_setup.authorize_url = "https://www.dropbox.com/oauth2/authorize?".into();
    app.dropbox_setup.code_verifier = "v".repeat(43);
    // Type a plausible-looking code.
    for c in "ABCDEFGHIJ12345".chars() {
        app.on_key(k(KeyCode::Char(c)));
    }
    assert_eq!(app.dropbox_setup.pasted_code.value(), "ABCDEFGHIJ12345");
    assert!(app.dropbox_setup.pasted_code_appears_valid());
    // Enter triggers exchange (transitions stage + arms flag).
    app.on_key(k(KeyCode::Enter));
    assert_eq!(app.dropbox_setup.stage, DropboxSetupStage::Exchanging);
    assert!(app.dropbox_request_exchange);
}

#[test]
fn enter_with_empty_code_does_not_arm_exchange() {
    let mut app = App::new();
    app.screen = Screen::SetupDropbox;
    app.dropbox_setup.stage = DropboxSetupStage::AwaitingPaste;
    // No pasted_code → enter is a no-op.
    app.on_key(k(KeyCode::Enter));
    assert_eq!(app.dropbox_setup.stage, DropboxSetupStage::AwaitingPaste);
    assert!(!app.dropbox_request_exchange);
}

#[test]
fn apply_dropbox_tokens_clears_pasted_code_and_arms_email() {
    let mut app = App::new();
    app.screen = Screen::SetupDropbox;
    app.dropbox_setup.stage = DropboxSetupStage::Exchanging;
    app.dropbox_setup.code_verifier = "v".repeat(43);
    app.dropbox_setup.pasted_code.set_value("PASTED-CANARY");

    app.apply_dropbox_tokens(
        "AT-CANARY".into(),
        Some("RT-CANARY".into()),
        "bearer".into(),
        14_400,
        Some("files.content.read".into()),
    );
    assert_eq!(app.dropbox_setup.stage, DropboxSetupStage::Fetching);
    assert!(app.dropbox_request_email);
    assert!(app.dropbox_setup.pasted_code.is_empty());
    assert!(app.dropbox_setup.code_verifier.is_empty());
    assert_eq!(app.dropbox_setup.access_token, "AT-CANARY");
    assert_eq!(app.dropbox_setup.refresh_token, "RT-CANARY");
}

#[test]
fn save_profile_with_alias_writes_a_dropbox_provider_profile() {
    // Drive `save_profile_with_alias` directly with a fully-stubbed
    // dropbox setup state, then load the resulting blob from disk
    // and assert the round-tripped profile matches the input
    // payload.
    let tmp = tempdir().unwrap();
    // Hermetic save: explicit path inside `tempdir`. Setting
    // `XDG_CONFIG_HOME` is unsafe on macOS — the `directories`
    // crate ignores it and falls back to
    // `~/Library/Application Support/`, which would clobber the
    // operator's real container during `cargo test`.
    let path = tmp.path().join("profiles-local.zz");

    let mut state = WizardState::default();
    state.provider_kind = ProviderKind::Dropbox;

    let gdrive_setup = GoogleDriveSetupState::default();
    let onedrive_setup = OneDriveSetupState::default();
    let mut dropbox_setup = DropboxSetupState::default();
    dropbox_setup.stage = DropboxSetupStage::Done;
    dropbox_setup.access_token = "access-token-canary".into();
    dropbox_setup.refresh_token = "refresh-token-canary".into();
    dropbox_setup.token_type = "bearer".into();
    dropbox_setup.access_expires_at = 1_700_000_000;
    dropbox_setup.scope = "files.content.write files.content.read \
                          files.metadata.read account_info.read"
        .into();
    dropbox_setup.user_email = "alice@example.org".into();
    // Dropbox's App-folder access type already sandboxes us under
    // `Apps/zz-drop/`; an empty `root_folder` means "write directly
    // there" instead of nesting another `zz-drop/` subfolder. The
    // wizard's default for new profiles is empty — assert that the
    // empty value round-trips through encrypt → decrypt unchanged.
    dropbox_setup.root_folder = String::new();

    let outcome = save_profile_with_alias_at(
        &state,
        &gdrive_setup,
        &onedrive_setup,
        &dropbox_setup,
        PASS,
        "dropbox-canary",
        &path,
    );
    let path_str = match outcome {
        SaveProfileOutcome::Ok { path } => path,
        SaveProfileOutcome::Failed(reason) => panic!("expected save success: {reason}"),
    };
    let path = std::path::PathBuf::from(path_str);
    assert!(path.exists(), "container blob not written at {path:?}");

    let (set, _kek) = load_set_zz(&path, PASS).unwrap();
    assert_eq!(set.profiles.len(), 1);
    let inner = &set.profiles[0];
    assert_eq!(inner.alias, "dropbox-canary");
    assert_eq!(inner.default_target, "dropbox");
    let db = match inner.providers.first() {
        Some(ProviderProfile::Dropbox(d)) => d,
        other => panic!("expected ProviderProfile::Dropbox, got {other:?}"),
    };
    assert_eq!(db.root_folder, "");
    assert_eq!(db.user_email, "alice@example.org");
    assert_eq!(db.auth.access_token, "access-token-canary");
    assert_eq!(db.auth.refresh_token, "refresh-token-canary");
    assert_eq!(db.auth.token_type, "bearer");
    assert_eq!(db.auth.expires_at, 1_700_000_000);
    assert!(db.auth.scope.contains("files.content.write"));
}

#[test]
fn alias_generator_uses_dropbox_prefix() {
    let alias =
        zz_drop_tui::alias_gen::suggest_alias_for(zz_drop_tui::alias_gen::ProviderPrefix::Dropbox);
    assert!(
        alias.starts_with("dropbox-"),
        "Dropbox prefix missing: `{alias}`"
    );
    // Total alias must stay within the 32-char API cap.
    assert!(alias.len() <= 32, "alias too long: `{alias}` ({})", alias.len());
}
