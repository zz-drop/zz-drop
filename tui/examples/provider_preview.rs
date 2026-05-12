//! Renders the Provider picker screen in all four focus states and
//! prints them side-by-side to stdout. Lets you eyeball the
//! contextual hint row (currently only OneDrive carries one) without
//! launching the full TUI wizard. Run with:
//!
//! ```
//! cargo run --example provider_preview
//! ```

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
    let area = Rect::new(0, 0, 100, 14);
    let mut buf = Buffer::empty(area);
    let theme = Theme::detect();
    ProviderScreen::render(area, &mut buf, &theme, selected);
    flatten(&buf)
}

fn main() {
    for (label, kind) in [
        ("Nextcloud focused", ProviderKind::Nextcloud),
        ("Google Drive focused", ProviderKind::GoogleDrive),
        ("OneDrive focused (warning row should appear)", ProviderKind::OneDrive),
        ("Dropbox focused", ProviderKind::Dropbox),
    ] {
        println!("\n=== {label} ===");
        print!("{}", render(kind));
    }
}
