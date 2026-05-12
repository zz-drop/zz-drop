// Standalone smoke renderer: prints what each wizard screen looks like
// in the terminal without launching an interactive TUI. Useful to eyeball
// the layout, the keybar, and the "terminal too small" branch.

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use zz_drop_tui::app::App;
use zz_drop_tui::screens::Screen;
use zz_drop_tui::screens::nextcloud_auth::AuthFocus;
use zz_drop_tui::theme::{MockEnv, Theme};
use zz_drop_tui::ui;
use zz_drop_tui::wizard::{
    LoginFlowStage, PassphraseStage, ProbeStepStatus, TestOutcome, WelcomeItem,
};

fn main() {
    // Force colour off so the snapshot is plain ASCII.
    let theme = Theme::from_parts(&MockEnv::empty(), false);

    snap("welcome (no steps band)", &theme, |app| {
        app.screen = Screen::Welcome;
        app.local_exists = false;
        app.remote_exists = false;
        app.welcome_item = WelcomeItem::Configure;
    });

    snap("welcome (with existing local profile)", &theme, |app| {
        app.screen = Screen::Welcome;
        app.local_exists = true;
        app.remote_exists = false;
        app.welcome_item = WelcomeItem::OpenLocal;
    });

    snap("welcome (both local and remote)", &theme, |app| {
        app.screen = Screen::Welcome;
        app.local_exists = true;
        app.remote_exists = true;
        app.welcome_item = WelcomeItem::OpenRemote;
    });

    snap("profile-unlock (form)", &theme, |app| {
        app.screen = Screen::ProfileUnlock;
        app.local_exists = true;
        app.profile_local_path = Some(std::path::PathBuf::from(
            "/Users/alice/Library/Application Support/zz-drop/profile-local.zz",
        ));
        app.manage_passphrase_input.set_value("hunter2hunter2");
    });

    snap("profile-unlock (wrong passphrase)", &theme, |app| {
        app.screen = Screen::ProfileUnlock;
        app.local_exists = true;
        app.profile_local_path = Some(std::path::PathBuf::from(
            "/Users/alice/Library/Application Support/zz-drop/profile-local.zz",
        ));
        app.manage_passphrase_input.set_value("wrongpass1234567");
        app.manage_unlock_error =
            Some("wrong passphrase or corrupted profile blob".into());
    });

    snap("profile-manage (masked · local source)", &theme, |app| {
        use zz_drop_core::providers::nextcloud::NextcloudAuth;
        use zz_drop_core::{
            CollisionPolicy, NextcloudProfile, PlainProfile, ProfileSettings, ProviderProfile,
        };
        app.screen = Screen::ProfileManage;
        app.manage_stage = zz_drop_tui::wizard::ManageStage::Viewing;
        app.local_exists = true;
        app.unlock_source = zz_drop_tui::app::ProfileSource::Local;
        app.unlocked_profile = Some(PlainProfile {
            profile_version: 1,
            profile_id: "local-123".into(),
            alias: "casa-nc".into(),
            default_target: "nextcloud".into(),
            providers: vec![ProviderProfile::Nextcloud(NextcloudProfile {
                server_url: "https://cloud.example.org".into(),
                username: "alice".into(),
                auth: NextcloudAuth::AppPassword {
                    secret: "topsecret-app-pwd".into(),
                },
                remote_root: "/zz-drop".into(),
            })],
            collision_policy: CollisionPolicy::Rename,
            settings: ProfileSettings::default(),
            created_at: "2026-04-28T19:54:00Z".into(),
            updated_at: "2026-04-28T19:54:00Z".into(),
        });
    });

    snap("profile-manage (revealed · remote source)", &theme, |app| {
        use zz_drop_core::providers::nextcloud::NextcloudAuth;
        use zz_drop_core::{
            CollisionPolicy, NextcloudProfile, PlainProfile, ProfileSettings, ProviderProfile,
        };
        app.screen = Screen::ProfileManage;
        app.manage_stage = zz_drop_tui::wizard::ManageStage::Viewing;
        app.manage_show_secret = true;
        app.remote_exists = true;
        app.unlock_source = zz_drop_tui::app::ProfileSource::Remote;
        app.unlocked_profile = Some(PlainProfile {
            profile_version: 1,
            profile_id: "local-123".into(),
            alias: "casa-nc".into(),
            default_target: "nextcloud".into(),
            providers: vec![ProviderProfile::Nextcloud(NextcloudProfile {
                server_url: "https://cloud.example.org".into(),
                username: "alice".into(),
                auth: NextcloudAuth::AppPassword {
                    secret: "topsecret-app-pwd".into(),
                },
                remote_root: "/zz-drop".into(),
            })],
            collision_policy: CollisionPolicy::Rename,
            settings: ProfileSettings::default(),
            created_at: "2026-04-28T19:54:00Z".into(),
            updated_at: "2026-04-28T19:54:00Z".into(),
        });
    });

    snap("profile-manage (wipe confirm)", &theme, |app| {
        use zz_drop_core::providers::nextcloud::NextcloudAuth;
        use zz_drop_core::{
            CollisionPolicy, NextcloudProfile, PlainProfile, ProfileSettings, ProviderProfile,
        };
        app.screen = Screen::ProfileManage;
        app.manage_stage = zz_drop_tui::wizard::ManageStage::WipeConfirm;
        app.api_base = "http://127.0.0.1:8080".into();
        app.unlocked_profile = Some(PlainProfile {
            profile_version: 1,
            profile_id: "local-123".into(),
            alias: "casa-nc".into(),
            default_target: "nextcloud".into(),
            providers: vec![ProviderProfile::Nextcloud(NextcloudProfile {
                server_url: "https://cloud.example.org".into(),
                username: "alice".into(),
                auth: NextcloudAuth::AppPassword {
                    secret: "topsecret".into(),
                },
                remote_root: "/zz-drop".into(),
            })],
            collision_policy: CollisionPolicy::Rename,
            settings: ProfileSettings::default(),
            created_at: "2026-04-28T19:54:00Z".into(),
            updated_at: "2026-04-28T19:54:00Z".into(),
        });
    });

    snap("welcome (Quit selected)", &theme, |app| {
        app.screen = Screen::Welcome;
        app.welcome_item = WelcomeItem::Quit;
    });

    snap("provider", &theme, |app| {
        app.screen = Screen::Provider;
    });

    snap("nextcloud-server (filled)", &theme, |app| {
        app.screen = Screen::NextcloudServer;
        app.server_input.set_value("https://cloud.example.org");
        app.state.server_url = app.server_input.value().to_string();
    });

    snap("nextcloud-auth (two-col, focus secret, masked)", &theme, |app| {
        app.screen = Screen::NextcloudAuth;
        app.username_input.set_value("alice");
        app.secret_input.set_value("topsecret");
        app.state.username = "alice".into();
        app.state.auth_secret = "topsecret".into();
        app.auth_focus = AuthFocus::Secret;
    });

    snap("nextcloud-auth (login flow selected — QR is on next screen)", &theme, |app| {
        app.screen = Screen::NextcloudAuth;
        app.state.auth_kind = zz_drop_tui::wizard::AuthKind::LoginFlow;
        app.auth_focus = AuthFocus::KindSelector;
    });

    snap("login-flow (polling + clipboard hint)", &theme, |app| {
        app.screen = Screen::NextcloudLoginFlow;
        app.login_flow.stage = LoginFlowStage::Polling;
        app.login_flow.login_url =
            "https://cloud.example.org/index.php/login/v2/flow/abcdef0123456789".into();
        app.login_flow.clipboard_message = Some("copied");
    });

    snap("login-flow (qr visible)", &theme, |app| {
        app.screen = Screen::NextcloudLoginFlow;
        app.login_flow.stage = LoginFlowStage::Polling;
        app.login_flow.login_url =
            "https://cloud.example.org/login/v2/flow/abc".into();
        app.login_flow.show_qr = true;
    });

    snap("login-flow (url detail modal)", &theme, |app| {
        app.screen = Screen::NextcloudLoginFlow;
        app.login_flow.stage = LoginFlowStage::Polling;
        app.login_flow.login_url =
            "https://cloud.example.org/index.php/login/v2/flow/abcdef0123456789".into();
        app.login_flow.show_url_modal = true;
    });

    snap("login-flow (done)", &theme, |app| {
        app.screen = Screen::NextcloudLoginFlow;
        app.apply_login_flow_done("alice".into(), "<redacted>".into());
    });

    snap("login-flow (initiating · contacting)", &theme, |app| {
        app.screen = Screen::NextcloudLoginFlow;
        app.state.server_url = "https://cloud.example.org".into();
        app.login_flow.stage = LoginFlowStage::Initiating;
    });

    snap("login-flow (failed · retry)", &theme, |app| {
        app.screen = Screen::NextcloudLoginFlow;
        app.state.server_url = "https://cloud.example.org".into();
        app.login_flow.stage = LoginFlowStage::Failed("network error".into());
    });

    snap("remote-folder (filled)", &theme, |app| {
        app.screen = Screen::RemoteFolder;
        app.remote_folder_input.set_value("/zz-drop");
        app.state.remote_folder = "/zz-drop".into();
    });

    snap("collision", &theme, |app| {
        app.screen = Screen::Collision;
    });

    snap("test-upload (idle)", &theme, |app| {
        app.screen = Screen::TestUpload;
    });

    snap("test-upload (ensure busy)", &theme, |app| {
        app.screen = Screen::TestUpload;
        app.test_running = true;
        app.start_probe();
    });

    snap("test-upload (upload busy)", &theme, |app| {
        app.screen = Screen::TestUpload;
        app.test_running = true;
        app.start_probe();
        app.mark_probe_ensure_ok();
    });

    snap("test-upload (all ok)", &theme, |app| {
        app.screen = Screen::TestUpload;
        app.test_running = true;
        app.start_probe();
        app.mark_probe_ensure_ok();
        app.mark_probe_upload_ok();
    });

    snap("test-upload (ensure failed)", &theme, |app| {
        app.screen = Screen::TestUpload;
        app.test_running = true;
        app.start_probe();
        app.fail_probe_ensure("ensure folder: 401 unauthorized".into());
    });

    snap("test-upload (upload failed)", &theme, |app| {
        app.screen = Screen::TestUpload;
        app.state.probe_progress.ensure = ProbeStepStatus::Ok;
        app.state.probe_progress.upload = ProbeStepStatus::Err;
        app.state.last_test_outcome = Some(TestOutcome::Failed(
            "upload: 507 insufficient storage".into(),
        ));
    });

    snap("profile-passphrase (editing)", &theme, |app| {
        app.screen = Screen::ProfilePassphrase;
        app.passphrase_input.set_value("hunter2hunter2");
        app.confirm_input.set_value("hunter2hunter2");
    });

    snap("profile-passphrase (weak warning)", &theme, |app| {
        app.screen = Screen::ProfilePassphrase;
        app.passphrase_input.set_value("abc");
        app.confirm_input.set_value("abc");
        app.passphrase_stage = PassphraseStage::WeakWarning;
    });

    snap("profile-passphrase (saved · push prompt)", &theme, |app| {
        app.screen = Screen::ProfilePassphrase;
        app.passphrase_stage = PassphraseStage::Saved(
            "/home/alice/.config/zz-drop/profile-local.zz".into(),
        );
    });

    snap("done (pushed — happy path)", &theme, |app| {
        app.screen = Screen::Done;
        app.passphrase_stage =
            PassphraseStage::Saved("/home/alice/.config/zz-drop/profile-local.zz".into());
        app.saved_path = Some("/home/alice/.config/zz-drop/profile-local.zz".into());
        app.apply_push_done("casa-nc".into(), 4096, 1);
    });

    snap("done (local-only · no recovery warning)", &theme, |app| {
        app.screen = Screen::Done;
        app.passphrase_stage =
            PassphraseStage::Saved("/home/alice/.config/zz-drop/profile-local.zz".into());
        app.saved_path = Some("/home/alice/.config/zz-drop/profile-local.zz".into());
        // pushed_summary stays None
    });

    snap("push · account (form)", &theme, |app| {
        app.screen = Screen::Account;
        app.account_email_input.set_value("alice@example.org");
        app.account_password_input.set_value("hunter2hunter2");
    });

    snap("push · account (sending)", &theme, |app| {
        app.screen = Screen::Account;
        app.account_email_input.set_value("alice@example.org");
        app.account_password_input.set_value("hunter2hunter2");
        app.push_flow.stage = zz_drop_tui::wizard::PushStage::AccountSending;
    });

    snap("push · account (failed · wrong creds)", &theme, |app| {
        app.screen = Screen::Account;
        app.account_email_input.set_value("alice@example.org");
        app.account_password_input.set_value("badpassword12345");
        app.push_flow.stage =
            zz_drop_tui::wizard::PushStage::Failed("wrong credentials".into());
    });

    snap("push · totp (form)", &theme, |app| {
        app.screen = Screen::LoginTotp;
        app.push_flow.login_challenge = Some("opaque-challenge".into());
        app.push_flow.stage = zz_drop_tui::wizard::PushStage::TotpForm;
        app.totp_code_input.set_value("123456");
    });

    snap("push · push-profile (picker)", &theme, |app| {
        app.screen = Screen::PushProfile;
        app.apply_aliases_loaded(vec![
            "casa-nc".into(),
            "work.nc".into(),
            "backup-2026".into(),
        ]);
    });

    snap("push · push-profile (typing new alias)", &theme, |app| {
        app.screen = Screen::PushProfile;
        app.apply_aliases_loaded(vec!["casa-nc".into()]);
        app.push_flow.picker_index = None;
        app.push_alias_input.set_value("phone-backup");
    });

    snap("push · push-profile (done — local server)", &theme, |app| {
        app.screen = Screen::PushProfile;
        app.api_base = "http://127.0.0.1:8080".into();
        app.apply_push_done("asdasdasd".into(), 745, 7);
    });

    snap("push · push-profile (done — hosted server)", &theme, |app| {
        app.screen = Screen::PushProfile;
        app.api_base = "https://zz-drop.net".into();
        app.apply_push_done("casa-nc".into(), 4096, 3);
    });

    snap("push · push-profile (failed)", &theme, |app| {
        app.screen = Screen::PushProfile;
        app.apply_push_failed("blob too large".into());
    });

    snap_at("welcome too small (60x20)", 60, 20, &theme, |app| {
        app.screen = Screen::Welcome;
    });
}

fn snap(label: &str, theme: &Theme, f: impl FnOnce(&mut App)) {
    snap_at(label, 120, 32, theme, f);
}

fn snap_at(label: &str, w: u16, h: u16, theme: &Theme, f: impl FnOnce(&mut App)) {
    let backend = TestBackend::new(w, h);
    let mut term = Terminal::new(backend).unwrap();
    let mut app = App::new();
    f(&mut app);
    term.draw(|frame| ui::draw(frame, &mut app, theme)).unwrap();

    println!("──── {label} ────");
    let buf = term.backend().buffer();
    for y in 0..h {
        let mut line = String::with_capacity(w as usize);
        for x in 0..w {
            let cell = &buf[(x, y)];
            let s = cell.symbol();
            line.push_str(if s.is_empty() { " " } else { s });
        }
        println!("{}", line.trim_end());
    }
    println!();
}
