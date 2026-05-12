use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, nav_item, panel};
use crate::wizard::WelcomeItem;

pub struct WelcomeScreen;

impl WelcomeScreen {
    pub fn title() -> &'static str {
        "welcome"
    }

    pub fn keybar_hint() -> Vec<KeyHint> {
        vec![
            KeyHint::new("↑↓", "select"),
            KeyHint::new("↵", "confirm"),
            KeyHint::new("q", "quit"),
        ]
    }

    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        focused: WelcomeItem,
        config_dir_display: &str,
        server_label: &str,
        local_exists: bool,
        remote_exists: bool,
    ) {
        // Layout: tagline · LOCAL block · separator · REMOTE block ·
        // separator · Quit. Each block has a section header (1 row),
        // an Open row when the file exists (1 row), and one or two
        // action rows. One blank row between blocks.
        // The REMOTE block is gated by the `remote` feature; default
        // builds collapse the layout to LOCAL + Quit only.
        let local_rows: u16 = 1 + (local_exists as u16) + 1; // header + (open?) + create
        #[cfg(feature = "remote")]
        let remote_rows: u16 = 1 + (remote_exists as u16) + 2; // header + (open?) + create-remote + signin
        #[cfg(feature = "remote")]
        let row_count: u16 = local_rows + 1 + remote_rows + 1 + 1; // + sep(local→remote) + sep(remote→quit) + Quit
        #[cfg(not(feature = "remote"))]
        let row_count: u16 = local_rows + 1 + 1; // + sep + Quit
        // Avoid unused-variable noise in the default build.
        #[cfg(not(feature = "remote"))]
        let _ = (remote_exists, server_label);
        let panel_height: u16 = row_count + 3; // title strip + bottom border + a breath line

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Length(panel_height),
                Constraint::Min(0),
                Constraint::Length(1), // footnote
                Constraint::Length(2), // bottom margin: raises the footnote
            ])
            .split(area);

        // tagline — centred horizontally
        let tagline = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("zz-drop", theme.accent_bold()),
                Span::styled("  ·  setup", theme.dim()),
            ]),
            Line::from(Span::styled(
                "Multi-cloud file transfer CLI",
                theme.body(),
            )),
        ])
        .alignment(Alignment::Center);
        ratatui::widgets::Widget::render(tagline, chunks[0], buf);

        // " › get started " panel — split into LOCAL and REMOTE
        // sub-blocks separated by a blank row, each with its own
        // section header.
        let inner = panel::open(chunks[1], buf, theme, PanelAccent::Mint, " › get started ");
        // Skip the menu only when the panel is actually too short
        // for the rows it must hold. The previous hard-coded `>= 5`
        // produced an empty panel in default (no `remote` feature)
        // builds without an existing container, where row_count = 4.
        if inner.height >= row_count {
            let mut constraints: Vec<Constraint> =
                (0..row_count).map(|_| Constraint::Length(1)).collect();
            constraints.push(Constraint::Min(0));
            let menu = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(inner);

            let mut row = 0usize;

            // ─── LOCAL section ────────────────────────────────────
            render_section_header(
                menu[row],
                buf,
                theme,
                "LOCAL",
                "this machine only · no recovery if lost",
            );
            row += 1;
            if local_exists {
                nav_item::render_row(
                    menu[row],
                    buf,
                    theme,
                    focused == WelcomeItem::OpenLocal,
                    "Open local container",
                    Some("profiles-local.zz on this machine"),
                    false,
                );
                row += 1;
            }
            let (configure_label, configure_hint): (&str, String) = if local_exists {
                (
                    "Add profile to local container",
                    "appends a new connection to profiles-local.zz".to_string(),
                )
            } else {
                (
                    "Create local container",
                    "new profiles-local.zz · pick a provider".to_string(),
                )
            };
            nav_item::render_row(
                menu[row],
                buf,
                theme,
                focused == WelcomeItem::Configure,
                configure_label,
                Some(&configure_hint),
                false,
            );
            row += 1;

            // ─── separator ────────────────────────────────────────
            row += 1;

            // ─── REMOTE section (gated) ───────────────────────────
            #[cfg(feature = "remote")]
            {
                let remote_subtitle =
                    format!("synced with {server_label} · recoverable from any shell");
                render_section_header(menu[row], buf, theme, "REMOTE", &remote_subtitle);
                row += 1;
                if remote_exists {
                    let hint = format!("profiles-remote.zz · cached from {server_label}");
                    nav_item::render_row(
                        menu[row],
                        buf,
                        theme,
                        focused == WelcomeItem::OpenRemote,
                        "Open synced container",
                        Some(&hint),
                        false,
                    );
                    row += 1;
                }
                let configure_remote_hint =
                    format!("appends a new inner profile to {server_label}");
                nav_item::render_row(
                    menu[row],
                    buf,
                    theme,
                    focused == WelcomeItem::ConfigureRemote,
                    "Add to remote container",
                    Some(&configure_remote_hint),
                    false,
                );
                row += 1;
                let (signin_label, signin_hint): (String, String) = if remote_exists {
                    (
                        format!("Fetch the latest container from {server_label}"),
                        "overwrites profiles-remote.zz".to_string(),
                    )
                } else {
                    (
                        format!("Sign in to {server_label}"),
                        "fetch the account's container into profiles-remote.zz".to_string(),
                    )
                };
                nav_item::render_row(
                    menu[row],
                    buf,
                    theme,
                    focused == WelcomeItem::SignIn,
                    &signin_label,
                    Some(&signin_hint),
                    false,
                );
                row += 1;

                // ─── separator ───────────────────────────────────
                row += 1;
            }

            // ─── Quit ─────────────────────────────────────────────
            nav_item::render_row(
                menu[row],
                buf,
                theme,
                focused == WelcomeItem::Quit,
                "Quit",
                None,
                false,
            );
        }

        // footnote — centred too, to match the tagline
        let footnote = Paragraph::new(Line::from(vec![
            Span::styled("profiles live in ", theme.dim()),
            Span::styled(config_dir_display.to_string(), theme.cyan()),
            Span::styled(". encrypted at rest.", theme.dim()),
        ]))
        .alignment(Alignment::Center);
        ratatui::widgets::Widget::render(footnote, chunks[3], buf);
    }
}

/// Section header: `▍ LABEL    subtitle` rendered in mint accent +
/// dim subtitle, on a single row inside the welcome panel.
fn render_section_header(area: Rect, buf: &mut Buffer, theme: &Theme, label: &str, subtitle: &str) {
    if area.width < 4 || area.height < 1 {
        return;
    }
    let line = Line::from(vec![
        Span::styled(" ▍ ", theme.accent_bold()),
        Span::styled(label.to_string(), theme.accent_bold()),
        Span::styled(format!("   {subtitle}"), theme.dim()),
    ]);
    let p = Paragraph::new(line);
    ratatui::widgets::Widget::render(p, area, buf);
}
