pub mod account;
pub mod collision;
pub mod container_picker;
pub mod done;
pub mod inner_alias;
pub mod login_totp;
pub mod nextcloud_auth;
pub mod nextcloud_login_flow;
pub mod nextcloud_server;
pub mod profile_manage;
pub mod setup_dropbox;
pub mod setup_onedrive;
pub mod profile_passphrase;
pub mod profile_unlock;
pub mod provider;
pub mod push_profile;
pub mod remote_folder;
pub mod setup_google_drive;
pub mod test_upload;
pub mod welcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Provider,
    NextcloudServer,
    NextcloudAuth,
    NextcloudLoginFlow,
    /// Google Drive OAuth Device Flow setup. Reached from `Provider`
    /// when the operator picks Google Drive; collects access tokens,
    /// the user email and the root folder name in one screen.
    SetupGoogleDrive,
    /// OneDrive OAuth Device Flow setup. Same shape as
    /// `SetupGoogleDrive` (both follow RFC 8628), driven by
    /// Microsoft identity platform v2 endpoints.
    SetupOneDrive,
    /// Dropbox OAuth Authorization Code + PKCE paste-code setup.
    /// Different shape from `SetupOneDrive` because Dropbox does
    /// not implement the device authorization grant — the operator
    /// approves the URL in a browser and pastes back the code.
    SetupDropbox,
    RemoteFolder,
    Collision,
    TestUpload,
    ProfilePassphrase,
    Done,
    /// Account login (email + password) — entry point of the
    /// "push to zz-drop.net" sub-flow accessible from `Done`.
    Account,
    /// 6-digit TOTP / recovery code prompt, only shown when the
    /// server returns `totp_required` after `Account`.
    LoginTotp,
    /// Pick or type an alias and push the local `profile.zz` blob.
    PushProfile,
    /// Passphrase prompt to decrypt an existing local `profile.zz`.
    /// Reachable from the Welcome menu when the file is present.
    ProfileUnlock,
    /// Read-only view of the unlocked profile, with actions
    /// (re-push, re-test, reveal app password, wipe, back).
    ProfileManage,
    /// Post-unlock picker: choose which inner profile of an
    /// unlocked container to make active. Skipped when the
    /// container holds exactly one profile.
    ContainerPicker,
    /// Alias prompt for the "add new connection" sub-flow. Reached
    /// after TestUpload succeeds when `WizardMode::AddInnerProfile`
    /// is active. Confirming the alias triggers an agent
    /// `UpdateProfileSet` and routes back to ProfileManage.
    InnerAlias,
}

impl Screen {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Welcome => welcome::WelcomeScreen::title(),
            Self::Provider => provider::ProviderScreen::title(),
            Self::NextcloudServer => nextcloud_server::NextcloudServerScreen::title(),
            Self::NextcloudAuth => nextcloud_auth::NextcloudAuthScreen::title(),
            Self::NextcloudLoginFlow => nextcloud_login_flow::NextcloudLoginFlowScreen::title(),
            Self::SetupGoogleDrive => setup_google_drive::SetupGoogleDriveScreen::title(),
            Self::SetupOneDrive => setup_onedrive::SetupOneDriveScreen::title(),
            Self::SetupDropbox => setup_dropbox::SetupDropboxScreen::title(),
            Self::RemoteFolder => remote_folder::RemoteFolderScreen::title(),
            Self::Collision => collision::CollisionScreen::title(),
            Self::TestUpload => test_upload::TestUploadScreen::title(),
            Self::ProfilePassphrase => profile_passphrase::ProfilePassphraseScreen::title(),
            Self::Done => done::DoneScreen::title(),
            Self::Account => account::AccountScreen::title(),
            Self::LoginTotp => login_totp::LoginTotpScreen::title(),
            Self::PushProfile => push_profile::PushProfileScreen::title(),
            Self::ProfileUnlock => profile_unlock::ProfileUnlockScreen::title(),
            Self::ProfileManage => profile_manage::ProfileManageScreen::title(),
            Self::ContainerPicker => container_picker::ContainerPickerScreen::title(),
            Self::InnerAlias => inner_alias::InnerAliasScreen::title(),
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Welcome => Self::Provider,
            // The static `next` keeps Provider → NextcloudServer as
            // the primary path so existing tests stay valid; the
            // actual branch to `SetupGoogleDrive` lives in
            // `App::handle_provider` where the picker selection is
            // known.
            Self::Provider => Self::NextcloudServer,
            Self::NextcloudServer => Self::NextcloudAuth,
            Self::NextcloudAuth => Self::RemoteFolder,
            Self::NextcloudLoginFlow => Self::RemoteFolder,
            // Google Drive setup collects tokens + user email + root
            // folder in a single screen, so it skips RemoteFolder
            // and goes straight to the collision policy step.
            Self::SetupGoogleDrive => Self::Collision,
            Self::SetupOneDrive => Self::Collision,
            Self::SetupDropbox => Self::Collision,
            Self::RemoteFolder => Self::Collision,
            Self::Collision => Self::TestUpload,
            Self::TestUpload => Self::ProfilePassphrase,
            Self::ProfilePassphrase => Self::Done,
            Self::Done => Self::Done,
            // The push sub-flow is sticky on each step — `next()` is
            // not the way you traverse it. Returning the same screen
            // keeps the contract sane for any consumer that calls
            // `next()` blindly.
            Self::Account => Self::Account,
            Self::LoginTotp => Self::LoginTotp,
            Self::PushProfile => Self::PushProfile,
            Self::ProfileUnlock | Self::ProfileManage => self,
            Self::ContainerPicker => Self::ProfileManage,
            Self::InnerAlias => Self::ProfileManage,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Welcome => Self::Welcome,
            Self::Provider => Self::Welcome,
            Self::NextcloudServer => Self::Provider,
            Self::NextcloudAuth => Self::NextcloudServer,
            Self::NextcloudLoginFlow => Self::NextcloudAuth,
            Self::SetupGoogleDrive => Self::Provider,
            Self::SetupOneDrive => Self::Provider,
            Self::SetupDropbox => Self::Provider,
            Self::RemoteFolder => Self::NextcloudAuth,
            Self::Collision => Self::RemoteFolder,
            Self::TestUpload => Self::Collision,
            Self::ProfilePassphrase => Self::TestUpload,
            Self::Done => Self::Done,
            // Push sub-flow: `Esc` semantics depend on the screen
            // (handled in `App::on_key`); the structural `previous()`
            // here is a fallback for callers that traverse blindly.
            Self::Account => Self::Done,
            Self::LoginTotp => Self::Account,
            Self::PushProfile => Self::Done,
            // Manage flow: unlock → manage; manage → welcome.
            Self::ProfileUnlock => Self::Welcome,
            Self::ProfileManage => Self::Welcome,
            // Picker → welcome (Esc locks the container).
            Self::ContainerPicker => Self::Welcome,
            // InnerAlias Esc → cancel the add-inner flow, back to
            // manage.
            Self::InnerAlias => Self::ProfileManage,
        }
    }

    /// Index in the design's 8-step `[welcome / provider / server / auth /
    /// folder / encrypt / push / done]` stepper, or `None` for screens
    /// that don't show the band.
    pub fn stepper_index(self) -> Option<usize> {
        match self {
            Self::Welcome => None,
            Self::Provider => Some(1),
            Self::NextcloudServer => Some(2),
            Self::NextcloudAuth | Self::NextcloudLoginFlow => Some(3),
            // Google Drive Device Flow is the auth step for that
            // provider; it lights up step 3 in the stepper.
            Self::SetupGoogleDrive => Some(3),
            Self::SetupOneDrive => Some(3),
            Self::SetupDropbox => Some(3),
            Self::RemoteFolder | Self::Collision | Self::TestUpload => Some(4),
            Self::ProfilePassphrase => Some(5),
            // step 6 = push (the new "push to zz-drop.net" sub-flow)
            Self::Account | Self::LoginTotp | Self::PushProfile => Some(6),
            Self::Done => Some(7),
            // Manage flow + add-inner lives outside the linear
            // setup stepper.
            Self::ProfileUnlock
            | Self::ProfileManage
            | Self::ContainerPicker
            | Self::InnerAlias => None,
        }
    }
}

/// True iff the screen wants the steps band rendered. `Welcome` and
/// `Done` are framed without the stepper. The push sub-flow does
/// show the band so the operator sees the active "push" step. The
/// manage flow (`ProfileUnlock` / `ProfileManage`) lives outside the
/// linear setup so it omits the band too.
pub fn screen_shows_steps(s: Screen) -> bool {
    !matches!(
        s,
        Screen::Welcome
            | Screen::Done
            | Screen::ProfileUnlock
            | Screen::ProfileManage
            | Screen::ContainerPicker
            | Screen::InnerAlias
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_app_password_path_chain() {
        let mut s = Screen::Welcome;
        for expected in [
            Screen::Provider,
            Screen::NextcloudServer,
            Screen::NextcloudAuth,
            Screen::RemoteFolder,
            Screen::Collision,
            Screen::TestUpload,
            Screen::ProfilePassphrase,
            Screen::Done,
        ] {
            s = s.next();
            assert_eq!(s, expected);
        }
        assert_eq!(s.next(), Screen::Done, "Done is sticky last");
    }

    #[test]
    fn login_flow_jumps_to_remote_folder_on_next() {
        assert_eq!(Screen::NextcloudLoginFlow.next(), Screen::RemoteFolder);
    }

    #[test]
    fn done_does_not_go_back() {
        assert_eq!(Screen::Done.previous(), Screen::Done);
    }

    #[test]
    fn stepper_index_maps_each_screen() {
        assert_eq!(Screen::Welcome.stepper_index(), None);
        assert_eq!(Screen::Provider.stepper_index(), Some(1));
        assert_eq!(Screen::NextcloudServer.stepper_index(), Some(2));
        assert_eq!(Screen::NextcloudAuth.stepper_index(), Some(3));
        assert_eq!(Screen::NextcloudLoginFlow.stepper_index(), Some(3));
        assert_eq!(Screen::SetupGoogleDrive.stepper_index(), Some(3));
        assert_eq!(Screen::RemoteFolder.stepper_index(), Some(4));
        assert_eq!(Screen::Collision.stepper_index(), Some(4));
        assert_eq!(Screen::TestUpload.stepper_index(), Some(4));
        assert_eq!(Screen::ProfilePassphrase.stepper_index(), Some(5));
        assert_eq!(Screen::Done.stepper_index(), Some(7));
    }

    #[test]
    fn setup_google_drive_routing() {
        assert_eq!(Screen::SetupGoogleDrive.next(), Screen::Collision);
        assert_eq!(Screen::SetupGoogleDrive.previous(), Screen::Provider);
    }

    #[test]
    fn welcome_and_done_dont_show_steps() {
        assert!(!screen_shows_steps(Screen::Welcome));
        assert!(!screen_shows_steps(Screen::Done));
        assert!(screen_shows_steps(Screen::Provider));
        assert!(screen_shows_steps(Screen::NextcloudLoginFlow));
        assert!(screen_shows_steps(Screen::ProfilePassphrase));
    }
}
