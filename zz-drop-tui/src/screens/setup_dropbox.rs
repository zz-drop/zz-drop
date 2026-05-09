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
use crate::tui_widgets::{KeyHint, form_field, panel, two_col};
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
    if inner.height < 9 {
        return;
    }
    // Top paragraph + bordered code field + bottom hint, stacked.
    // Layout (heights):
    //   1 blank
    //   1 "open the URL…"
    //   1 blank
    //   1 the URL itself
    //   1 blank
    //   1 "approve, copy code, paste here:"
    //   3 form_field bordered textbox  ← needs exactly 3 rows
    //   1 blank
    //   1 "press ↵ to exchange"
    //   *  remaining slack
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // blank
            Constraint::Length(1), // intro
            Constraint::Length(1), // blank
            Constraint::Length(1), // URL
            Constraint::Length(1), // blank
            Constraint::Length(1), // "approve, copy, paste here"
            Constraint::Length(3), // form_field
            Constraint::Length(1), // blank
            Constraint::Length(1), // exchange hint
            Constraint::Min(0),
        ])
        .split(inner);

    let intro = Paragraph::new(Line::from(Span::styled(
        "  open the URL on any device with a browser:",
        theme.accent_bold(),
    )));
    ratatui::widgets::Widget::render(intro, chunks[1], buf);

    let url_line = Paragraph::new(Line::from(vec![
        Span::styled("  ", theme.dim()),
        Span::styled(state.authorize_url.clone(), theme.cyan()),
    ]));
    ratatui::widgets::Widget::render(url_line, chunks[3], buf);

    let prompt = Paragraph::new(Line::from(Span::styled(
        "  approve, copy the code Dropbox shows, paste it here:",
        theme.dim(),
    )));
    ratatui::widgets::Widget::render(prompt, chunks[5], buf);

    // Bordered textbox with the typed/pasted code. Always focused
    // while the screen owns the keyboard, so the cursor stays
    // visible and the border is highlighted. The label "code"
    // mirrors the convention used by Nextcloud server / auth
    // screens.
    form_field::render(chunks[6], buf, theme, "code", &state.pasted_code, true);

    let hint_text = if state.pasted_code_appears_valid() {
        "  press ↵ to exchange the code for tokens."
    } else {
        "  type or ctrl-v to paste, then press ↵ to exchange."
    };
    let hint = Paragraph::new(Line::from(Span::styled(hint_text, theme.dim())));
    ratatui::widgets::Widget::render(hint, chunks[8], buf);
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

        s.pasted_code.set_value("ABCDEFGHIJ");
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
        s.pasted_code.set_value("PASTED-CANARY");
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
        s.pasted_code.set_value("abc");
        assert!(!s.pasted_code_appears_valid()); // too short
        s.pasted_code.set_value("valid-code-12345");
        assert!(s.pasted_code_appears_valid());
        s.pasted_code.set_value("has space inside");
        assert!(!s.pasted_code_appears_valid()); // invalid char
    }
}
