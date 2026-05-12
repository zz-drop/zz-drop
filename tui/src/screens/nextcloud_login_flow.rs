use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::qr::qr_outer_size;
use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{CheckStatus, KeyHint, check, panel, two_col};
use crate::wizard::{LoginFlowStage, LoginFlowState};

pub struct NextcloudLoginFlowScreen;

impl NextcloudLoginFlowScreen {
    pub fn title() -> &'static str {
        "nextcloud — login flow"
    }

    pub fn keybar_hint(state: &LoginFlowState) -> Vec<KeyHint> {
        if state.show_url_modal {
            return vec![KeyHint::new("esc", "close")];
        }
        match state.stage {
            LoginFlowStage::NotStarted | LoginFlowStage::Initiating => {
                vec![KeyHint::new("…", "contacting server"), KeyHint::new("esc", "back")]
            }
            LoginFlowStage::Polling => {
                if state.show_qr {
                    let qr_toggle = if state.disable_inline_qr {
                        KeyHint::new("i", "inline qr")
                    } else {
                        KeyHint::new("i", "ascii qr")
                    };
                    vec![
                        KeyHint::new("c", "copy"),
                        KeyHint::new("o", "open"),
                        KeyHint::new("u", "url detail"),
                        qr_toggle,
                        KeyHint::new("q", "hide qr"),
                        KeyHint::new("esc", "back"),
                    ]
                } else {
                    vec![
                        KeyHint::new("c", "copy link"),
                        KeyHint::new("o", "open"),
                        KeyHint::new("u", "url detail"),
                        KeyHint::new("q", "show qr"),
                        KeyHint::new("esc", "cancel"),
                    ]
                }
            }
            LoginFlowStage::Done => vec![
                KeyHint::new("↵", "continue"),
                KeyHint::new("esc", "back"),
            ],
            LoginFlowStage::Failed(_) => vec![
                KeyHint::new("r", "retry"),
                KeyHint::new("esc", "back"),
            ],
        }
    }

    /// Render. Returns the `Rect` reserved for the QR (when applicable)
    /// so `ui::draw` can decide whether to render an inline image or
    /// fall back to ASCII.
    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        state: &LoginFlowState,
        server_url: &str,
    ) -> Option<Rect> {
        if state.show_url_modal {
            render_url_modal(area, buf, theme, state);
            return None;
        }

        // Compute the left-pane width that fits a half-block QR for the
        // current login URL (≥ 44 to keep the design default for short
        // URLs). Long URLs (real Nextcloud Login Flow tokens) push this
        // up to ~57 cells, otherwise the renderer would have to fall
        // back to quadrant packing — which doesn't scan from a phone.
        let qr_left_outer = qr_outer_size(&state.login_url)
            .map(|(w, _h)| w)
            .unwrap_or(44)
            .max(44);
        // Two-col on wide terminals: left = qr/instructions, right = link + checks.
        // We accept a right pane down to 22 cells (default split needs 30) so
        // long-URL QRs can keep the side-by-side layout even on 80-col terms.
        let (left_area, right_area) = two_col::split_with_min_right(area, qr_left_outer, 2, 22);
        let two_col_active = right_area.width > 0;

        match &state.stage {
            LoginFlowStage::NotStarted | LoginFlowStage::Initiating => {
                let inner = panel::open(area, buf, theme, PanelAccent::Mint, " login flow ");
                if inner.height >= 3 {
                    let p = Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "  starting Nextcloud Login Flow…",
                            theme.accent_bold(),
                        )),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("  contacting ", theme.dim()),
                            Span::styled(server_url.to_string(), theme.cyan()),
                        ]),
                        Line::from(""),
                        Line::from(Span::styled(
                            "  this can take up to 30s if the server is slow.",
                            theme.dim(),
                        )),
                        Line::from(Span::styled(
                            "  press esc to cancel.",
                            theme.dim(),
                        )),
                    ]);
                    ratatui::widgets::Widget::render(p, inner, buf);
                }
                return None;
            }
            LoginFlowStage::Failed(msg) => {
                let inner = panel::open(area, buf, theme, PanelAccent::Red, " login flow ");
                if inner.height >= 3 {
                    let p = Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(format!("  ✗ {msg}"), theme.danger())),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("  server: ", theme.dim()),
                            Span::styled(server_url.to_string(), theme.cyan()),
                        ]),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("  press ", theme.dim()),
                            Span::styled("r", theme.accent_bold()),
                            Span::styled(" to retry, ", theme.dim()),
                            Span::styled("esc", theme.accent_bold()),
                            Span::styled(" to go back.", theme.dim()),
                        ]),
                    ]);
                    ratatui::widgets::Widget::render(p, inner, buf);
                }
                return None;
            }
            LoginFlowStage::Done => {
                let inner = panel::open(area, buf, theme, PanelAccent::Mint, " login flow ");
                if inner.height >= 3 {
                    let p = Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled("  ✓ authorized", theme.ok())),
                        Line::from(""),
                        Line::from(Span::styled(
                            "  press enter to continue.",
                            theme.dim(),
                        )),
                    ]);
                    ratatui::widgets::Widget::render(p, inner, buf);
                }
                return None;
            }
            LoginFlowStage::Polling => {
                // continues below
            }
        }

        // Polling layout
        let mut qr_area: Option<Rect> = None;

        // Left panel: QR or instructions
        let left_inner = panel::open(
            left_area,
            buf,
            theme,
            PanelAccent::Mint,
            if state.show_qr {
                " authorize with phone "
            } else {
                " open on any device "
            },
        );
        if left_inner.height >= 4 {
            if state.show_qr {
                qr_area = Some(left_inner);
            } else {
                let p = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  open the link (right panel) on any",
                        theme.body(),
                    )),
                    Line::from(Span::styled(
                        "  device with a browser, then authorize",
                        theme.body(),
                    )),
                    Line::from(Span::styled("  zz-drop in Nextcloud.", theme.body())),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  press q to show a QR for this URL.",
                        theme.dim(),
                    )),
                ]);
                ratatui::widgets::Widget::render(p, left_inner, buf);
            }
        }

        if !two_col_active {
            // Single-col fallback: ignore right panel; show URL inline below intro
            return qr_area;
        }

        // Right panel: short URL summary + checks + status messages.
        // Buttons live in the keybar at the bottom of the screen — no
        // need to duplicate them here.
        let right_inner = panel::open(right_area, buf, theme, PanelAccent::Cyan, " link ");
        if right_inner.height < 6 {
            return qr_area;
        }
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(right_inner);

        // Short summary instead of the full URL — the URL is too long to
        // fit on one row and is available in the `u url detail` modal.
        let host = state.login_url_host();
        let url_p = Paragraph::new(vec![
            Line::from(Span::styled("  authorization url", theme.dim())),
            Line::from(vec![
                Span::styled("  ", theme.dim()),
                Span::styled(host, theme.cyan()),
                Span::styled("  ·  press ", theme.dim()),
                Span::styled("u", theme.accent_bold()),
                Span::styled(" for full URL", theme.dim()),
            ]),
        ]);
        ratatui::widgets::Widget::render(url_p, rows[0], buf);

        let header = Paragraph::new(Line::from(Span::styled("  status", theme.dim())));
        ratatui::widgets::Widget::render(header, rows[1], buf);

        check::render_row(
            rows[2],
            buf,
            theme,
            CheckStatus::Ok,
            "poll endpoint",
            Some("reachable"),
        );
        check::render_row(
            rows[3],
            buf,
            theme,
            CheckStatus::Busy,
            "user grant",
            Some("awaiting"),
        );

        // Bottom: status messages
        let mut status_lines: Vec<Line<'_>> = Vec::new();
        if let Some(msg) = state.clipboard_message {
            status_lines.push(Line::from(vec![
                Span::styled("  clipboard: ", theme.dim()),
                Span::styled(msg.to_string(), theme.dim_bright()),
            ]));
        }
        if let Some(msg) = state.browser_message {
            status_lines.push(Line::from(vec![
                Span::styled("  browser:   ", theme.dim()),
                Span::styled(msg.to_string(), theme.dim_bright()),
            ]));
        }
        if !status_lines.is_empty() {
            let p = Paragraph::new(status_lines);
            ratatui::widgets::Widget::render(p, rows[4], buf);
        }

        qr_area
    }
}

fn render_url_modal(area: Rect, buf: &mut Buffer, theme: &Theme, state: &LoginFlowState) {
    let inner = panel::open(area, buf, theme, PanelAccent::Cyan, " url detail ");
    if inner.height < 4 {
        return;
    }
    let body = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled("  full login URL:", theme.dim())),
        Line::from(""),
        Line::from(Span::styled(format!("  {}", state.login_url), theme.cyan())),
        Line::from(""),
        Line::from(Span::styled(
            "  esc closes this view; the polling continues",
            theme.dim(),
        )),
        Line::from(Span::styled("  in the background.", theme.dim())),
    ]);
    ratatui::widgets::Widget::render(body, inner, buf);
}
