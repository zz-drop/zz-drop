use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::input::TextInput;
use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, TagKind, form_field, panel, tag};

pub struct NextcloudServerScreen;

impl NextcloudServerScreen {
    pub fn title() -> &'static str {
        "nextcloud — server"
    }

    pub fn keybar_hint(_valid: bool) -> Vec<KeyHint> {
        vec![KeyHint::new("↵", "continue"), KeyHint::new("esc", "back")]
    }

    pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme, input: &TextInput, valid: bool) {
        let inner = panel::open(area, buf, theme, PanelAccent::Mint, " Nextcloud endpoint ");
        if inner.height < 5 {
            return;
        }

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Length(2),
                Constraint::Min(1),
            ])
            .split(inner);

        let intro = Paragraph::new(vec![
            Line::from(Span::styled(
                "  enter the URL of your Nextcloud server.",
                theme.dim(),
            )),
            Line::from(Span::styled(
                "  e.g. https://cloud.example.org  ·  http://localhost:8080",
                theme.dim_bright(),
            )),
        ]);
        ratatui::widgets::Widget::render(intro, rows[0], buf);

        form_field::render(rows[1], buf, theme, "server URL", input, true);

        let tag_area = Rect::new(rows[2].x + 2, rows[2].y, rows[2].width.saturating_sub(2), 1);
        if input.is_empty() {
            tag::render(tag_area, buf, theme, TagKind::Busy, "type a URL");
        } else if valid {
            tag::render(tag_area, buf, theme, TagKind::Ok, "URL parses");
        } else {
            tag::render(
                tag_area,
                buf,
                theme,
                TagKind::Err,
                "not a URL (need http:// or https://)",
            );
        }
    }
}
