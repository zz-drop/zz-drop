use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::input::TextInput;
use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, form_field, panel};
use crate::wizard::{AccountFocus, PushFlowMode, PushStage};

pub struct AccountScreen;

impl AccountScreen {
    pub fn title() -> &'static str {
        "account login"
    }

    pub fn keybar_hint(stage: &PushStage, can_back: bool) -> Vec<KeyHint> {
        if matches!(stage, PushStage::AccountSending) {
            return vec![KeyHint::new("…", "logging in")];
        }
        let primary = match stage {
            PushStage::Failed(_) => KeyHint::new("↵", "retry"),
            _ => KeyHint::new("↵", "login"),
        };
        let mut hints = vec![primary, KeyHint::new("tab", "field")];
        // Always offer Esc when the request failed — server outage
        // shouldn't trap the operator, even in wizard mode.
        if can_back || matches!(stage, PushStage::Failed(_)) {
            hints.push(KeyHint::new("esc", "back"));
        }
        hints
    }

    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        api_base: &str,
        server_label: &str,
        email: &TextInput,
        password: &TextInput,
        focus: AccountFocus,
        stage: &PushStage,
        mode: PushFlowMode,
        validation_error: Option<&'static str>,
    ) {
        let title = match mode {
            PushFlowMode::Push => format!(" push profile to {server_label} "),
            PushFlowMode::SignIn => format!(" sign in to {server_label} "),
        };
        let inner = panel::open(area, buf, theme, PanelAccent::Mint, &title);
        if inner.height < 9 {
            return;
        }
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(inner);

        let intro = match mode {
            PushFlowMode::Push => {
                "  log in to push your encrypted profile to this server."
            }
            PushFlowMode::SignIn => {
                "  log in to download an alias from this server into this machine."
            }
        };
        let header = Paragraph::new(vec![
            Line::from(Span::styled(intro, theme.dim())),
            Line::from(vec![
                Span::styled("  server: ", theme.dim()),
                Span::styled(api_base.to_string(), theme.cyan()),
            ]),
        ]);
        ratatui::widgets::Widget::render(header, rows[0], buf);

        form_field::render(
            rows[1],
            buf,
            theme,
            "email",
            email,
            focus == AccountFocus::Email,
        );
        form_field::render(
            rows[2],
            buf,
            theme,
            "password",
            password,
            focus == AccountFocus::Password,
        );

        // Validation hint takes precedence over the neutral tip but
        // not over an in-flight request or a network failure: those
        // describe a more recent state.
        let footer = match (stage, validation_error) {
            (PushStage::AccountSending, _) => {
                Line::from(Span::styled("  …  contacting server", theme.cyan()))
            }
            (PushStage::Failed(reason), _) => {
                Line::from(Span::styled(format!("  ✗ {reason}"), theme.danger()))
            }
            (_, Some(reason)) => {
                Line::from(Span::styled(format!("  ✗ {reason}"), theme.danger()))
            }
            (_, None) => Line::from(Span::styled(
                "  the password is sent over TLS; the session lives only in RAM.",
                theme.dim(),
            )),
        };
        ratatui::widgets::Widget::render(Paragraph::new(footer), rows[3], buf);
    }
}
