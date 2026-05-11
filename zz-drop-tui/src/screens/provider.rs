use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

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
        let inner = panel::open(area, buf, theme, PanelAccent::Mint, " Choose Provider ");
        if inner.height < 4 {
            return;
        }

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // [0] Nextcloud
                Constraint::Length(1), // [1] Google Drive
                Constraint::Length(1), // [2] OneDrive
                Constraint::Length(1), // [3] Dropbox
                Constraint::Length(1), // [4] Proton (disabled)
                Constraint::Length(1), // [5] spacer
                Constraint::Length(1), // [6] contextual hint (per-provider notes)
                Constraint::Min(0),
            ])
            .split(inner);

        let nc_focused = matches!(selected, ProviderKind::Nextcloud);
        let gd_focused = matches!(selected, ProviderKind::GoogleDrive);
        let od_focused = matches!(selected, ProviderKind::OneDrive);
        let db_focused = matches!(selected, ProviderKind::Dropbox);

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
            db_focused,
            db_focused,
            "Dropbox · OAuth",
            Some("paste-code · App folder"),
            false,
        );
        radio::render_row(
            rows[4],
            buf,
            theme,
            false,
            false,
            "Proton Drive",
            Some("planned · pending official CLI/SDK"),
            true,
        );

        // Per-provider contextual hint. Currently only OneDrive
        // carries one: the upstream default `client_id` is not
        // publisher-verified, so strict-policy business tenants
        // hard-block consent and must rebuild with their own
        // client_id (`ZZ_DROP_ONEDRIVE_CLIENT_ID`). See
        // `zz-drop/docs/onedrive.md`.
        if matches!(selected, ProviderKind::OneDrive) && rows[6].height >= 1 {
            let warning = Paragraph::new(Line::from(vec![
                Span::styled("  ⚠  ", theme.warn()),
                Span::styled(
                    "strict business tenants: own client_id needed — see docs/onedrive.md",
                    theme.dim(),
                ),
            ]));
            warning.render(rows[6], buf);
        }
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
