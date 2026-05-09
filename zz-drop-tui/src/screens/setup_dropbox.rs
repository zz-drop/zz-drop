//! Dropbox setup — OAuth Authorization Code + PKCE paste-code screen.
//!
//! Differs from `setup_onedrive` / `setup_google_drive` because
//! Dropbox does not implement RFC 8628 device authorization. The
//! flow is:
//!
//! 1. zz-drop builds an authorize URL locally (PKCE
//!    `code_challenge` derived from a freshly generated
//!    `code_verifier`) and shows it to the operator.
//! 2. Operator opens the URL in any browser, approves, and Dropbox
//!    displays an authorization code on the dropbox.com consent
//!    page.
//! 3. Operator types the code back into the TUI.
//! 4. zz-drop POSTs `code` + `code_verifier` to the Dropbox token
//!    endpoint, receives access + refresh tokens, fetches the
//!    account email, and lands on `Done`.
//!
//! Wiring into the wizard event loop happens at the call site in
//! `app.rs`; this module owns the rendering and the per-stage
//! keybar hints.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::qr::qr_outer_size;
use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, panel, two_col};
use crate::wizard::{DropboxSetupStage, DropboxSetupState};

pub struct SetupDropboxScreen;

impl SetupDropboxScreen {
    pub fn title() -> &'static str {
        "dropbox — paste-code"
    }

    pub fn keybar_hint(state: &DropboxSetupState) -> Vec<KeyHint> {
        if state.show_url_modal {
            return vec![KeyHint::new("esc", "close")];
        }
        match &state.stage {
            DropboxSetupStage::NotStarted | DropboxSetupStage::Initiating => {
                vec![
                    KeyHint::new("…", "preparing url"),
                    KeyHint::new("esc", "back"),
                ]
            }
            DropboxSetupStage::AwaitingPaste => {
                let qr_toggle = if state.show_qr {
                    if state.disable_inline_qr {
                        KeyHint::new("i", "inline qr")
                    } else {
                        KeyHint::new("i", "ascii qr")
                    }
                } else {
                    KeyHint::new("q", "show qr")
                };
                let mut hints = vec![
                    KeyHint::new("c", "copy url"),
                    KeyHint::new("o", "open"),
                    KeyHint::new("u", "url detail"),
                    KeyHint::new("ctrl-v", "paste code"),
                ];
                if state.pasted_code_appears_valid() {
                    hints.push(KeyHint::new("↵", "exchange"));
                }
                if state.show_qr {
                    hints.push(qr_toggle);
                    hints.push(KeyHint::new("q", "hide qr"));
                } else {
                    hints.push(qr_toggle);
                }
                hints.push(KeyHint::new("esc", "cancel"));
                hints
            }
            DropboxSetupStage::Exchanging => {
                vec![
                    KeyHint::new("…", "exchanging code"),
                    KeyHint::new("esc", "back"),
                ]
            }
            DropboxSetupStage::Fetching => {
                vec![
                    KeyHint::new("…", "fetching account"),
                    KeyHint::new("esc", "back"),
                ]
            }
            DropboxSetupStage::Done => vec![
                KeyHint::new("↵", "continue"),
                KeyHint::new("esc", "back"),
            ],
            DropboxSetupStage::Failed(_) => vec![
                KeyHint::new("r", "retry"),
                KeyHint::new("esc", "back"),
            ],
        }
    }

    /// Render the screen. Returns the `Rect` reserved for the QR
    /// (if any) so the surrounding `ui::draw` can decide between
    /// inline-image and ASCII fallback, mirroring the contract of
    /// the device-flow setup screens.
    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        state: &DropboxSetupState,
    ) -> Option<Rect> {
        if state.show_url_modal {
            render_url_modal(area, buf, theme, state);
            return None;
        }

        match &state.stage {
            DropboxSetupStage::NotStarted | DropboxSetupStage::Initiating => {
                let inner = panel::open(area, buf, theme, PanelAccent::Mint, " paste-code ");
                if inner.height >= 3 {
                    let p = Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "  preparing Dropbox authorization URL…",
                            theme.accent_bold(),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            "  press esc to cancel.",
                            theme.dim(),
                        )),
                    ]);
                    ratatui::widgets::Widget::render(p, inner, buf);
                }
                None
            }
            DropboxSetupStage::AwaitingPaste => render_awaiting_paste(area, buf, theme, state),
            DropboxSetupStage::Exchanging => {
                let inner = panel::open(area, buf, theme, PanelAccent::Mint, " paste-code ");
                if inner.height >= 3 {
                    let p = Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "  exchanging code for tokens…",
                            theme.accent_bold(),
                        )),
                    ]);
                    ratatui::widgets::Widget::render(p, inner, buf);
                }
                None
            }
            DropboxSetupStage::Fetching => {
                let inner = panel::open(area, buf, theme, PanelAccent::Mint, " paste-code ");
                if inner.height >= 3 {
                    let p = Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "  resolving Dropbox account…",
                            theme.accent_bold(),
                        )),
                    ]);
                    ratatui::widgets::Widget::render(p, inner, buf);
                }
                None
            }
            DropboxSetupStage::Done => {
                let inner = panel::open(area, buf, theme, PanelAccent::Mint, " dropbox ");
                if inner.height >= 4 {
                    let p = Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "  signed in.",
                            theme.accent_bold(),
                        )),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("  account: ", theme.dim()),
                            Span::styled(state.user_email.clone(), theme.cyan()),
                        ]),
                        Line::from(vec![
                            Span::styled("  folder:  ", theme.dim()),
                            Span::styled(
                                format!("Apps/zz-drop/{}", state.root_folder),
                                theme.cyan(),
                            ),
                        ]),
                    ]);
                    ratatui::widgets::Widget::render(p, inner, buf);
                }
                None
            }
            DropboxSetupStage::Failed(msg) => {
                let inner = panel::open(area, buf, theme, PanelAccent::Red, " paste-code ");
                if inner.height >= 3 {
                    let p = Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "  setup failed.",
                            theme.danger(),
                        )),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("  reason: ", theme.dim()),
                            Span::styled(msg.clone(), theme.danger()),
                        ]),
                        Line::from(""),
                        Line::from(Span::styled(
                            "  press r to retry, esc to go back.",
                            theme.dim(),
                        )),
                    ]);
                    ratatui::widgets::Widget::render(p, inner, buf);
                }
                None
            }
        }
    }
}

fn render_awaiting_paste(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    state: &DropboxSetupState,
) -> Option<Rect> {
    let qr_url = state.qr_url();

    let qr_left_outer = qr_outer_size(qr_url)
        .map(|(w, _h)| w)
        .unwrap_or(44)
        .max(44);
    let (left_area, right_area) = two_col::split_with_min_right(area, qr_left_outer, 2, 22);
    let two_col_active = right_area.width > 0;

    if !state.show_qr || !two_col_active {
        let inner = panel::open(area, buf, theme, PanelAccent::Mint, " paste-code ");
        render_instructions(inner, buf, theme, state);
        return None;
    }

    let qr_inner = panel::open(left_area, buf, theme, PanelAccent::Mint, " scan ");
    let right_inner = panel::open(right_area, buf, theme, PanelAccent::Mint, " paste-code ");
    render_instructions(right_inner, buf, theme, state);
    Some(qr_inner)
}

fn render_instructions(
    inner: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    state: &DropboxSetupState,
) {
    if inner.height < 6 {
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1)])
        .split(inner);

    let pasted_display = if state.pasted_code.is_empty() {
        Span::styled("  (empty)  type or ctrl-v to paste", theme.dim())
    } else {
        Span::styled(
            format!("  {}", redact_middle(&state.pasted_code)),
            theme.accent_bold(),
        )
    };

    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  open the URL on any device with a browser:",
            theme.accent_bold(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", theme.dim()),
            Span::styled(state.authorize_url.clone(), theme.cyan()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  approve, copy the code Dropbox shows, paste it here:",
            theme.dim(),
        )),
        Line::from(""),
        Line::from(pasted_display),
        Line::from(""),
        Line::from(Span::styled(
            "  press ↵ to exchange the code for tokens.",
            theme.dim(),
        )),
    ]);
    ratatui::widgets::Widget::render(p, chunks[0], buf);
}

fn render_url_modal(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    state: &DropboxSetupState,
) {
    let inner = panel::open(area, buf, theme, PanelAccent::Mint, " authorize url ");
    if inner.height < 3 {
        return;
    }
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  full Dropbox authorize URL:",
            theme.accent_bold(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", state.authorize_url),
            theme.cyan(),
        )),
        Line::from(""),
        Line::from(Span::styled("  esc closes this overlay.", theme.dim())),
    ]);
    ratatui::widgets::Widget::render(p, inner, buf);
}

/// Show only the first 3 and last 3 chars of the pasted code in the
/// AwaitingPaste view. The code is short-lived (≤10 minutes per
/// Dropbox docs) and not a long-term secret, but rendering it in
/// full would print it to the user's terminal scrollback. The
/// abbreviated form lets the operator confirm "I pasted something"
/// without pasting the entire code into history.
fn redact_middle(s: &str) -> String {
    let trimmed = s.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 6 {
        return chars.iter().map(|_| '•').collect();
    }
    let head: String = chars.iter().take(3).collect();
    let mut tail_chars: Vec<char> = chars.iter().rev().take(3).copied().collect();
    tail_chars.reverse();
    let tail: String = tail_chars.into_iter().collect();
    let middle_len = chars.len() - 6;
    format!("{head}{}{tail}", "•".repeat(middle_len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wizard::{DropboxSetupStage, DropboxSetupState};

    fn awaiting_state() -> DropboxSetupState {
        DropboxSetupState {
            stage: DropboxSetupStage::AwaitingPaste,
            authorize_url: "https://www.dropbox.com/oauth2/authorize?client_id=abc&response_type=code&code_challenge=XYZ&code_challenge_method=S256&token_access_type=offline".into(),
            code_verifier: "verifier-canary".into(),
            ..Default::default()
        }
    }

    #[test]
    fn keybar_hint_changes_per_stage() {
        let mut s = DropboxSetupState::default();
        assert!(SetupDropboxScreen::keybar_hint(&s)
            .iter()
            .any(|k| k.label.contains("preparing")));

        s.stage = DropboxSetupStage::AwaitingPaste;
        s.show_qr = true;
        let hints = SetupDropboxScreen::keybar_hint(&s);
        assert!(hints.iter().any(|k| k.label.contains("copy")));
        assert!(hints.iter().any(|k| k.label.contains("paste")));
        assert!(hints.iter().any(|k| k.label.contains("hide qr")));
        // ↵ exchange only after a valid-looking paste.
        assert!(!hints.iter().any(|k| k.label.contains("exchange")));

        s.pasted_code = "ABCDEFGHIJ".into();
        let hints = SetupDropboxScreen::keybar_hint(&s);
        assert!(hints.iter().any(|k| k.label.contains("exchange")));

        s.stage = DropboxSetupStage::Exchanging;
        assert!(SetupDropboxScreen::keybar_hint(&s)
            .iter()
            .any(|k| k.label.contains("exchanging")));

        s.stage = DropboxSetupStage::Done;
        assert!(SetupDropboxScreen::keybar_hint(&s)
            .iter()
            .any(|k| k.label.contains("continue")));

        s.stage = DropboxSetupStage::Failed("expired".into());
        assert!(SetupDropboxScreen::keybar_hint(&s)
            .iter()
            .any(|k| k.label.contains("retry")));
    }

    #[test]
    fn qr_url_returns_authorize_url() {
        let s = awaiting_state();
        assert!(s.qr_url().starts_with("https://www.dropbox.com/oauth2/authorize"));
    }

    #[test]
    fn debug_redacts_secrets_keeps_stage_marker() {
        let mut s = awaiting_state();
        s.code_verifier = "VERIFIER-CANARY".into();
        s.access_token = "AT-CANARY".into();
        s.refresh_token = "RT-CANARY".into();
        s.pasted_code = "PASTED-CANARY".into();
        s.user_email = "alice@example.org".into();
        let d = format!("{s:?}");
        assert!(!d.contains("VERIFIER-CANARY"));
        assert!(!d.contains("AT-CANARY"));
        assert!(!d.contains("RT-CANARY"));
        assert!(!d.contains("PASTED-CANARY"));
        assert!(!d.contains("alice@example.org"));
        assert!(d.contains("AwaitingPaste"));
        assert!(d.contains("user_email_set: true"));
    }

    #[test]
    fn pasted_code_appears_valid_rejects_short_or_garbage() {
        let mut s = DropboxSetupState::default();
        assert!(!s.pasted_code_appears_valid());
        s.pasted_code = "abc".into();
        assert!(!s.pasted_code_appears_valid()); // too short
        s.pasted_code = "valid-code-12345".into();
        assert!(s.pasted_code_appears_valid());
        s.pasted_code = "has space inside".into();
        assert!(!s.pasted_code_appears_valid()); // invalid char
    }

    #[test]
    fn redact_middle_keeps_endpoints_visible() {
        assert_eq!(redact_middle("ABC123XYZ"), "ABC•••XYZ");
        // Short codes get fully bulleted.
        assert_eq!(redact_middle("abc"), "•••");
        // Trims surrounding whitespace.
        assert_eq!(redact_middle("  ABCDEFGHI  "), "ABC•••GHI");
    }
}
