use std::io::{self, IsTerminal};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use zz_drop_core::providers::google_drive::{
    self, GoogleDriveAuth, GoogleDriveClient, GoogleDriveProfile,
};
use zz_drop_core::providers::oauth::{DeviceFlowClient, PollOutcome};
use zz_drop_core::providers::onedrive::{
    self as onedrive_core, OneDriveAuth, OneDriveClient, OneDriveProfile,
};
use zz_drop_tui::app::App;
use zz_drop_tui::qr::GraphicsCtx;
use zz_drop_tui::theme::Theme;
use zz_drop_tui::ui;
#[cfg(feature = "remote")]
use zz_drop_tui::api_client;
#[cfg(feature = "remote")]
use zz_drop_tui::api_client::PushLoginOutcome;
use zz_drop_tui::upload_test::{
    self, LoginFlowInitOutcome, LoginFlowPollOutcome, ProbeCleanupOutcome, ProbeEnsureOutcome,
    ProbeStepOutcome, SaveProfileOutcome,
};
use zz_drop_tui::wizard::{GoogleDriveSetupStage, LoginFlowStage, OneDriveSetupStage};

const LOGIN_FLOW_POLL_INTERVAL: Duration = Duration::from_secs(2);

fn main() -> ExitCode {
    if !io::stdout().is_terminal() {
        eprintln!("zz-tui requires an interactive terminal");
        return ExitCode::from(2);
    }

    // Inline graphics policy:
    // - Default: detect the host terminal via env vars; inline only on
    //   an allowlist of terminals known to render quietly (Kitty,
    //   WezTerm, Ghostty). Everything else uses ASCII half-block.
    // - `ZZ_DROP_TUI_INLINE_QR=1` forces inline regardless of the
    //   allowlist, e.g. for iTerm2 users who clicked "always allow".
    // - `ZZ_DROP_TUI_NO_INLINE_QR=1` forces ASCII (legacy opt-out;
    //   beats the opt-in if both are set).
    let inline_opt_in = std::env::var_os("ZZ_DROP_TUI_INLINE_QR")
        .filter(|v| !v.is_empty())
        .is_some();
    let no_inline_legacy = std::env::var_os("ZZ_DROP_TUI_NO_INLINE_QR")
        .filter(|v| !v.is_empty())
        .is_some();
    let graphics = if no_inline_legacy {
        None
    } else {
        GraphicsCtx::detect_with(inline_opt_in)
    };
    let no_inline = graphics.is_none();

    init_diag_log();
    zz_drop_core::diag_log::log("invoke");

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, graphics, no_inline);
    ratatui::restore();

    match result {
        Ok(()) => {
            zz_drop_core::diag_log::log("exit ok");
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("zz-tui error: {e}");
            zz_drop_core::diag_log::log(&format!("exit err={e}"));
            ExitCode::from(1)
        }
    }
}

fn init_diag_log() {
    use zz_drop_core::config::{PathOverrides, discover_paths};
    let uid = rustix::process::geteuid().as_raw();
    if let Ok(paths) = discover_paths(uid, &PathOverrides::default()) {
        let _ = zz_drop_core::config::ensure_dir(&paths.cache_dir, 0o700);
        zz_drop_core::diag_log::init(paths.debug_log_file(), "zz-tui");
    }
}

fn run(
    terminal: &mut DefaultTerminal,
    graphics: Option<GraphicsCtx>,
    no_inline: bool,
) -> io::Result<()> {
    let theme = Theme::detect();
    let mut app = App::new();
    app.graphics = graphics;
    // The env var was already read in `main` to skip `GraphicsCtx::detect()`.
    // We mirror the choice here so the in-app toggle hint reflects it.
    if no_inline {
        app.login_flow.disable_inline_qr = true;
    }
    let mut last_poll: Option<Instant> = None;
    let mut last_gdrive_poll: Option<Instant> = None;
    let mut last_onedrive_poll: Option<Instant> = None;

    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;

        // Probe runs in four stages with a redraw between each so the
        // operator sees the rolling ✓ marks: ensure folder → leave
        // marker → upload test file → delete test file.
        if app.test_request {
            app.test_request = false;
            app.test_running = true;
            app.start_probe();
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            match upload_test::run_probe_ensure(
                &app.state,
                &app.gdrive_setup,
                &app.onedrive_setup,
            ) {
                ProbeEnsureOutcome::Ok(ctx) => {
                    app.mark_probe_ensure_ok();
                    terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
                    match upload_test::run_probe_marker(ctx) {
                        ProbeStepOutcome::Ok(ctx) => {
                            app.mark_probe_marker_ok();
                            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
                            match upload_test::run_probe_upload(ctx) {
                                ProbeStepOutcome::Ok(ctx) => {
                                    app.mark_probe_upload_ok();
                                    terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
                                    match upload_test::run_probe_cleanup(ctx) {
                                        ProbeCleanupOutcome::Ok => app.mark_probe_cleanup_ok(),
                                        ProbeCleanupOutcome::Failed(reason) => {
                                            app.fail_probe_cleanup(reason)
                                        }
                                    }
                                }
                                ProbeStepOutcome::Failed(reason) => app.fail_probe_upload(reason),
                            }
                        }
                        ProbeStepOutcome::Failed(reason) => app.fail_probe_marker(reason),
                    }
                }
                ProbeEnsureOutcome::Failed(reason) => app.fail_probe_ensure(reason),
            }
            continue;
        }

        // Add new inner profile to the unlocked container. The
        // container's KEK is cached in `app.unlocked_kek`, so this
        // path re-encrypts in place without prompting the operator
        // for the passphrase. Also pushes the new set to the agent
        // so subsequent `zz` invocations see all profiles.
        if app.add_inner_request {
            app.add_inner_request = false;
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            let alias = app.inner_alias_input.value().trim().to_string();
            zz_drop_core::diag_log::log(&format!("add_inner alias={alias}"));
            let res = perform_add_inner_profile(&mut app, &alias);
            match &res {
                Ok(new_set) => zz_drop_core::diag_log::log(&format!(
                    "add_inner ok profiles_after={}",
                    new_set.profiles.len()
                )),
                Err(reason) => zz_drop_core::diag_log::log(&format!("add_inner fail reason={reason}")),
            }
            match res {
                Ok(new_set) => app.apply_inner_added(new_set, alias),
                Err(reason) => app.apply_inner_failed(reason),
            }
            continue;
        }

        // Delete the active inner profile from the unlocked
        // container. Re-encrypts with the cached KEK and writes
        // atomically; pushes the new set to the agent so a stale
        // RAM snapshot can't undo the deletion on the next refresh.
        // Also clears the cached default-alias sidecar if it
        // pointed at the deleted profile.
        if app.delete_inner_request {
            app.delete_inner_request = false;
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            let alias = app
                .unlocked_profile
                .as_ref()
                .map(|p| p.alias.clone())
                .unwrap_or_default();
            zz_drop_core::diag_log::log(&format!("delete_inner alias={alias}"));
            let res = perform_delete_inner_profile(&mut app, &alias);
            match &res {
                Ok(new_set) => zz_drop_core::diag_log::log(&format!(
                    "delete_inner ok profiles_after={}",
                    new_set.profiles.len()
                )),
                Err(reason) => {
                    zz_drop_core::diag_log::log(&format!("delete_inner fail reason={reason}"))
                }
            }
            match res {
                Ok(new_set) => app.apply_inner_deleted(new_set, alias),
                Err(reason) => app.apply_inner_delete_failed(reason),
            }
            continue;
        }

        // save profile.zz trigger (TASK 14)
        if app.save_request {
            app.save_request = false;
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            let pass = app.passphrase_input.value().to_string();
            zz_drop_core::diag_log::log(&format!(
                "save_profile pass_len={} provider={:?}",
                pass.len(),
                app.state.provider_kind
            ));
            let outcome = upload_test::run_save_profile(
                &app.state,
                &app.gdrive_setup,
                &app.onedrive_setup,
                &pass,
            );
            match &outcome {
                SaveProfileOutcome::Ok { path } => {
                    zz_drop_core::diag_log::log(&format!("save_profile ok path={path}"));
                }
                SaveProfileOutcome::Failed(reason) => {
                    zz_drop_core::diag_log::log(&format!("save_profile fail reason={reason}"));
                }
            }
            match outcome {
                SaveProfileOutcome::Ok { path } => app.apply_save_done(path),
                SaveProfileOutcome::Failed(reason) => app.apply_save_failed(reason),
            }
            continue;
        }

        // ── manage existing profile: unlock + wipe ────────────────
        if app.unlock_request {
            app.unlock_request = false;
            let pass = app.manage_passphrase_input.value().to_string();
            let path = app.active_profile_path().map(|p| p.to_path_buf());
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            zz_drop_core::diag_log::log(&format!(
                "tui_unlock pass_len={} path={:?}",
                pass.len(),
                path.as_ref().map(|p| p.display().to_string())
            ));
            match path {
                Some(p) => match zz_drop_core::profile::format::load_set_zz(&p, &pass) {
                    Ok((set, kek)) => {
                        zz_drop_core::diag_log::log(&format!(
                            "tui_unlock decrypt_ok profiles={} salt_fnv={:016x}",
                            set.profiles.len(),
                            zz_drop_core::diag_log::fnv64(kek.salt())
                        ));
                        if set.profiles.is_empty() {
                            app.apply_unlock_failed("profile container is empty".into());
                        } else {
                            // Read the cached sidecar default if any
                            // — the picker honours it; with one
                            // profile the call short-circuits to
                            // ProfileManage.
                            let default_alias = container_default_alias(&p);
                            app.apply_unlock_set_done(set, kek, default_alias);
                        }
                    }
                    Err(e) => {
                        zz_drop_core::diag_log::log(&format!(
                            "tui_unlock decrypt_fail kind={e:?}"
                        ));
                        app.apply_unlock_failed(
                            "wrong passphrase or corrupted profile blob".into(),
                        );
                    }
                },
                None => app.apply_unlock_failed("could not resolve profile path".into()),
            }
            continue;
        }
        if app.wipe_request {
            app.wipe_request = false;
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            zz_drop_core::diag_log::log("wipe requested");
            match full_wipe() {
                Ok(()) => {
                    zz_drop_core::diag_log::log("wipe done");
                    app.apply_wipe_done();
                }
                Err(e) => {
                    zz_drop_core::diag_log::log(&format!("wipe fail err={e}"));
                    app.apply_wipe_failed(e);
                }
            }
            continue;
        }

        // ── push to zz-drop.net ────────────────────────────────────
        // Everything in this block hits `zz-drop.net` and is gated
        // by the `remote` feature. v1 ships local-only; in default
        // builds none of these `if`s are present so no network code
        // is compiled in.
        // Account login (step 1).
        #[cfg(feature = "remote")]
        if app.push_request_login {
            app.push_request_login = false;
            let base = app.api_base.clone();
            let email = app.account_email_input.value().to_string();
            let password = app.account_password_input.value().to_string();
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            match api_client::login(&base, &email, &password) {
                Ok(PushLoginOutcome::Session(token)) => app.apply_login_session(token),
                Ok(PushLoginOutcome::TotpRequired(challenge)) => {
                    app.apply_login_totp_required(challenge)
                }
                Err(reason) => app.apply_login_failed(reason),
            }
            continue;
        }
        // TOTP login (step 2).
        #[cfg(feature = "remote")]
        if app.push_request_totp {
            app.push_request_totp = false;
            let base = app.api_base.clone();
            let challenge = app.push_flow.login_challenge.clone().unwrap_or_default();
            let code = app.totp_code_input.value().to_string();
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            match api_client::login_totp(&base, &challenge, &code) {
                Ok(token) => app.apply_totp_session(token),
                Err(reason) => app.apply_totp_failed(reason),
            }
            continue;
        }
        // List aliases.
        #[cfg(feature = "remote")]
        if app.push_request_list {
            app.push_request_list = false;
            let base = app.api_base.clone();
            let token = app.push_flow.session_token.clone().unwrap_or_default();
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            match api_client::list_aliases(&base, &token) {
                Ok(aliases) => app.apply_aliases_loaded(aliases),
                Err(reason) => app.apply_push_failed(reason),
            }
            continue;
        }
        // Wizard push: re-encrypt the local blob with the alias the
        // operator just picked, *before* pushing — so the alias
        // inside the encrypted PlainProfile matches the alias used
        // as key on the server (and shown in the pill).
        #[cfg(feature = "remote")]
        if app.rewrite_blob_for_alias_request {
            app.rewrite_blob_for_alias_request = false;
            let pass = app.passphrase_input.value().to_string();
            let alias = app.push_alias_input.value().to_string();
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            match upload_test::save_profile_with_alias(
                &app.state,
                &app.gdrive_setup,
                &app.onedrive_setup,
                &pass,
                &alias,
            ) {
                upload_test::SaveProfileOutcome::Ok { path } => {
                    app.saved_path = Some(path);
                    app.push_request_send = true;
                }
                upload_test::SaveProfileOutcome::Failed(reason) => {
                    app.apply_push_failed(reason);
                }
            }
            continue;
        }
        // Push the local profile.zz blob.
        #[cfg(feature = "remote")]
        if app.push_request_send {
            app.push_request_send = false;
            let base = app.api_base.clone();
            let token = app.push_flow.session_token.clone().unwrap_or_default();
            let alias = app.push_alias_input.value().to_string();
            let path = app.saved_path.clone().unwrap_or_default();
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            let blob = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => {
                    app.apply_push_failed(format!("could not read {path}: {e}"));
                    continue;
                }
            };
            match api_client::push_blob(&base, &token, &alias, blob) {
                Ok(s) => app.apply_push_done(s.alias, s.blob_size, s.blob_version),
                Err(reason) => app.apply_push_failed(reason),
            }
            continue;
        }
        // SignIn: download the picked alias's blob into
        // `profile-remote.zz`, then route to ProfileUnlock.
        #[cfg(feature = "remote")]
        if app.signin_request_download {
            app.signin_request_download = false;
            let base = app.api_base.clone();
            let token = app.push_flow.session_token.clone().unwrap_or_default();
            let alias = app.push_alias_input.value().to_string();
            let dest = app.profile_remote_path.clone();
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            let dest = match dest {
                Some(p) => p,
                None => {
                    app.apply_push_failed("could not resolve config dir".into());
                    continue;
                }
            };
            match api_client::download_blob(&base, &token, &alias) {
                Ok(blob) => {
                    if let Some(parent) = dest.parent()
                        && let Err(e) = std::fs::create_dir_all(parent)
                    {
                        app.apply_push_failed(format!("could not create config dir: {e}"));
                        continue;
                    }
                    if let Err(e) = std::fs::write(&dest, &blob) {
                        app.apply_push_failed(format!("could not write {}: {e}", dest.display()));
                        continue;
                    }
                    app.apply_signin_done(alias);
                }
                Err(reason) => app.apply_push_failed(reason),
            }
            continue;
        }

        // login flow init trigger (TASK 13)
        if app.login_flow_request_init {
            app.login_flow_request_init = false;
            let server = app.state.server_url.clone();
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            match upload_test::run_login_flow_init(&server) {
                LoginFlowInitOutcome::Ok {
                    login_url,
                    token,
                    endpoint,
                } => {
                    app.apply_login_flow_init(login_url, token, endpoint);
                    last_poll = Some(Instant::now());
                }
                LoginFlowInitOutcome::Failed(reason) => {
                    app.apply_login_flow_init_failed(reason);
                }
            }
            continue;
        }

        // automatic poll while in Polling stage
        if matches!(app.login_flow.stage, LoginFlowStage::Polling) {
            let due = last_poll
                .map(|t| t.elapsed() >= LOGIN_FLOW_POLL_INTERVAL)
                .unwrap_or(true);
            if due {
                last_poll = Some(Instant::now());
                let token = app.login_flow.poll_token.clone();
                let endpoint = app.login_flow.poll_endpoint.clone();
                match upload_test::run_login_flow_poll(&token, &endpoint) {
                    LoginFlowPollOutcome::Pending => {}
                    LoginFlowPollOutcome::Done {
                        login_name,
                        app_password,
                    } => {
                        app.apply_login_flow_done(login_name, app_password);
                    }
                    LoginFlowPollOutcome::Failed(reason) => {
                        app.apply_login_flow_failed(reason);
                    }
                }
            }
        }

        // Google Drive Device Flow: kick off the `device/code` request.
        if app.gdrive_request_init {
            app.gdrive_request_init = false;
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            let cfg = google_drive::device_flow_config();
            let client = DeviceFlowClient::new(cfg);
            match client.initiate() {
                Ok(resp) => {
                    app.apply_gdrive_init(
                        resp.user_code,
                        resp.verification_uri,
                        resp.verification_uri_complete,
                        resp.device_code,
                        resp.expires_in,
                        resp.interval,
                    );
                    last_gdrive_poll = Some(Instant::now());
                }
                Err(e) => app.apply_gdrive_init_failed(format!("{e}")),
            }
            continue;
        }

        // Google Drive: fetch the user email after tokens are issued so
        // the profile summary can show it without asking the operator.
        if app.gdrive_request_email {
            app.gdrive_request_email = false;
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            let auth = GoogleDriveAuth {
                access_token: app.gdrive_setup.access_token.clone(),
                refresh_token: app.gdrive_setup.refresh_token.clone(),
                token_type: app.gdrive_setup.token_type.clone(),
                expires_at: app.gdrive_setup.access_expires_at,
                scope: app.gdrive_setup.scope.clone(),
            };
            let profile = GoogleDriveProfile {
                root_folder: app.gdrive_setup.root_folder.clone(),
                user_email: String::new(),
                root_folder_id: None,
                auth,
            };
            let result = GoogleDriveClient::from_profile(profile)
                .and_then(|c| c.fetch_user_email());
            match result {
                Ok(email) => app.apply_gdrive_email(email),
                Err(e) => app.apply_gdrive_failed(format!("could not resolve account: {e}")),
            }
            continue;
        }

        // Google Drive: tick the token endpoint while the operator is
        // still on the verification page. RFC 8628 polling cadence
        // honours the server-supplied interval and `slow_down` errors.
        if matches!(app.gdrive_setup.stage, GoogleDriveSetupStage::Polling) {
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if app.gdrive_setup.expires_at > 0 && now_secs >= app.gdrive_setup.expires_at {
                app.apply_gdrive_failed("device code expired".into());
                continue;
            }

            let interval = Duration::from_secs(app.gdrive_setup.interval_secs.max(1));
            let due = last_gdrive_poll
                .map(|t| t.elapsed() >= interval)
                .unwrap_or(true);
            if due {
                last_gdrive_poll = Some(Instant::now());
                let device_code = app.gdrive_setup.device_code.clone();
                let cfg = google_drive::device_flow_config();
                let client = DeviceFlowClient::new(cfg);
                match client.poll_once(&device_code) {
                    Ok(PollOutcome::Pending) => {}
                    Ok(PollOutcome::SlowDown) => app.bump_gdrive_interval(),
                    Ok(PollOutcome::Tokens(t)) => {
                        app.apply_gdrive_tokens(
                            t.access_token,
                            t.refresh_token,
                            t.token_type,
                            t.expires_in,
                            t.scope,
                        );
                    }
                    Err(e) => app.apply_gdrive_failed(format!("{e}")),
                }
            }
        }

        // OneDrive Device Flow — same RFC 8628 shape as Google
        // Drive, different endpoints and scope. Mirror the three
        // blocks above (init / fetch_email / polling).
        if app.onedrive_request_init {
            app.onedrive_request_init = false;
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            let cfg = onedrive_core::device_flow_config();
            let client = DeviceFlowClient::new(cfg);
            match client.initiate() {
                Ok(resp) => {
                    app.apply_onedrive_init(
                        resp.user_code,
                        resp.verification_uri,
                        resp.verification_uri_complete,
                        resp.device_code,
                        resp.expires_in,
                        resp.interval,
                    );
                    last_onedrive_poll = Some(Instant::now());
                }
                Err(e) => app.apply_onedrive_init_failed(format!("{e}")),
            }
            continue;
        }

        if app.onedrive_request_email {
            app.onedrive_request_email = false;
            terminal.draw(|frame| ui::draw(frame, &mut app, &theme))?;
            let auth = OneDriveAuth {
                access_token: app.onedrive_setup.access_token.clone(),
                refresh_token: app.onedrive_setup.refresh_token.clone(),
                token_type: app.onedrive_setup.token_type.clone(),
                expires_at: app.onedrive_setup.access_expires_at,
                scope: app.onedrive_setup.scope.clone(),
            };
            let profile = OneDriveProfile {
                root_folder: app.onedrive_setup.root_folder.clone(),
                user_email: String::new(),
                root_folder_id: None,
                auth,
            };
            let result = OneDriveClient::from_profile(profile)
                .and_then(|c| c.fetch_user_email());
            match result {
                Ok(email) => app.apply_onedrive_email(email),
                Err(e) => app.apply_onedrive_failed(format!("could not resolve account: {e}")),
            }
            continue;
        }

        if matches!(app.onedrive_setup.stage, OneDriveSetupStage::Polling) {
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if app.onedrive_setup.expires_at > 0 && now_secs >= app.onedrive_setup.expires_at {
                app.apply_onedrive_failed("device code expired".into());
                continue;
            }

            let interval = Duration::from_secs(app.onedrive_setup.interval_secs.max(1));
            let due = last_onedrive_poll
                .map(|t| t.elapsed() >= interval)
                .unwrap_or(true);
            if due {
                last_onedrive_poll = Some(Instant::now());
                let device_code = app.onedrive_setup.device_code.clone();
                let cfg = onedrive_core::device_flow_config();
                let client = DeviceFlowClient::new(cfg);
                match client.poll_once(&device_code) {
                    Ok(PollOutcome::Pending) => {}
                    Ok(PollOutcome::SlowDown) => app.bump_onedrive_interval(),
                    Ok(PollOutcome::Tokens(t)) => {
                        app.apply_onedrive_tokens(
                            t.access_token,
                            t.refresh_token,
                            t.token_type,
                            t.expires_in,
                            t.scope,
                        );
                    }
                    Err(e) => app.apply_onedrive_failed(format!("{e}")),
                }
            }
        }

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                app.on_key(key);
            }
        }
    }

    Ok(())
}

/// Full local wipe — mirrors `zz w` from the CLI:
///
/// 1. Best-effort: connect to the local agent socket and send `Exit`,
///    so the in-RAM decrypted profile is cleared immediately. If the
///    agent is unreachable or absent, the agent's own auto-lock
///    (10 min) and idle-exit (5 min after lock) provide a backstop.
/// 2. Remove `profile-local.zz` and `profile-remote.zz`.
/// 3. Remove `config.toml`.
/// 4. Remove the agent socket and the token file.
/// 5. Remove the runtime directory entirely (recursive).
/// 6. Remove the config directory if it has no remaining entries.
///
/// Failures on individual file removals are silently ignored — the
/// goal is "best-effort, end with as little local state as possible".
/// The function returns `Err` only when the path resolution itself
/// fails, which is unrecoverable.
/// Build a fresh `PlainProfile` from the current wizard state, append
/// it to the unlocked container, encrypt with the cached KEK, write
/// the new envelope to disk and ship the new set to the agent. The
/// container's passphrase is **never** re-prompted: this is what
/// makes "add another connection" feel natural.
fn perform_add_inner_profile(
    app: &mut App,
    alias: &str,
) -> Result<zz_drop_core::ProfileSet, String> {
    use std::time::SystemTime;
    use zz_drop_core::profile::format::encrypt_set_with_kek;
    use zz_drop_core::providers::google_drive::{GoogleDriveAuth, GoogleDriveProfile};
    use zz_drop_core::providers::nextcloud::types::{NextcloudAuth, NextcloudProfile};
    use zz_drop_core::providers::onedrive::{OneDriveAuth, OneDriveProfile};
    use zz_drop_core::{PlainProfile, ProfileSettings, ProviderProfile};

    if !zz_drop_core::sidecars::validate_alias(alias) {
        return Err("alias rejected".into());
    }

    let kek = app
        .unlocked_kek
        .as_ref()
        .ok_or_else(|| "no cached KEK; run `zz c` and unlock again".to_string())?
        .clone();
    let mut new_set = app
        .unlocked_set
        .clone()
        .ok_or_else(|| "container not in RAM".to_string())?;
    if new_set.contains_alias(alias) {
        return Err("alias already exists in this container".into());
    }

    let (providers, default_target) = match app.state.provider_kind {
        zz_drop_tui::wizard::ProviderKind::Nextcloud => {
            let nc = NextcloudProfile {
                server_url: app.state.server_url.clone(),
                username: app.state.username.clone(),
                auth: NextcloudAuth::AppPassword {
                    secret: app.state.auth_secret.clone(),
                },
                remote_root: app.state.remote_folder.clone(),
            };
            (vec![ProviderProfile::Nextcloud(nc)], "nextcloud")
        }
        zz_drop_tui::wizard::ProviderKind::GoogleDrive => {
            let gd = &app.gdrive_setup;
            if gd.access_token.is_empty() || gd.refresh_token.is_empty() {
                return Err("google drive setup did not yield tokens".into());
            }
            let auth = GoogleDriveAuth {
                access_token: gd.access_token.clone(),
                refresh_token: gd.refresh_token.clone(),
                token_type: gd.token_type.clone(),
                expires_at: gd.access_expires_at,
                scope: gd.scope.clone(),
            };
            let root = if gd.root_folder.is_empty() {
                "zz-drop".to_string()
            } else {
                gd.root_folder.clone()
            };
            let gdp = GoogleDriveProfile {
                root_folder: root,
                user_email: gd.user_email.clone(),
                root_folder_id: None,
                auth,
            };
            (vec![ProviderProfile::GoogleDrive(gdp)], "google_drive")
        }
        zz_drop_tui::wizard::ProviderKind::OneDrive => {
            let od = &app.onedrive_setup;
            if od.access_token.is_empty() || od.refresh_token.is_empty() {
                return Err("onedrive setup did not yield tokens".into());
            }
            let auth = OneDriveAuth {
                access_token: od.access_token.clone(),
                refresh_token: od.refresh_token.clone(),
                token_type: od.token_type.clone(),
                expires_at: od.access_expires_at,
                scope: od.scope.clone(),
            };
            let root = if od.root_folder.is_empty() {
                "zz-drop".to_string()
            } else {
                od.root_folder.clone()
            };
            let odp = OneDriveProfile {
                root_folder: root,
                user_email: od.user_email.clone(),
                root_folder_id: None,
                auth,
            };
            (vec![ProviderProfile::OneDrive(odp)], "onedrive")
        }
    };

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let timestamp = format!("epoch:{now}");
    let new_profile = PlainProfile {
        profile_version: 1,
        profile_id: format!("local-{now}"),
        alias: alias.to_string(),
        default_target: default_target.into(),
        providers,
        collision_policy: app.state.collision,
        settings: ProfileSettings::default(),
        created_at: timestamp.clone(),
        updated_at: timestamp,
    };
    new_set.profiles.push(new_profile);

    // Re-encrypt with the cached KEK and write atomically.
    let envelope =
        encrypt_set_with_kek(&new_set, &kek).map_err(|_| "could not re-encrypt".to_string())?;
    let path = match config_profile_path() {
        Some(p) => p,
        None => return Err("could not resolve config dir".into()),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let tmp = path.with_extension("zz.tmp");
    std::fs::write(&tmp, envelope).map_err(|e| format!("write tmp: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("chmod: {e}"))?;
    }
    std::fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;

    // Best-effort: keep the running agent's in-RAM snapshot in sync
    // with the disk file. Without this, an agent unlocked before
    // this append still holds the old set and will re-encrypt its
    // stale snapshot the next time `update_profile` fires (e.g. on
    // an OAuth token refresh) — silently overwriting the new
    // container.
    if let Some(paths) = agent_paths() {
        zz_drop_tui::agent_kill::try_update_profile_set(
            &paths.agent_socket,
            &paths.token_file,
            &new_set,
        );
    }

    Ok(new_set)
}

/// Remove the inner profile with `alias` from the unlocked
/// container, re-encrypt with the cached KEK, write atomically,
/// push the new set to the agent, and clear the cached default
/// sidecar if it pointed at the removed alias. The container's
/// passphrase is **never** re-prompted.
fn perform_delete_inner_profile(
    app: &mut App,
    alias: &str,
) -> Result<zz_drop_core::ProfileSet, String> {
    use zz_drop_core::profile::format::encrypt_set_with_kek;

    if alias.is_empty() {
        return Err("no active inner profile to delete".into());
    }
    let kek = app
        .unlocked_kek
        .as_ref()
        .ok_or_else(|| "no cached KEK; run `zz c` and unlock again".to_string())?
        .clone();
    let mut new_set = app
        .unlocked_set
        .clone()
        .ok_or_else(|| "container not in RAM".to_string())?;
    let len_before = new_set.profiles.len();
    new_set.profiles.retain(|p| p.alias != alias);
    if new_set.profiles.len() == len_before {
        return Err(format!("alias `{alias}` not found in container"));
    }
    if new_set.profiles.is_empty() {
        // Defence in depth — the TUI guards this case, but the
        // re-encrypt path should refuse too.
        return Err("refusing to write an empty container".into());
    }

    let envelope =
        encrypt_set_with_kek(&new_set, &kek).map_err(|_| "could not re-encrypt".to_string())?;
    let path = match config_profile_path() {
        Some(p) => p,
        None => return Err("could not resolve config dir".into()),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let tmp = path.with_extension("zz.tmp");
    std::fs::write(&tmp, envelope).map_err(|e| format!("write tmp: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("chmod: {e}"))?;
    }
    std::fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;

    // Clear the cached default-alias sidecar if it pointed at the
    // deleted alias. Best-effort: a stale sidecar is just a
    // less-useful picker default, never a correctness issue.
    if let Some(paths) = agent_paths() {
        let sidecar = paths.last_default_local_file.clone();
        if let Ok(d) = zz_drop_core::sidecars::read_local_default(&sidecar)
            && d.alias == alias
        {
            let _ = std::fs::remove_file(&sidecar);
        }
        zz_drop_tui::agent_kill::try_update_profile_set(
            &paths.agent_socket,
            &paths.token_file,
            &new_set,
        );
    }

    Ok(new_set)
}

fn config_profile_path() -> Option<std::path::PathBuf> {
    use zz_drop_core::config::{PathOverrides, discover_paths};
    let uid = rustix::process::geteuid().as_raw();
    discover_paths(uid, &PathOverrides::default())
        .ok()
        .map(|p| p.profiles_local_file)
}

fn agent_paths() -> Option<zz_drop_core::config::Paths> {
    use zz_drop_core::config::{PathOverrides, discover_paths};
    let uid = rustix::process::geteuid().as_raw();
    discover_paths(uid, &PathOverrides::default()).ok()
}

/// Read whichever sidecar is paired with the unlocked container and
/// return the cached default alias. `None` on missing/malformed
/// sidecars — the picker handles that path.
fn container_default_alias(container_path: &std::path::Path) -> Option<String> {
    let parent = container_path.parent()?;
    let file = container_path.file_name()?.to_str()?;
    match file {
        "profiles-local.zz" => {
            zz_drop_core::sidecars::read_local_default(&parent.join("last-default-local"))
                .ok()
                .map(|d| d.alias)
        }
        "profiles-remote.zz" => {
            zz_drop_core::sidecars::read_remote_default(&parent.join("last-default-remote"))
                .ok()
                .and_then(|d| d.alias)
        }
        _ => None,
    }
}

fn full_wipe() -> Result<(), String> {
    let uid = rustix::process::geteuid().as_raw();
    let paths = zz_drop_core::config::discover_paths(
        uid,
        &zz_drop_core::config::PathOverrides::default(),
    )
    .map_err(|e| format!("could not resolve paths: {e}"))?;

    zz_drop_tui::agent_kill::try_exit_agent(&paths.agent_socket, &paths.token_file);

    let _ = std::fs::remove_file(&paths.profiles_local_file);
    let _ = std::fs::remove_file(&paths.profiles_remote_file);
    let _ = std::fs::remove_file(&paths.config_file);
    let _ = std::fs::remove_file(&paths.agent_socket);
    let _ = std::fs::remove_file(&paths.token_file);
    let _ = std::fs::remove_dir_all(&paths.runtime_dir);
    // Only succeeds if the dir is empty — that's intentional.
    let _ = std::fs::remove_dir(&paths.config_dir);

    Ok(())
}
