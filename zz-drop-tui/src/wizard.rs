use std::fmt;

use zz_drop_core::CollisionPolicy;

#[derive(Clone, Default)]
pub struct WizardState {
    pub provider_kind: ProviderKind,
    pub server_url: String,
    pub username: String,
    pub auth_kind: AuthKind,
    pub auth_secret: String,
    pub remote_folder: String,
    pub collision: CollisionPolicy,
    pub last_test_outcome: Option<TestOutcome>,
    pub probe_progress: ProbeProgress,
    /// Alias chosen by the operator on the InnerAlias screen
    /// during a *first* container setup. When `Some`, takes
    /// precedence over the per-provider placeholder
    /// (Nextcloud username / OAuth email local-part) used by
    /// `run_save_profile`. Reset to `None` whenever the wizard
    /// is restarted from the welcome screen.
    pub alias_override: Option<String>,
}

/// Which provider the user picked at the Provider screen. Drives the
/// branching from `Provider` → `NextcloudServer` vs. `Provider` →
/// `SetupGoogleDrive` vs. `Provider` → `SetupOneDrive`. Stored on
/// `WizardState` so re-entering the Provider picker keeps the
/// previous choice highlighted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProviderKind {
    #[default]
    Nextcloud,
    GoogleDrive,
    OneDrive,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProbeStepStatus {
    /// Not yet attempted. Rendered as a dim dash so the operator can read
    /// the upcoming sequence before pressing `t`.
    #[default]
    Pending,
    Busy,
    Ok,
    Err,
    /// Intentionally not run (e.g. cleanup pending TASK 27).
    Skip,
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct ProbeProgress {
    pub ensure: ProbeStepStatus,
    /// Stage 2: persistent "I was here" marker file. Always
    /// overwritten so the operator can find it after the probe runs;
    /// repeated probes update it in place.
    pub marker: ProbeStepStatus,
    pub upload: ProbeStepStatus,
    pub cleanup: ProbeStepStatus,
}

impl fmt::Debug for WizardState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WizardState { <redacted> }")
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WelcomeItem {
    /// Open `profile-local.zz` — the local-only blob that lives only
    /// on this machine. Visible only when the file is on disk.
    OpenLocal,
    /// Open `profile-remote.zz` — a cached server-synced blob.
    /// Visible only when the file is on disk.
    OpenRemote,
    /// Wizard entry — generates a fresh `profile-local.zz`. Always
    /// visible. The flow shows strong "no recovery" warnings, then
    /// gives the operator a *prompt* to push it to a server.
    #[default]
    Configure,
    /// Wizard entry — generates a profile and *automatically* pushes
    /// it to a server, mirroring the encrypted blob into
    /// `profile-remote.zz` on success. Always visible.
    ConfigureRemote,
    /// Sign in to a `zz-drop.net`-compatible server, list the
    /// account's aliases, pick one and download the encrypted blob
    /// into `profile-remote.zz`. Always visible.
    SignIn,
    Quit,
}

/// Drives the post-save behaviour of the Configure wizard:
/// `CreateLocal` shows a `p · push / ↵ · skip` prompt on the Saved
/// stage; `CreateRemote` auto-routes into the push sub-flow and, on
/// success, mirrors `profiles-local.zz` into `profiles-remote.zz`.
/// `AddInnerProfile` is the sub-flow entered from `ProfileManage`
/// when the operator wants to append a new connection to the
/// already-unlocked container — it skips the ProfilePassphrase
/// screen (the container's KEK is cached) and routes through a
/// dedicated `InnerAlias` screen that captures only the new alias.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WizardMode {
    #[default]
    CreateLocal,
    CreateRemote,
    AddInnerProfile,
}

impl WelcomeItem {
    /// Cycle to the next item in `available`. Returns `self` if the
    /// list is empty (defensive — should never happen in practice).
    pub fn next_in(self, available: &[Self]) -> Self {
        match available.iter().position(|x| *x == self) {
            Some(i) if i + 1 < available.len() => available[i + 1],
            Some(_) => available[0],
            None => available.first().copied().unwrap_or(self),
        }
    }
    pub fn previous_in(self, available: &[Self]) -> Self {
        match available.iter().position(|x| *x == self) {
            Some(0) => *available.last().unwrap_or(&self),
            Some(i) => available[i - 1],
            None => available.first().copied().unwrap_or(self),
        }
    }
}

/// State of the manage-existing-profile flow. The unlock screen is
/// effectively `Locked`; once the passphrase is verified the operator
/// lands on `Viewing` and can step into `WipeConfirm` to delete the
/// profile, or trigger transient `RevealPassword` to see the
/// provider secret.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ManageStage {
    /// Locked — passphrase form on screen.
    #[default]
    Locked,
    /// Unlock attempt in flight (decryption is sync, so this is brief).
    Unlocking,
    /// Profile decrypted; `unlocked_profile` is `Some(...)`.
    Viewing,
    /// Operator has just pressed `w`; waits for `y` confirmation.
    WipeConfirm,
    /// File deletion in flight (sync, brief).
    Wiping,
    /// Operator has just pressed `D`; waits for `y` to remove the
    /// active inner profile from the container.
    DeleteInnerConfirm,
    /// Inner-profile delete in flight (sync, brief).
    DeletingInner,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AuthKind {
    #[default]
    AppPassword,
    LoginFlow,
}

impl AuthKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::AppPassword => "app password",
            Self::LoginFlow => "login flow",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::AppPassword => Self::LoginFlow,
            Self::LoginFlow => Self::AppPassword,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TestOutcome {
    Ok,
    Failed(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LoginFlowStage {
    #[default]
    NotStarted,
    Initiating,
    Polling,
    Done,
    Failed(String),
}

pub struct LoginFlowState {
    pub stage: LoginFlowStage,
    pub login_url: String,
    pub poll_token: String,
    pub poll_endpoint: String,
    pub show_url_modal: bool,
    pub show_qr: bool,
    /// When true, force the half-block ASCII renderer instead of the
    /// inline-image protocol. Some terminals advertise Kitty/iTerm/Sixel
    /// support but render the magic bytes as empty area. The user can
    /// toggle this with `i`, or set `ZZ_DROP_TUI_NO_INLINE_QR=1`.
    pub disable_inline_qr: bool,
    pub clipboard_message: Option<&'static str>,
    pub browser_message: Option<&'static str>,
}

impl Default for LoginFlowState {
    fn default() -> Self {
        Self {
            stage: LoginFlowStage::default(),
            login_url: String::new(),
            poll_token: String::new(),
            poll_endpoint: String::new(),
            show_url_modal: false,
            // Default to QR visible: the login flow is meant for headless
            // setups where the phone is the second device.
            show_qr: true,
            disable_inline_qr: false,
            clipboard_message: None,
            browser_message: None,
        }
    }
}

impl fmt::Debug for LoginFlowState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LoginFlowState {{ stage: {:?}, <url+token redacted> }}",
            self.stage
        )
    }
}

/// Stages of the OAuth Device Authorization Grant flow used by the
/// Google Drive setup screen. Mirrors the Nextcloud login-flow shape
/// so the same UI affordances (QR, copy URL, retry on failure) carry
/// over.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GoogleDriveSetupStage {
    #[default]
    NotStarted,
    /// POSTing to the device-authorization endpoint.
    Initiating,
    /// User-facing pair `{user_code, verification_uri}` is on screen;
    /// background polling on the token endpoint is in flight.
    Polling,
    /// Tokens have been issued; resolving the user's email via
    /// `drive/v3/about?fields=user(emailAddress)` so the profile
    /// summary can show "you upload as alice@gmail.com".
    Fetching,
    /// Setup completed — `tokens` and `user_email` populated.
    Done,
    Failed(String),
}

/// State for the Google Drive setup screen. The `device_code` is a
/// short-lived secret bound to the active polling window; the access
/// and refresh tokens are post-success secrets persisted only into
/// the encrypted profile blob, never to disk in clear.
pub struct GoogleDriveSetupState {
    pub stage: GoogleDriveSetupStage,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub device_code: String,
    /// Unix timestamp at which the device_code expires. After this
    /// the polling stops and the screen surfaces a "code expired"
    /// retry affordance.
    pub expires_at: u64,
    /// Polling cadence in seconds, mutated up by 5 when the server
    /// returns `slow_down`.
    pub interval_secs: u64,
    /// Last unix timestamp at which we polled, used to space requests.
    pub last_poll_at: u64,
    pub show_url_modal: bool,
    pub show_qr: bool,
    pub disable_inline_qr: bool,
    pub clipboard_message: Option<&'static str>,
    pub browser_message: Option<&'static str>,
    /// Folder name input. Default `"zz-drop"`. Edited in the Done
    /// stage before the profile is committed.
    pub root_folder: String,
    pub user_email: String,
    /// Populated after the polling completes. The setup flow hands
    /// these off to the profile-creation step and clears them.
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub access_expires_at: u64,
    pub scope: String,
}

impl Default for GoogleDriveSetupState {
    fn default() -> Self {
        Self {
            stage: GoogleDriveSetupStage::default(),
            user_code: String::new(),
            verification_uri: String::new(),
            verification_uri_complete: None,
            device_code: String::new(),
            expires_at: 0,
            interval_secs: 5,
            last_poll_at: 0,
            show_url_modal: false,
            show_qr: true,
            disable_inline_qr: false,
            clipboard_message: None,
            browser_message: None,
            root_folder: "zz-drop".to_string(),
            user_email: String::new(),
            access_token: String::new(),
            refresh_token: String::new(),
            token_type: String::new(),
            access_expires_at: 0,
            scope: String::new(),
        }
    }
}

impl fmt::Debug for GoogleDriveSetupState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GoogleDriveSetupState {{ stage: {:?}, user_email_set: {}, <secrets redacted> }}",
            self.stage,
            !self.user_email.is_empty()
        )
    }
}

impl GoogleDriveSetupState {
    /// URL the QR encodes — prefer the `_complete` form so the user
    /// only needs to scan and approve, falling back to the basic
    /// verification URI when the server didn't send one.
    pub fn qr_url(&self) -> &str {
        self.verification_uri_complete
            .as_deref()
            .unwrap_or(&self.verification_uri)
    }
}

/// OneDrive uses the same RFC 8628 device-authorization shape as
/// Google Drive, so we share the state struct and stage enum
/// rather than maintain two byte-identical copies. The fields are
/// device-flow agnostic; the provider-specific bits (endpoints,
/// scope, branding) live in `zz-drop-core::providers::onedrive`
/// and in the dedicated `screens::setup_onedrive` render.
pub type OneDriveSetupState = GoogleDriveSetupState;
pub type OneDriveSetupStage = GoogleDriveSetupStage;

impl LoginFlowState {
    pub fn truncated_login_url(&self, max_chars: usize) -> String {
        truncate_middle(&self.login_url, max_chars)
    }

    /// Extract `scheme://host` from the login URL, e.g.
    /// `"https://cloud.example.org/index.php/login/v2/flow/abc"` →
    /// `"https://cloud.example.org"`. Returns the full URL when it has
    /// no recognisable scheme.
    pub fn login_url_host(&self) -> String {
        let url = &self.login_url;
        if url.is_empty() {
            return String::new();
        }
        let after_scheme = match url.find("://") {
            Some(i) => i + 3,
            None => return url.clone(),
        };
        match url[after_scheme..].find('/') {
            Some(j) => url[..after_scheme + j].to_string(),
            None => url.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassphraseFocus {
    Passphrase,
    Confirm,
}

impl PassphraseFocus {
    pub fn next(self) -> Self {
        match self {
            Self::Passphrase => Self::Confirm,
            Self::Confirm => Self::Passphrase,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PassphraseStage {
    #[default]
    Editing,
    /// Asked the user to confirm a weak passphrase. Pending y/n.
    WeakWarning,
    /// Encrypt+write running on the main thread; UI shows "encrypting…".
    Encrypting,
    /// Profile saved at the displayed path.
    Saved(String),
    /// Save failed; reason shown inline.
    Failed(String),
}

// ─── push to zz-drop.net (TASK 20 Phase 2) ──────────────────────────

/// Whether the Account/LoginTotp/PushProfile sub-flow is being used
/// to *upload* the local blob (Push) or *download* an alias's blob
/// from the server into `profile-remote.zz` (SignIn).
///
/// Both modes share the login + alias-picker UI. The branch happens
/// after the operator picks an alias.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PushFlowMode {
    /// Upload `saved_path` to the picked alias.
    #[default]
    Push,
    /// Download the picked alias's blob and write it to
    /// `profile-remote.zz`.
    SignIn,
}

/// State of the "push profile to zz-drop.net" sub-flow that lives
/// behind `Done → p`. The flow walks through three screens
/// (`Account`, `LoginTotp`, `PushProfile`); a single `stage` enum
/// drives all three and tells each screen what to render.
pub struct PushFlowState {
    pub stage: PushStage,
    /// Whether this run uploads (Push) or downloads (SignIn).
    pub mode: PushFlowMode,
    /// Holds the in-flight TOTP challenge so the LoginTotp screen
    /// can submit it with the operator-provided code.
    pub login_challenge: Option<String>,
    /// Bearer token for the active session, if any. Set after a
    /// successful Account login (or LoginTotp step 2). Never logged.
    pub session_token: Option<String>,
    /// Aliases the operator already owns on the server, fetched the
    /// first time the PushProfile screen is reached.
    pub remote_aliases: Vec<String>,
    /// Highlighted index inside `remote_aliases` for the "pick an
    /// existing alias" UI. `None` means the operator is typing a
    /// new alias instead of picking one.
    pub picker_index: Option<usize>,
    /// `Some(alias)` after a successful `put_blob` — used by the
    /// success state to display the final summary.
    pub pushed_alias: Option<String>,
    pub pushed_size: u64,
    pub pushed_version: u64,
}

impl Default for PushFlowState {
    fn default() -> Self {
        Self {
            stage: PushStage::AccountForm,
            mode: PushFlowMode::Push,
            login_challenge: None,
            session_token: None,
            remote_aliases: Vec::new(),
            picker_index: None,
            pushed_alias: None,
            pushed_size: 0,
            pushed_version: 0,
        }
    }
}

impl fmt::Debug for PushFlowState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PushFlowState {{ stage: {:?}, <token+challenge redacted> }}",
            self.stage
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PushStage {
    /// Operator typing email + password.
    #[default]
    AccountForm,
    /// Login HTTP call in flight.
    AccountSending,
    /// Operator typing 6-digit code or recovery code.
    TotpForm,
    /// TOTP verify call in flight.
    TotpSending,
    /// Server returned the alias list; operator picking or typing
    /// an alias.
    PushForm,
    /// Initial GET /profiles in flight.
    PushFetching,
    /// PUT /profiles/{alias}/blob in flight.
    PushSending,
    /// Last call succeeded — `pushed_*` fields populated.
    Done,
    /// Last call returned an error; the message is on screen so the
    /// operator can decide whether to retry or `esc` out.
    Failed(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccountFocus {
    #[default]
    Email,
    Password,
}

impl AccountFocus {
    pub fn next(self) -> Self {
        match self {
            Self::Email => Self::Password,
            Self::Password => Self::Email,
        }
    }
}

/// Pretty-printed config directory for the current OS, with `~/`
/// substitution against the user's home directory when applicable.
///
/// - Linux: `~/.config/zz-drop/`
/// - macOS: `~/Library/Application Support/zz-drop/`
/// - Windows: typically `~/AppData/Roaming/zz-drop/`
///
/// Falls back to the literal Linux XDG path when no home directory
/// can be resolved (e.g. in a sandbox without `$HOME`).
pub fn config_dir_display() -> String {
    use directories::BaseDirs;
    let Some(base) = BaseDirs::new() else {
        return "~/.config/zz-drop/".to_string();
    };
    let path = base.config_dir().join("zz-drop");
    let home = base.home_dir();
    match path.strip_prefix(home) {
        Ok(rel) => format!("~/{}/", rel.display()),
        Err(_) => format!("{}/", path.display()),
    }
}

pub fn truncate_middle(s: &str, max_chars: usize) -> String {
    let len = s.chars().count();
    if len <= max_chars {
        return s.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let keep = max_chars - 1;
    let head_len = keep.div_ceil(2);
    let tail_len = keep - head_len;
    let head: String = s.chars().take(head_len).collect();
    let tail_chars: Vec<char> = s.chars().rev().take(tail_len).collect();
    let tail: String = tail_chars.into_iter().rev().collect();
    format!("{head}…{tail}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionChoice {
    Rename,
    Overwrite,
    Fail,
}

impl CollisionChoice {
    pub fn from_policy(p: CollisionPolicy) -> Self {
        match p {
            CollisionPolicy::Rename => Self::Rename,
            CollisionPolicy::Overwrite => Self::Overwrite,
            CollisionPolicy::Fail => Self::Fail,
        }
    }

    pub fn to_policy(self) -> CollisionPolicy {
        match self {
            Self::Rename => CollisionPolicy::Rename,
            Self::Overwrite => CollisionPolicy::Overwrite,
            Self::Fail => CollisionPolicy::Fail,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Rename => Self::Overwrite,
            Self::Overwrite => Self::Fail,
            Self::Fail => Self::Rename,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Rename => Self::Fail,
            Self::Overwrite => Self::Rename,
            Self::Fail => Self::Overwrite,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Rename => "rename — keep both files (foo (1).md)",
            Self::Overwrite => "overwrite — replace remote file silently",
            Self::Fail => "fail — refuse to overwrite",
        }
    }
}

impl WizardState {
    pub fn server_url_valid(&self) -> bool {
        let s = self.server_url.trim();
        if s.is_empty() {
            return false;
        }
        match url::Url::parse(s) {
            Ok(u) => matches!(u.scheme(), "https" | "http"),
            Err(_) => false,
        }
    }

    pub fn username_valid(&self) -> bool {
        !self.username.trim().is_empty()
    }

    pub fn secret_valid(&self) -> bool {
        !self.auth_secret.is_empty()
    }

    pub fn remote_folder_valid(&self) -> bool {
        let s = self.remote_folder.trim();
        if s.is_empty() {
            return false;
        }
        zz_drop_core::providers::nextcloud::path::encode_remote_root(s).is_ok()
    }

    pub fn debug_redacted_does_not_leak(&self) -> String {
        format!("{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_url_validation() {
        let mut w = WizardState::default();
        assert!(!w.server_url_valid());
        w.server_url = "not-a-url".into();
        assert!(!w.server_url_valid());
        w.server_url = "https://nc.example.org".into();
        assert!(w.server_url_valid());
        w.server_url = "http://localhost:8080".into();
        assert!(w.server_url_valid());
        w.server_url = "ftp://nc.example.org".into();
        assert!(!w.server_url_valid());
    }

    #[test]
    fn username_validation() {
        let mut w = WizardState::default();
        assert!(!w.username_valid());
        w.username = "user".into();
        assert!(w.username_valid());
        w.username = "   ".into();
        assert!(!w.username_valid());
    }

    #[test]
    fn remote_folder_validation_uses_core() {
        let mut w = WizardState::default();
        assert!(!w.remote_folder_valid());
        w.remote_folder = "/zz-drop".into();
        assert!(w.remote_folder_valid());
        w.remote_folder = "/a/../b".into();
        assert!(!w.remote_folder_valid());
    }

    #[test]
    fn auth_kind_cycles() {
        let mut k = AuthKind::default();
        assert_eq!(k, AuthKind::AppPassword);
        k = k.next();
        assert_eq!(k, AuthKind::LoginFlow);
        k = k.next();
        assert_eq!(k, AuthKind::AppPassword);
    }

    #[test]
    fn collision_choice_roundtrip() {
        for c in [
            CollisionChoice::Rename,
            CollisionChoice::Overwrite,
            CollisionChoice::Fail,
        ] {
            assert_eq!(CollisionChoice::from_policy(c.to_policy()), c);
        }
    }

    #[test]
    fn debug_does_not_leak_secret() {
        let mut w = WizardState::default();
        w.auth_secret = "topsecret-canary".into();
        let dbg = w.debug_redacted_does_not_leak();
        assert!(!dbg.contains("topsecret-canary"));
    }

    #[test]
    fn truncate_middle_short_passthrough() {
        assert_eq!(truncate_middle("hello", 10), "hello");
    }

    #[test]
    fn truncate_middle_inserts_ellipsis() {
        let out = truncate_middle("https://nc.example.org/index.php/login/v2/flow/abcdef", 30);
        assert_eq!(out.chars().count(), 30);
        assert!(out.contains('…'));
    }

    #[test]
    fn truncate_middle_keeps_head_and_tail_visible() {
        let out = truncate_middle("https://nc.example.org/index.php/login/v2/flow/abcdef", 30);
        assert!(out.starts_with("https://"));
        assert!(out.ends_with("abcdef"));
    }

    #[test]
    fn login_flow_state_debug_does_not_leak() {
        let s = LoginFlowState {
            stage: LoginFlowStage::Polling,
            login_url: "https://nc.example.org/index.php/login/v2/flow/canary-leak".into(),
            poll_token: "token-canary-leak".into(),
            poll_endpoint: "https://nc.example.org/poll".into(),
            show_url_modal: false,
            show_qr: false,
            disable_inline_qr: false,
            clipboard_message: None,
            browser_message: None,
        };
        let d = format!("{s:?}");
        assert!(!d.contains("canary-leak"));
        assert!(!d.contains("token-canary-leak"));
    }
}
