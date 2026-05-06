//! Render snapshot for the post-setup `DoneScreen`. Locks two
//! invariants:
//!
//! - the operator gets the daily-use cheat sheet (`zz x`,
//!   `zz file.md`, ...);
//! - the optional shell-completion install hint shows the three
//!   one-liners (bash / zsh / fish) so the user can copy-paste
//!   without hunting the docs.
//!
//! The test renders into a synthetic 100×30 buffer (the design
//! target size) and inspects the resulting cell text. It does
//! not depend on terminal colour: comparisons go against the
//! string content, the theme decides only the styling.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use zz_drop_tui::screens::done::DoneScreen;
use zz_drop_tui::theme::Theme;

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

fn render_local_only_done() -> String {
    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);
    let theme = Theme::detect();
    DoneScreen::render(
        area,
        &mut buf,
        &theme,
        Some("/tmp/zz-drop-test/profiles-local.zz"),
        None,
        "cloud.example.org",
        None,
    );
    flatten(&buf)
}

#[test]
fn done_screen_lists_the_daily_cli_recap() {
    let s = render_local_only_done();
    for needle in ["zz x", "zz file.md", "zz d file.md", "zz q"] {
        assert!(
            s.contains(needle),
            "daily recap missing `{needle}` — render was:\n{s}"
        );
    }
}

#[test]
fn done_screen_offers_shell_completion_install_lines() {
    let s = render_local_only_done();
    // The three one-liners must surface verbatim so the
    // operator can copy-paste them; matching on the verb +
    // shell name guards against any future restyling.
    assert!(
        s.contains("zz --completions bash"),
        "missing bash install line:\n{s}"
    );
    assert!(
        s.contains("zz --completions zsh"),
        "missing zsh install line:\n{s}"
    );
    assert!(
        s.contains("zz --completions fish"),
        "missing fish install line:\n{s}"
    );
    // The section is labelled "optional" so the operator
    // understands it isn't required to finish setup.
    assert!(s.contains("optional"), "completion section not flagged optional:\n{s}");
}

#[test]
fn done_screen_does_not_leak_zz_drop_net_or_real_emails() {
    // Privacy lock per design §12 — the screen is shown to
    // every operator at the end of setup. Real-looking hosts,
    // emails, OAuth-shaped strings must never appear in the
    // hard-coded copy. The bare word "passphrase" is allowed:
    // the local-only branch warns the operator that losing
    // the passphrase is unrecoverable, which is part of the
    // expected UX.
    let s = render_local_only_done();
    assert!(!s.contains("zz-drop.net"));
    assert!(!s.contains("@example.com"));
    assert!(!s.contains("@example.org"));
    assert!(!s.to_lowercase().contains("bearer"));
    assert!(!s.contains("access_token"));
    assert!(!s.contains("refresh_token"));
}
