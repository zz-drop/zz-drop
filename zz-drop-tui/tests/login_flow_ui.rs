use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use zz_drop_tui::app::App;
use zz_drop_tui::screens::Screen;
use zz_drop_tui::wizard::{AuthKind, LoginFlowStage};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn populate_for_login_flow(app: &mut App) {
    // server URL must be valid for the auth screen to dispatch into the flow
    app.state.server_url = "https://nc.example.org".into();
    app.server_input
        .set_value(&app.state.server_url.clone());
    app.state.auth_kind = AuthKind::LoginFlow;
}

#[test]
fn auth_login_flow_enter_jumps_to_login_flow_screen_and_requests_init() {
    let mut app = App::new();
    populate_for_login_flow(&mut app);
    app.screen = Screen::NextcloudAuth;
    app.on_key(key(KeyCode::Enter));
    assert_eq!(app.screen, Screen::NextcloudLoginFlow);
    assert!(
        matches!(app.login_flow.stage, LoginFlowStage::Initiating),
        "expected stage Initiating, got {:?}",
        app.login_flow.stage
    );
    assert!(app.login_flow_request_init);
}

#[test]
fn login_flow_apply_init_moves_to_polling_with_url_token_endpoint() {
    let mut app = App::new();
    populate_for_login_flow(&mut app);
    app.screen = Screen::NextcloudLoginFlow;
    app.apply_login_flow_init(
        "https://nc.example.org/index.php/login/v2/flow/abc".into(),
        "tok123".into(),
        "https://nc.example.org/index.php/login/v2/poll".into(),
    );
    assert_eq!(app.login_flow.stage, LoginFlowStage::Polling);
    assert!(app.login_flow.login_url.contains("/flow/"));
    assert_eq!(app.login_flow.poll_token, "tok123");
}

#[test]
fn login_flow_q_during_polling_toggles_qr() {
    let mut app = App::new();
    app.screen = Screen::NextcloudLoginFlow;
    app.login_flow.stage = LoginFlowStage::Polling;
    app.login_flow.login_url = "https://nc.example.org/login".into();
    // QR is shown by default in Polling — pressing `q` hides it,
    // pressing again brings it back.
    assert!(app.login_flow.show_qr);
    app.on_key(key(KeyCode::Char('q')));
    assert!(!app.login_flow.show_qr);
    app.on_key(key(KeyCode::Char('q')));
    assert!(app.login_flow.show_qr);
}

#[test]
fn login_flow_i_during_polling_toggles_inline_qr() {
    let mut app = App::new();
    app.screen = Screen::NextcloudLoginFlow;
    app.login_flow.stage = LoginFlowStage::Polling;
    app.login_flow.login_url = "https://nc.example.org/login".into();
    assert!(!app.login_flow.disable_inline_qr);
    app.on_key(key(KeyCode::Char('i')));
    assert!(app.login_flow.disable_inline_qr);
    app.on_key(key(KeyCode::Char('i')));
    assert!(!app.login_flow.disable_inline_qr);
}

#[test]
fn login_flow_u_opens_url_modal_when_url_set() {
    let mut app = App::new();
    app.screen = Screen::NextcloudLoginFlow;
    app.login_flow.stage = LoginFlowStage::Polling;
    app.login_flow.login_url = "https://nc.example.org/login".into();
    app.on_key(key(KeyCode::Char('u')));
    assert!(app.login_flow.show_url_modal);
    // Esc closes the modal but does NOT exit the flow.
    app.on_key(key(KeyCode::Esc));
    assert!(!app.login_flow.show_url_modal);
    assert_eq!(app.screen, Screen::NextcloudLoginFlow);
}

#[test]
fn login_flow_esc_outside_modal_returns_to_auth_and_resets_state() {
    let mut app = App::new();
    app.screen = Screen::NextcloudLoginFlow;
    app.login_flow.stage = LoginFlowStage::Polling;
    app.login_flow.login_url = "https://nc.example.org/login".into();
    app.login_flow.poll_token = "tok".into();
    app.on_key(key(KeyCode::Esc));
    assert_eq!(app.screen, Screen::NextcloudAuth);
    assert_eq!(app.login_flow.stage, LoginFlowStage::NotStarted);
    assert!(app.login_flow.login_url.is_empty());
    assert!(app.login_flow.poll_token.is_empty());
}

#[test]
fn login_flow_apply_done_populates_credentials_and_state() {
    let mut app = App::new();
    app.screen = Screen::NextcloudLoginFlow;
    app.apply_login_flow_done("alice".into(), "topsecret-app-pw".into());
    assert_eq!(app.login_flow.stage, LoginFlowStage::Done);
    assert_eq!(app.state.username, "alice");
    assert_eq!(app.state.auth_secret, "topsecret-app-pw");
    assert_eq!(app.state.auth_kind, AuthKind::LoginFlow);
}

#[test]
fn login_flow_enter_after_done_advances_to_remote_folder() {
    let mut app = App::new();
    app.screen = Screen::NextcloudLoginFlow;
    app.apply_login_flow_done("alice".into(), "pw".into());
    app.on_key(key(KeyCode::Enter));
    assert_eq!(app.screen, Screen::RemoteFolder);
}

#[test]
fn login_flow_apply_failed_shows_message_and_blocks_advance() {
    let mut app = App::new();
    app.screen = Screen::NextcloudLoginFlow;
    app.apply_login_flow_failed("network error".into());
    assert!(matches!(
        app.login_flow.stage,
        LoginFlowStage::Failed(ref m) if m == "network error"
    ));
    // Enter on Failed should NOT advance.
    app.on_key(key(KeyCode::Enter));
    assert_eq!(app.screen, Screen::NextcloudLoginFlow);
}

#[test]
fn login_flow_c_with_empty_url_is_noop() {
    let mut app = App::new();
    app.screen = Screen::NextcloudLoginFlow;
    // login_url empty
    app.on_key(key(KeyCode::Char('c')));
    assert!(app.login_flow.clipboard_message.is_none());
}

#[test]
fn login_flow_secret_does_not_appear_in_login_flow_state_debug() {
    let mut app = App::new();
    app.apply_login_flow_done("alice".into(), "topsecret-canary".into());
    let dbg = format!("{:?}", app.login_flow);
    assert!(!dbg.contains("topsecret-canary"));
}
