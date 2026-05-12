use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::input::TextInput;
use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, form_field, panel};
use crate::wizard::PushStage;

pub struct LoginTotpScreen;

impl LoginTotpScreen {
    pub fn title() -> &'static str {
        "two-factor"
    }

    pub fn keybar_hint(stage: &PushStage) -> Vec<KeyHint> {
        match stage {
            PushStage::TotpSending => vec![KeyHint::new("…", "verifying")],
            _ => vec![
                KeyHint::new("↵", "verify"),
                KeyHint::new("esc", "back"),
            ],
        }
    }

    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        code: &TextInput,
        stage: &PushStage,
    ) {
        let inner = panel::open(
            area,
            buf,
            theme,
            PanelAccent::Cyan,
            " two-factor authentication ",
        );
        if inner.height < 8 {
            return;
        }
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(inner);

        let header = Paragraph::new(vec![
            Line::from(Span::styled(
                "  this account has 2FA enabled.",
                theme.body(),
            )),
            Line::from(Span::styled(
                "  enter the 6-digit code from your authenticator app, or one recovery code.",
                theme.dim(),
            )),
        ]);
        ratatui::widgets::Widget::render(header, rows[0], buf);

        form_field::render(rows[1], buf, theme, "code", code, true);

        let footer = match stage {
            PushStage::TotpSending => {
                Line::from(Span::styled("  …  verifying code", theme.cyan()))
            }
            PushStage::Failed(reason) => {
                Line::from(Span::styled(format!("  ✗ {reason}"), theme.danger()))
            }
            _ => Line::from(Span::styled(
                "  recovery codes are 10-character; TOTP codes are 6 digits.",
                theme.dim(),
            )),
        };
        ratatui::widgets::Widget::render(Paragraph::new(footer), rows[2], buf);
    }
}
