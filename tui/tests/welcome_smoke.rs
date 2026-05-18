//! Smoke render of the Welcome screen with the default state
//! (no container present, no remote feature). Quick visual diff
//! to confirm the page is non-empty after recent provider work.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use zz_drop_tui::screens::welcome::WelcomeScreen;
use zz_drop_tui::theme::Theme;
use zz_drop_tui::wizard::WelcomeItem;

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

#[test]
fn welcome_default_state_renders_content() {
    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);
    let theme = Theme::detect();
    WelcomeScreen::render(
        area,
        &mut buf,
        &theme,
        WelcomeItem::Configure,
        "/tmp/zz-drop-config",
        "zz-drop.net",
        false,
        false,
        None,
        None,
    );
    let s = flatten(&buf);
    assert!(s.contains("zz-drop"), "missing tagline:\n{s}");
    assert!(s.contains("setup"), "missing tagline subtitle:\n{s}");
    // Regression guard for "panel rendered empty" — the Welcome
    // page must always show at least the LOCAL section header,
    // the Create / Add action, and the Quit row, even on the
    // smallest default state (no container, `remote` feature
    // off).
    assert!(s.contains("LOCAL"), "missing LOCAL section:\n{s}");
    assert!(
        s.contains("Create local container") || s.contains("Add profile to local container"),
        "missing local action row:\n{s}"
    );
    assert!(s.contains("Quit"), "missing Quit row:\n{s}");
}

#[test]
fn welcome_with_existing_local_container_renders_open_row() {
    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);
    let theme = Theme::detect();
    WelcomeScreen::render(
        area,
        &mut buf,
        &theme,
        WelcomeItem::OpenLocal,
        "/tmp/zz-drop-config",
        "zz-drop.net",
        true,
        false,
        None,
        None,
    );
    let s = flatten(&buf);
    assert!(s.contains("Open local container"), "{s}");
    assert!(
        s.contains("Add profile to local container"),
        "missing add-mode label:\n{s}"
    );
}
