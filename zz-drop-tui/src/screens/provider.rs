use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, panel, radio};
use crate::wizard::ProviderKind;

pub struct ProviderScreen;

impl ProviderScreen {
    pub fn title() -> &'static str {
        "provider"
    }

    pub fn keybar_hint() -> Vec<KeyHint> {
        vec![
            KeyHint::new("↑↓", "select"),
            KeyHint::new("↵", "continue"),
            KeyHint::new("esc", "back"),
        ]
    }

    pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme, selected: ProviderKind) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)])
            .split(area);

        let intro = Paragraph::new(Line::from(Span::styled(
            "Choose a provider",
            theme.header(),
        )));
        ratatui::widgets::Widget::render(intro, chunks[0], buf);

        let inner = panel::open(chunks[1], buf, theme, PanelAccent::Mint, " provider ");
        if inner.height < 4 {
            return;
        }

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner);

        let nc_focused = matches!(selected, ProviderKind::Nextcloud);
        let gd_focused = matches!(selected, ProviderKind::GoogleDrive);
        let od_focused = matches!(selected, ProviderKind::OneDrive);

        radio::render_row(
            rows[0],
            buf,
            theme,
            nc_focused,
            nc_focused,
            "Nextcloud · WebDAV",
            Some("self-hosted or managed · login flow / app password"),
            false,
        );
        radio::render_row(
            rows[1],
            buf,
            theme,
            gd_focused,
            gd_focused,
            "Google Drive · OAuth",
            Some("device flow · scope drive.file"),
            false,
        );
        radio::render_row(
            rows[2],
            buf,
            theme,
            od_focused,
            od_focused,
            "OneDrive · OAuth",
            Some("device flow · Microsoft Graph"),
            false,
        );
        radio::render_row(
            rows[3],
            buf,
            theme,
            false,
            false,
            "Proton Drive",
            Some("planned · pending official CLI/SDK"),
            true,
        );
        radio::render_row(
            rows[4],
            buf,
            theme,
            false,
            false,
            "S3 / R2",
            Some("planned · object storage"),
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keybar_hint_includes_navigation() {
        let hints = ProviderScreen::keybar_hint();
        assert!(hints.iter().any(|k| k.label.contains("select")));
        assert!(hints.iter().any(|k| k.label.contains("continue")));
    }
}
