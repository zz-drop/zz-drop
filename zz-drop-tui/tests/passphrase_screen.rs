use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use zz_drop_tui::app::App;
use zz_drop_tui::screens::Screen;
use zz_drop_tui::wizard::{PassphraseFocus, PassphraseStage};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn type_into_passphrase(app: &mut App, s: &str) {
    for c in s.chars() {
        app.on_key(key(KeyCode::Char(c)));
    }
}

fn populate_state_for_save(app: &mut App) {
    app.state.server_url = "https://nc.example.org".into();
    app.state.username = "alice".into();
    app.state.auth_secret = "app-pw".into();
    app.state.remote_folder = "/zz-drop".into();
    app.state.collision = zz_drop_core::CollisionPolicy::Rename;
}

#[test]
fn screen_starts_on_passphrase_focus() {
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    assert_eq!(app.passphrase_focus, PassphraseFocus::Passphrase);
}

#[test]
fn typing_chars_accumulates_in_passphrase_input() {
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    type_into_passphrase(&mut app, "hello");
    assert_eq!(app.passphrase_input.value(), "hello");
    assert_eq!(app.confirm_input.value(), "");
}

#[test]
fn tab_switches_focus_to_confirm_field() {
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    app.on_key(key(KeyCode::Tab));
    assert_eq!(app.passphrase_focus, PassphraseFocus::Confirm);
    type_into_passphrase(&mut app, "world");
    assert_eq!(app.confirm_input.value(), "world");
}

#[test]
fn enter_with_empty_passphrase_does_nothing() {
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    app.on_key(key(KeyCode::Enter));
    assert!(matches!(app.passphrase_stage, PassphraseStage::Editing));
    assert!(!app.save_request);
}

#[test]
fn enter_with_mismatch_does_not_advance() {
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    app.passphrase_input.set_value("foo");
    app.confirm_input.set_value("bar");
    app.on_key(key(KeyCode::Enter));
    assert!(matches!(app.passphrase_stage, PassphraseStage::Editing));
    assert!(!app.save_request);
}

#[test]
fn enter_with_weak_pass_triggers_warning() {
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    populate_state_for_save(&mut app);
    app.passphrase_input.set_value("password");
    app.confirm_input.set_value("password");
    app.on_key(key(KeyCode::Enter));
    assert!(matches!(app.passphrase_stage, PassphraseStage::WeakWarning));
    assert!(!app.save_request);
}

#[test]
fn weak_warning_y_triggers_save_request() {
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    populate_state_for_save(&mut app);
    app.passphrase_input.set_value("password");
    app.confirm_input.set_value("password");
    app.on_key(key(KeyCode::Enter));
    assert!(matches!(app.passphrase_stage, PassphraseStage::WeakWarning));
    app.on_key(key(KeyCode::Char('y')));
    assert!(matches!(app.passphrase_stage, PassphraseStage::Encrypting));
    assert!(app.save_request);
}

#[test]
fn weak_warning_n_returns_to_editing() {
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    populate_state_for_save(&mut app);
    app.passphrase_input.set_value("password");
    app.confirm_input.set_value("password");
    app.on_key(key(KeyCode::Enter));
    app.on_key(key(KeyCode::Char('n')));
    assert!(matches!(app.passphrase_stage, PassphraseStage::Editing));
    assert!(!app.save_request);
}

#[test]
fn enter_with_strong_long_pass_skips_warning_and_requests_save() {
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    populate_state_for_save(&mut app);
    let strong = "correct horse battery staple jaguar 17";
    app.passphrase_input.set_value(strong);
    app.confirm_input.set_value(strong);
    app.on_key(key(KeyCode::Enter));
    assert!(matches!(app.passphrase_stage, PassphraseStage::Encrypting));
    assert!(app.save_request);
}

#[test]
fn esc_during_editing_returns_to_test_upload() {
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    app.on_key(key(KeyCode::Esc));
    assert_eq!(app.screen, Screen::TestUpload);
}

#[test]
fn apply_save_done_sets_saved_state_and_path() {
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    app.apply_save_done("/tmp/profile.zz".into());
    assert!(matches!(app.passphrase_stage, PassphraseStage::Saved(_)));
    assert_eq!(app.saved_path.as_deref(), Some("/tmp/profile.zz"));
}

#[test]
fn enter_after_saved_skips_push_and_lands_on_done() {
    // Push is optional. Enter on the Saved stage skips the push and
    // lands on Done — the Done screen renders the local-only warning.
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    app.apply_save_done("/tmp/profile-local.zz".into());
    app.on_key(key(KeyCode::Enter));
    assert_eq!(app.screen, Screen::Done);
    assert!(app.pushed_summary.is_none());
}

#[test]
fn p_after_saved_enters_push_flow() {
    let mut app = App::new();
    app.screen = Screen::ProfilePassphrase;
    app.apply_save_done("/tmp/profile-local.zz".into());
    app.on_key(key(KeyCode::Char('p')));
    assert_eq!(app.screen, Screen::Account);
}

#[test]
fn done_screen_q_quits() {
    let mut app = App::new();
    app.screen = Screen::Done;
    app.on_key(key(KeyCode::Char('q')));
    assert!(app.should_quit);
}

#[test]
fn done_screen_enter_returns_to_welcome_without_quitting() {
    let mut app = App::new();
    app.screen = Screen::Done;
    app.local_exists = true;
    app.remote_exists = false;
    app.saved_path = Some("/tmp/profile-local.zz".into());
    app.on_key(key(KeyCode::Enter));
    assert!(!app.should_quit);
    assert_eq!(app.screen, Screen::Welcome);
    // Cursor lands on the just-saved profile slot.
    assert!(matches!(
        app.welcome_item,
        zz_drop_tui::wizard::WelcomeItem::OpenLocal
    ));
}

#[test]
fn debug_does_not_leak_passphrase_input() {
    let mut app = App::new();
    app.passphrase_input.set_value("topsecret-canary");
    let dbg = format!("{:?}", app.state);
    assert!(!dbg.contains("topsecret-canary"));
}
