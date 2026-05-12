//! End-to-end check that the "Add profile to local container" flow
//! preserves the existing inner profiles on disk. The TUI routes
//! through `Screen::ProfileUnlock` → `apply_unlock_set_done`
//! (AddInnerProfile branch) → wizard → `Screen::InnerAlias`. Pressing
//! Enter on InnerAlias arms `add_inner_request`, which `main.rs`
//! consumes to clone the unlocked set, push the new profile and
//! re-encrypt with the cached KEK. This test inlines that final
//! container-rewrite step (since `main.rs`'s function is private)
//! and asserts the file ends with both profiles, not just the new one.
//!
//! The container is written / read in a `tempfile::tempdir`, so the
//! test never touches the operator's real `~/.config/zz-drop`.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use zz_drop_core::profile::format::{encrypt_set_with_kek, load_set_zz, save_set_zz_with_config};
use zz_drop_core::{
    Argon2idConfig, CollisionPolicy, NextcloudAuth, NextcloudProfile, PlainProfile, ProfileSet,
    ProfileSettings, ProviderProfile,
};
use zz_drop_tui::app::{App, ProfileSource};
use zz_drop_tui::screens::Screen;
use zz_drop_tui::wizard::{TestOutcome, WelcomeItem, WizardMode};

const PASS: &str = "correct horse battery staple";
const FAST_KDF: Argon2idConfig = Argon2idConfig {
    memory_kib: 8 * 1024,
    iterations: 1,
    parallelism: 1,
};

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn nc_profile(alias: &str, username: &str) -> PlainProfile {
    PlainProfile {
        profile_version: 1,
        profile_id: format!("p-{alias}"),
        alias: alias.into(),
        default_target: "nextcloud".into(),
        providers: vec![ProviderProfile::Nextcloud(NextcloudProfile {
            server_url: "https://example.org".into(),
            username: username.into(),
            auth: NextcloudAuth::AppPassword {
                secret: "x".into(),
            },
            remote_root: "/zz-drop".into(),
        })],
        collision_policy: CollisionPolicy::Rename,
        settings: ProfileSettings::default(),
        created_at: "epoch:0".into(),
        updated_at: "epoch:0".into(),
    }
}

#[test]
fn add_inner_profile_flow_appends_instead_of_overwriting() {
    let tmp = tempfile::tempdir().unwrap();
    let container_path = tmp.path().join("profiles-local.zz");

    // 1. Seed the container with one existing profile.
    let mut initial = ProfileSet::new();
    initial.profiles.push(nc_profile("first", "alice"));
    save_set_zz_with_config(&initial, PASS, &container_path, &FAST_KDF).unwrap();

    // 2. Mimic the TUI unlock: read file → decrypt_set → hand the set
    //    + KEK to the App via `apply_unlock_set_done`. On the
    //    AddInnerProfile path, the App stores them and routes to
    //    Provider with the wizard reset.
    let (unlocked_set, kek) = load_set_zz(&container_path, PASS).unwrap();
    assert_eq!(unlocked_set.profiles.len(), 1);

    let mut app = App::new();
    app.local_exists = true;
    app.welcome_item = WelcomeItem::Configure;
    app.unlock_source = ProfileSource::Local;
    app.on_key(k(KeyCode::Enter));
    assert_eq!(app.wizard_mode, WizardMode::AddInnerProfile);
    assert_eq!(app.screen, Screen::ProfileUnlock);

    app.apply_unlock_set_done(unlocked_set, kek, None);
    assert_eq!(app.screen, Screen::Provider);
    assert_eq!(
        app.unlocked_set.as_ref().map(|s| s.profiles.len()),
        Some(1)
    );

    // 3. Walk the wizard with stub state, mark the probe Ok and press
    //    Enter on TestUpload. The AddInnerProfile branch must route
    //    to InnerAlias (the alias prompt), *not* ProfilePassphrase
    //    (which would arm `save_request` → overwrite the container
    //    via `run_save_profile`).
    app.state.server_url = "https://example.org".into();
    app.state.username = "bob".into();
    app.state.auth_secret = "y".into();
    app.state.remote_folder = "/zz-drop".into();
    app.screen = Screen::TestUpload;
    app.state.last_test_outcome = Some(TestOutcome::Ok);
    app.on_key(k(KeyCode::Enter));
    assert_eq!(app.screen, Screen::InnerAlias);
    assert!(!app.save_request, "AddInnerProfile must not arm save_request");

    // 4. Type the new alias and submit. `add_inner_request` is armed
    //    and `unlocked_set` still has the original profile — main.rs
    //    appends from there before re-encrypting.
    app.inner_alias_input.set_value("second");
    app.on_key(k(KeyCode::Enter));
    assert!(app.add_inner_request);
    assert_eq!(
        app.unlocked_set.as_ref().map(|s| s.profiles.len()),
        Some(1),
        "the set must still hold the existing profile when main.rs picks it up"
    );

    // 5. Inline replica of `main.rs::perform_add_inner_profile`: clone
    //    the in-RAM set, append the new profile, re-encrypt with the
    //    cached KEK and write atomically. If perform_add_inner_profile
    //    accidentally rebuilt the set from scratch, this would
    //    drop the original.
    let cached_kek = app.unlocked_kek.as_ref().unwrap().clone();
    let mut new_set = app.unlocked_set.clone().unwrap();
    new_set.profiles.push(nc_profile("second", "bob"));
    let envelope = encrypt_set_with_kek(&new_set, &cached_kek).unwrap();
    let tmp_path = container_path.with_extension("zz.tmp");
    std::fs::write(&tmp_path, envelope).unwrap();
    std::fs::rename(&tmp_path, &container_path).unwrap();

    // 6. Reload from disk and assert both profiles survived.
    let (final_set, _kek2) = load_set_zz(&container_path, PASS).unwrap();
    assert_eq!(
        final_set.profiles.len(),
        2,
        "expected 2 profiles after add-inner-profile flow, got {} — the previous profile was dropped",
        final_set.profiles.len()
    );
    let aliases: Vec<&str> = final_set.profiles.iter().map(|p| p.alias.as_str()).collect();
    assert_eq!(aliases, vec!["first", "second"]);
}
