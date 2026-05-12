//! Smoke render of the Provider screen.
//! Guards the per-provider contextual hint row: when OneDrive is
//! the focused option, a warning about strict-business-tenant
//! consent + the BYO client_id workaround must appear. Other
//! providers must not show the warning.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use zz_drop_tui::screens::provider::ProviderScreen;
use zz_drop_tui::theme::Theme;
use zz_drop_tui::wizard::ProviderKind;

fn flatten(buf: &Buffer) -> String {
    let mut out = String::new();
    let area = *buf.area();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(area.x + x, area.y + y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn render(selected: ProviderKind) -> String {
    let area = Rect::new(0, 0, 120, 16);
    let mut buf = Buffer::empty(area);
    let theme = Theme::detect();
    ProviderScreen::render(area, &mut buf, &theme, selected);
    flatten(&buf)
}

#[test]
fn onedrive_focus_shows_publisher_warning() {
    let s = render(ProviderKind::OneDrive);
    assert!(s.contains("OneDrive"), "OneDrive row missing:\n{s}");
    assert!(
        s.contains("own client_id"),
        "OneDrive warning hint missing when OneDrive is focused:\n{s}"
    );
    assert!(
        s.contains("docs/onedrive.md"),
        "OneDrive doc pointer missing:\n{s}"
    );
}

#[test]
fn nextcloud_focus_hides_onedrive_warning() {
    let s = render(ProviderKind::Nextcloud);
    assert!(
        !s.contains("own client_id"),
        "OneDrive warning leaked into Nextcloud focus:\n{s}"
    );
}

#[test]
fn google_drive_focus_hides_onedrive_warning() {
    let s = render(ProviderKind::GoogleDrive);
    assert!(
        !s.contains("own client_id"),
        "OneDrive warning leaked into Google Drive focus:\n{s}"
    );
}

#[test]
fn dropbox_focus_hides_onedrive_warning() {
    let s = render(ProviderKind::Dropbox);
    assert!(
        !s.contains("own client_id"),
        "OneDrive warning leaked into Dropbox focus:\n{s}"
    );
}
