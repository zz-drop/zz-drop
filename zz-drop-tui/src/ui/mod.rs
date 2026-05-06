pub mod layout;

use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::qr::{render_qr, render_qr_image};
use crate::screens::{
    Screen, account::AccountScreen, collision::CollisionScreen, done::DoneScreen,
    login_totp::LoginTotpScreen, nextcloud_auth::NextcloudAuthScreen,
    nextcloud_login_flow::NextcloudLoginFlowScreen, nextcloud_server::NextcloudServerScreen,
    profile_manage::ProfileManageScreen, profile_passphrase::ProfilePassphraseScreen,
    container_picker::ContainerPickerScreen, inner_alias::InnerAliasScreen,
    profile_unlock::ProfileUnlockScreen, provider::ProviderScreen,
    push_profile::PushProfileScreen, remote_folder::RemoteFolderScreen, screen_shows_steps,
    setup_google_drive::SetupGoogleDriveScreen, setup_onedrive::SetupOneDriveScreen,
    test_upload::TestUploadScreen,
    welcome::WelcomeScreen,
};
use crate::theme::Theme;
use crate::tui_widgets::{KeyHint, StepState, keybar, steps as steps_widget, title_bar};
use crate::wizard::WizardMode;

/// 8-step labels matching the original wizard stepper.
const STEP_LABELS: [&str; 8] = [
    "welcome",
    "provider",
    "server",
    "auth",
    "folder",
    "encrypt",
    "push",
    "done",
];

/// Add-inner-profile sub-flow stepper. The container is already
/// unlocked when the operator enters this flow, so there is no
/// "encrypt" or "push" step — the cached KEK re-encrypts in place
/// and there is no remote save. The "done" tick lands on the
/// reused `Screen::Done` rendered in "connection added" mode.
const ADD_INNER_STEP_LABELS: [&str; 6] =
    ["provider", "server", "auth", "folder", "alias", "done"];

fn add_inner_stepper_index(s: Screen) -> Option<usize> {
    match s {
        Screen::Provider => Some(0),
        Screen::NextcloudServer => Some(1),
        Screen::NextcloudAuth
        | Screen::NextcloudLoginFlow
        | Screen::SetupGoogleDrive
        | Screen::SetupOneDrive => {
            Some(2)
        }
        Screen::RemoteFolder | Screen::Collision | Screen::TestUpload => Some(3),
        Screen::InnerAlias => Some(4),
        Screen::Done => Some(5),
        _ => None,
    }
}

pub fn draw(frame: &mut Frame<'_>, app: &mut App, theme: &Theme) {
    let area = frame.area();
    let rects = layout::split(area);

    if rects.too_small {
        let msg = format!(
            "terminal too small ({}x{}), need {}x{}",
            area.width,
            area.height,
            layout::MIN_WIDTH,
            layout::MIN_HEIGHT
        );
        let p = Paragraph::new(msg).alignment(Alignment::Center);
        frame.render_widget(p, area);
        return;
    }

    // Re-test from ProfileManage hits `TestUpload` as a manage
    // action, not as a setup step — hide the wizard stepper and
    // re-route the breadcrumb so the operator does not think they
    // are reconfiguring the profile.
    let in_manage_retest =
        app.screen == Screen::TestUpload && app.test_upload_back.is_some();
    let breadcrumb = if in_manage_retest {
        "manage › probe"
    } else {
        breadcrumb_for(app.screen)
    };
    let pill = app.agent_pill();
    let server_label = app.server_label();
    let pill_ref = &pill;

    {
        let buf = frame.buffer_mut();
        title_bar::render(rects.title, buf, theme, breadcrumb, pill_ref);

        let in_add_inner = app.wizard_mode == WizardMode::AddInnerProfile;
        if !in_manage_retest {
            if in_add_inner && let Some(active) = add_inner_stepper_index(app.screen) {
                let labels = build_add_inner_step_labels(active);
                steps_widget::render(rects.steps, buf, theme, &labels);
            } else if !in_add_inner && screen_shows_steps(app.screen) {
                let active = app.screen.stepper_index();
                let labels = build_step_labels(active);
                steps_widget::render(rects.steps, buf, theme, &labels);
            }
        }
    }

    let mut qr_area: Option<ratatui::layout::Rect> = None;
    {
        let buf = frame.buffer_mut();
        match app.screen {
            Screen::Welcome => WelcomeScreen::render(
                rects.body,
                buf,
                theme,
                app.welcome_item,
                &app.config_dir_display,
                &server_label,
                app.local_exists,
                app.remote_exists,
            ),
            Screen::Provider => {
                ProviderScreen::render(rects.body, buf, theme, app.state.provider_kind)
            }
            Screen::NextcloudServer => NextcloudServerScreen::render(
                rects.body,
                buf,
                theme,
                &app.server_input,
                app.state.server_url_valid(),
            ),
            Screen::NextcloudAuth => NextcloudAuthScreen::render(
                rects.body,
                buf,
                theme,
                app.state.auth_kind,
                &app.username_input,
                &app.secret_input,
                app.auth_focus,
            ),
            Screen::NextcloudLoginFlow => {
                qr_area = NextcloudLoginFlowScreen::render(
                    rects.body,
                    buf,
                    theme,
                    &app.login_flow,
                    &app.state.server_url,
                );
            }
            Screen::SetupGoogleDrive => {
                qr_area =
                    SetupGoogleDriveScreen::render(rects.body, buf, theme, &app.gdrive_setup);
            }
            Screen::SetupOneDrive => {
                qr_area =
                    SetupOneDriveScreen::render(rects.body, buf, theme, &app.onedrive_setup);
            }
            Screen::RemoteFolder => RemoteFolderScreen::render(
                rects.body,
                buf,
                theme,
                &app.remote_folder_input,
                app.state.remote_folder_valid(),
            ),
            Screen::Collision => CollisionScreen::render(rects.body, buf, theme, app.collision),
            Screen::TestUpload => TestUploadScreen::render(
                rects.body,
                buf,
                theme,
                &app.state.probe_progress,
                app.state.last_test_outcome.as_ref(),
                app.test_running,
            ),
            Screen::ProfilePassphrase => ProfilePassphraseScreen::render(
                rects.body,
                buf,
                theme,
                &app.passphrase_input,
                &app.confirm_input,
                app.passphrase_focus,
                &app.passphrase_stage,
                &server_label,
            ),
            Screen::Done => {
                let add_inner_alias = if app.wizard_mode == WizardMode::AddInnerProfile {
                    app.unlocked_profile.as_ref().map(|p| p.alias.as_str())
                } else {
                    None
                };
                DoneScreen::render(
                    rects.body,
                    buf,
                    theme,
                    app.saved_path.as_deref(),
                    app.pushed_summary.as_ref(),
                    &server_label,
                    add_inner_alias,
                )
            }
            Screen::Account => AccountScreen::render(
                rects.body,
                buf,
                theme,
                &app.api_base,
                &server_label,
                &app.account_email_input,
                &app.account_password_input,
                app.account_focus,
                &app.push_flow.stage,
                app.push_flow.mode,
                app.account_validation_error,
            ),
            Screen::LoginTotp => LoginTotpScreen::render(
                rects.body,
                buf,
                theme,
                &app.totp_code_input,
                &app.push_flow.stage,
            ),
            Screen::PushProfile => PushProfileScreen::render(
                rects.body,
                buf,
                theme,
                &app.push_flow,
                &app.push_alias_input,
                &app.api_base,
            ),
            Screen::ProfileUnlock => {
                let path = app
                    .active_profile_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                ProfileUnlockScreen::render(
                    rects.body,
                    buf,
                    theme,
                    &path,
                    &app.manage_passphrase_input,
                    &app.manage_stage,
                    app.manage_unlock_error.as_deref(),
                    app.unlock_source,
                )
            }
            Screen::ProfileManage => ProfileManageScreen::render(
                rects.body,
                buf,
                theme,
                app.unlocked_profile.as_ref(),
                app.manage_show_secret,
                &app.manage_stage,
                &app.api_base,
            ),
            Screen::ContainerPicker => {
                if let Some(set) = app.unlocked_set.as_ref() {
                    let default = app.picker_default_alias.as_deref();
                    ContainerPickerScreen::render(
                        rects.body,
                        buf,
                        theme,
                        set,
                        app.picker_index,
                        default,
                    );
                }
            }
            Screen::InnerAlias => {
                InnerAliasScreen::render(
                    rects.body,
                    buf,
                    theme,
                    &app.inner_alias_input,
                    app.inner_alias_state,
                    app.inner_alias_error.as_deref(),
                );
            }
        }
    }

    if let Some(qr_area) = qr_area {
        let (url, disable_inline) = match app.screen {
            Screen::SetupGoogleDrive => (
                app.gdrive_setup.qr_url().to_string(),
                app.gdrive_setup.disable_inline_qr,
            ),
            Screen::SetupOneDrive => (
                app.onedrive_setup.qr_url().to_string(),
                app.onedrive_setup.disable_inline_qr,
            ),
            _ => (
                app.login_flow.login_url.clone(),
                app.login_flow.disable_inline_qr,
            ),
        };
        let mut rendered = false;
        if !disable_inline {
            if let Some(graphics) = app.graphics.as_mut() {
                rendered = render_qr_image(&url, qr_area, frame, graphics);
            }
        }
        if !rendered {
            let buf = frame.buffer_mut();
            render_qr(&url, qr_area, buf);
        }
    }

    let buf = frame.buffer_mut();
    let hints = keybar_for(app);
    keybar::render(rects.keybar, buf, theme, &hints);
}

fn breadcrumb_for(s: Screen) -> &'static str {
    match s {
        Screen::Welcome => "welcome",
        Screen::Provider => "setup › provider",
        Screen::NextcloudServer => "setup › server",
        Screen::NextcloudAuth => "setup › auth",
        Screen::NextcloudLoginFlow => "setup › auth › login flow",
        Screen::SetupGoogleDrive => "setup › google drive",
        Screen::SetupOneDrive => "setup › onedrive",
        Screen::RemoteFolder => "setup › folder",
        Screen::Collision => "setup › collision",
        Screen::TestUpload => "setup › probe",
        Screen::ProfilePassphrase => "setup › encrypt",
        Screen::Done => "done",
        Screen::Account => "push › account",
        Screen::LoginTotp => "push › account › 2fa",
        Screen::PushProfile => "push › alias",
        Screen::ProfileUnlock => "manage › unlock",
        Screen::ProfileManage => "manage › profile",
        Screen::ContainerPicker => "manage › select profile",
        Screen::InnerAlias => "manage › add connection › alias",
    }
}

fn build_step_labels(active: Option<usize>) -> Vec<(&'static str, StepState)> {
    STEP_LABELS
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let state = match active {
                Some(a) if a == i => StepState::Active,
                Some(a) if a > i => StepState::Past,
                _ => StepState::Future,
            };
            (*label, state)
        })
        .collect()
}

fn build_add_inner_step_labels(active: usize) -> Vec<(&'static str, StepState)> {
    ADD_INNER_STEP_LABELS
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let state = if i == active {
                StepState::Active
            } else if i < active {
                StepState::Past
            } else {
                StepState::Future
            };
            (*label, state)
        })
        .collect()
}

fn keybar_for(app: &App) -> Vec<KeyHint> {
    match app.screen {
        Screen::Welcome => WelcomeScreen::keybar_hint(),
        Screen::Provider => ProviderScreen::keybar_hint(),
        Screen::NextcloudServer => {
            NextcloudServerScreen::keybar_hint(app.state.server_url_valid())
        }
        Screen::NextcloudAuth => NextcloudAuthScreen::keybar_hint(),
        Screen::NextcloudLoginFlow => NextcloudLoginFlowScreen::keybar_hint(&app.login_flow),
        Screen::SetupGoogleDrive => SetupGoogleDriveScreen::keybar_hint(&app.gdrive_setup),
        Screen::SetupOneDrive => SetupOneDriveScreen::keybar_hint(&app.onedrive_setup),
        Screen::RemoteFolder => RemoteFolderScreen::keybar_hint(app.state.remote_folder_valid()),
        Screen::Collision => CollisionScreen::keybar_hint(),
        Screen::TestUpload => TestUploadScreen::keybar_hint(
            app.test_running,
            app.state.last_test_outcome.as_ref(),
        ),
        Screen::ProfilePassphrase => {
            ProfilePassphraseScreen::keybar_hint(&app.passphrase_stage)
        }
        Screen::Done => DoneScreen::keybar_hint(),
        Screen::Account => {
            AccountScreen::keybar_hint(&app.push_flow.stage, app.push_back.is_some())
        }
        Screen::LoginTotp => LoginTotpScreen::keybar_hint(&app.push_flow.stage),
        Screen::PushProfile => PushProfileScreen::keybar_hint(
            &app.push_flow.stage,
            app.push_flow.mode,
            app.push_back.is_some(),
        ),
        Screen::ProfileUnlock => ProfileUnlockScreen::keybar_hint(&app.manage_stage),
        Screen::ProfileManage => {
            ProfileManageScreen::keybar_hint(&app.manage_stage, app.unlock_source)
        }
        Screen::ContainerPicker => ContainerPickerScreen::keybar_hint(),
        Screen::InnerAlias => InnerAliasScreen::keybar_hint(app.inner_alias_state),
    }
}
