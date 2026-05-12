use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::input::TextInput;
use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, form_field, panel, radio, two_col};
use crate::wizard::AuthKind;

pub struct NextcloudAuthScreen;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthFocus {
    KindSelector,
    Username,
    Secret,
}

impl AuthFocus {
    pub fn next(self) -> Self {
        match self {
            Self::KindSelector => Self::Username,
            Self::Username => Self::Secret,
            Self::Secret => Self::KindSelector,
        }
    }
    pub fn previous(self) -> Self {
        match self {
            Self::KindSelector => Self::Secret,
            Self::Username => Self::KindSelector,
            Self::Secret => Self::Username,
        }
    }
}

impl NextcloudAuthScreen {
    pub fn title() -> &'static str {
        "nextcloud — auth"
    }

    pub fn keybar_hint() -> Vec<KeyHint> {
        vec![
            KeyHint::new("tab", "field"),
            KeyHint::new("←→", "method"),
            KeyHint::new("↵", "continue"),
            KeyHint::new("esc", "back"),
        ]
    }

    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        kind: AuthKind,
        username: &TextInput,
        secret: &TextInput,
        focus: AuthFocus,
    ) {
        let (left_area, right_area) = two_col::split(area, 44, 2);
        let two_col_active = right_area.width > 0;

        // ─── left: method + (when app password) creds ────────────
        let left_inner = panel::open(left_area, buf, theme, PanelAccent::Mint, " method ");
        if left_inner.height >= 6 {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(1),
                ])
                .split(left_inner);

            let kind_focused = focus == AuthFocus::KindSelector;
            radio::render_row(
                rows[0],
                buf,
                theme,
                kind_focused && matches!(kind, AuthKind::AppPassword),
                matches!(kind, AuthKind::AppPassword),
                "App Password",
                Some("paste from Nextcloud"),
                false,
            );
            radio::render_row(
                rows[1],
                buf,
                theme,
                kind_focused && matches!(kind, AuthKind::LoginFlow),
                matches!(kind, AuthKind::LoginFlow),
                "Login Flow v2",
                Some("recommended · headless ok"),
                false,
            );
            // disabled extra row (spacing for visual parity with design)
            radio::render_row(
                rows[2],
                buf,
                theme,
                false,
                false,
                "Basic Auth",
                Some("not recommended"),
                true,
            );

            // app-password fields under the method choice
            if matches!(kind, AuthKind::AppPassword) {
                form_field::render(
                    rows[3],
                    buf,
                    theme,
                    "username",
                    username,
                    focus == AuthFocus::Username,
                );
                form_field::render(
                    rows[4],
                    buf,
                    theme,
                    "app password",
                    secret,
                    focus == AuthFocus::Secret,
                );
            }
        }

        // ─── right: contextual panel ────────────────────────────
        if !two_col_active {
            return;
        }
        match kind {
            AuthKind::LoginFlow => render_login_flow_pane(right_area, buf, theme),
            AuthKind::AppPassword => render_app_password_pane(right_area, buf, theme),
        }
    }
}

fn render_login_flow_pane(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let inner = panel::open(area, buf, theme, PanelAccent::Cyan, " Login Flow v2 ");
    if inner.height < 4 {
        return;
    }
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  zz-drop will not open a browser by default.",
            theme.dim(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  press enter to start; you'll get a link to",
            theme.body(),
        )),
        Line::from(Span::styled(
            "  open on any device. zz polls and finishes",
            theme.body(),
        )),
        Line::from(Span::styled("  here automatically.", theme.body())),
        Line::from(""),
        Line::from(Span::styled(
            "  • headless ok",
            theme.dim(),
        )),
        Line::from(Span::styled(
            "  • copy / qr / open buttons inside the next screen",
            theme.dim(),
        )),
    ]);
    ratatui::widgets::Widget::render(p, inner, buf);
}

fn render_app_password_pane(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let inner = panel::open(area, buf, theme, PanelAccent::Cyan, " app password ");
    if inner.height < 4 {
        return;
    }
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  generate an app password from your",
            theme.body(),
        )),
        Line::from(Span::styled(
            "  Nextcloud → settings → security → devices",
            theme.body(),
        )),
        Line::from(Span::styled(
            "  & sessions, then paste it here.",
            theme.body(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  the app password lives only inside the",
            theme.dim(),
        )),
        Line::from(Span::styled("  encrypted profile.zz.", theme.dim())),
    ]);
    ratatui::widgets::Widget::render(p, inner, buf);
}
