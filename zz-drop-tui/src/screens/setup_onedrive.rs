//! OneDrive setup — OAuth Device Authorization Grant screen.
//!
//! Mirrors the Nextcloud login-flow screen at the UI level: the user
//! sees a short `user_code`, a verification URL, and a QR encoding
//! the URL with the code pre-filled. The TUI polls the token endpoint
//! in the background until tokens are issued, the user denies, or the
//! code expires.
//!
//! Wiring into the wizard event loop and the provider picker happens
//! at the call site in `app.rs`; this module only owns the rendering
//! and the per-stage keybar.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::qr::qr_outer_size;
use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, panel, two_col};
use crate::wizard::{OneDriveSetupStage, OneDriveSetupState};

pub struct SetupOneDriveScreen;

impl SetupOneDriveScreen {
    pub fn title() -> &'static str {
        "onedrive — device flow"
    }

    pub fn keybar_hint(state: &OneDriveSetupState) -> Vec<KeyHint> {
        if state.show_url_modal {
            return vec![KeyHint::new("esc", "close")];
        }
        match state.stage {
            OneDriveSetupStage::NotStarted | OneDriveSetupStage::Initiating => {
                vec![
                    KeyHint::new("…", "contacting microsoft"),
                    KeyHint::new("esc", "back"),
                ]
            }
            OneDriveSetupStage::Polling => {
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
                ];
                if state.show_qr {
                    hints.push(qr_toggle);
                    hints.push(KeyHint::new("q", "hide qr"));
                } else {
                    hints.push(qr_toggle);
                }
                hints.push(KeyHint::new("esc", "cancel"));
                hints
            }
            OneDriveSetupStage::Fetching => {
                vec![KeyHint::new("…", "fetching account"), KeyHint::new("esc", "back")]
            }
            OneDriveSetupStage::Done => vec![
                KeyHint::new("↵", "continue"),
                KeyHint::new("esc", "back"),
            ],
            OneDriveSetupStage::Failed(_) => vec![
                KeyHint::new("r", "retry"),
                KeyHint::new("esc", "back"),
            ],
        }
    }

    /// Render the screen. Returns the `Rect` reserved for the QR (if
    /// any) so the surrounding `ui::draw` can decide between an
    /// inline-image render and ASCII fall-back, mirroring the
    /// Nextcloud login-flow contract.
    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        state: &OneDriveSetupState,
    ) -> Option<Rect> {
        if state.show_url_modal {
            render_url_modal(area, buf, theme, state);
            return None;
        }

        match &state.stage {
            OneDriveSetupStage::NotStarted | OneDriveSetupStage::Initiating => {
                let inner = panel::open(area, buf, theme, PanelAccent::Mint, " device flow ");
                if inner.height >= 3 {
                    let p = Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "  starting Microsoft OAuth Device Flow…",
                            theme.accent_bold(),
                        )),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("  contacting ", theme.dim()),
                            Span::styled("login.microsoftonline.com", theme.cyan()),
                        ]),
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
            OneDriveSetupStage::Polling => render_polling(area, buf, theme, state),
            OneDriveSetupStage::Fetching => {
                let inner = panel::open(area, buf, theme, PanelAccent::Mint, " device flow ");
                if inner.height >= 3 {
                    let p = Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "  resolving Microsoft account…",
                            theme.accent_bold(),
                        )),
                    ]);
                    ratatui::widgets::Widget::render(p, inner, buf);
                }
                None
            }
            OneDriveSetupStage::Done => {
                let inner = panel::open(area, buf, theme, PanelAccent::Mint, " onedrive ");
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
                            Span::styled(state.root_folder.clone(), theme.cyan()),
                        ]),
                    ]);
                    ratatui::widgets::Widget::render(p, inner, buf);
                }
                None
            }
            OneDriveSetupStage::Failed(msg) => {
                let inner = panel::open(area, buf, theme, PanelAccent::Red, " device flow ");
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

fn render_polling(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    state: &OneDriveSetupState,
) -> Option<Rect> {
    let qr_url = state.qr_url();

    let qr_left_outer = qr_outer_size(qr_url)
        .map(|(w, _h)| w)
        .unwrap_or(44)
        .max(44);
    let (left_area, right_area) = two_col::split_with_min_right(area, qr_left_outer, 2, 22);
    let two_col_active = right_area.width > 0;

    if !state.show_qr || !two_col_active {
        let inner = panel::open(area, buf, theme, PanelAccent::Mint, " device flow ");
        render_instructions(inner, buf, theme, state);
        return None;
    }

    let qr_inner = panel::open(left_area, buf, theme, PanelAccent::Mint, " scan ");
    let right_inner = panel::open(right_area, buf, theme, PanelAccent::Mint, " device flow ");
    render_instructions(right_inner, buf, theme, state);
    Some(qr_inner)
}

fn render_instructions(
    inner: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    state: &OneDriveSetupState,
) {
    if inner.height < 6 {
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1)])
        .split(inner);
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  open the URL on any device with a browser:",
            theme.accent_bold(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", theme.dim()),
            Span::styled(state.verification_uri.clone(), theme.cyan()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  enter this code on the page:",
            theme.dim(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", theme.dim()),
            Span::styled(state.user_code.clone(), theme.accent_bold()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  this terminal will continue automatically once approved.",
            theme.dim(),
        )),
    ]);
    ratatui::widgets::Widget::render(p, chunks[0], buf);
}

fn render_url_modal(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    state: &OneDriveSetupState,
) {
    let inner = panel::open(area, buf, theme, PanelAccent::Mint, " verification url ");
    if inner.height < 3 {
        return;
    }
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  full verification URL:",
            theme.accent_bold(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", state.verification_uri),
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
    use crate::wizard::{OneDriveSetupStage, OneDriveSetupState};

    fn polling_state() -> OneDriveSetupState {
        OneDriveSetupState {
            stage: OneDriveSetupStage::Polling,
            user_code: "ABCD-EFGH".into(),
            verification_uri: "https://login.microsoft.com/device".into(),
            verification_uri_complete: Some(
                "https://login.microsoft.com/device?user_code=ABCD-EFGH".into(),
            ),
            ..Default::default()
        }
    }

    #[test]
    fn keybar_hint_changes_per_stage() {
        let mut s = OneDriveSetupState::default();
        assert!(SetupOneDriveScreen::keybar_hint(&s)
            .iter()
            .any(|k| k.label.contains("contacting")));

        s.stage = OneDriveSetupStage::Polling;
        s.show_qr = true;
        let hints = SetupOneDriveScreen::keybar_hint(&s);
        assert!(hints.iter().any(|k| k.label.contains("copy")));
        assert!(hints.iter().any(|k| k.label.contains("hide qr")));

        s.stage = OneDriveSetupStage::Done;
        assert!(SetupOneDriveScreen::keybar_hint(&s)
            .iter()
            .any(|k| k.label.contains("continue")));

        s.stage = OneDriveSetupStage::Failed("expired".into());
        assert!(SetupOneDriveScreen::keybar_hint(&s)
            .iter()
            .any(|k| k.label.contains("retry")));
    }

    #[test]
    fn qr_url_prefers_complete_form() {
        let s = polling_state();
        assert!(s.qr_url().contains("user_code=ABCD-EFGH"));

        let mut s = polling_state();
        s.verification_uri_complete = None;
        assert_eq!(s.qr_url(), "https://login.microsoft.com/device");
    }

    #[test]
    fn debug_redacts_secrets_keeps_stage_marker() {
        let mut s = polling_state();
        s.device_code = "DC-CANARY".into();
        s.access_token = "AT-CANARY".into();
        s.refresh_token = "RT-CANARY".into();
        s.user_email = "alice@example.org".into();
        let d = format!("{s:?}");
        assert!(!d.contains("DC-CANARY"));
        assert!(!d.contains("AT-CANARY"));
        assert!(!d.contains("RT-CANARY"));
        assert!(!d.contains("alice@example.com"));
        assert!(d.contains("Polling"));
        assert!(d.contains("user_email_set: true"));
    }
}
