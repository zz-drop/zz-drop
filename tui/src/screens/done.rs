use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::PushedSummary;
use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, panel};

pub struct DoneScreen;

impl DoneScreen {
    pub fn title() -> &'static str {
        "done"
    }

    pub fn keybar_hint() -> Vec<KeyHint> {
        vec![
            KeyHint::new("↵", "back to welcome"),
            KeyHint::new("q", "exit"),
        ]
    }

    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        profile_path: Option<&str>,
        pushed: Option<&PushedSummary>,
        server_label: &str,
        add_inner_alias: Option<&str>,
    ) {
        // Add-inner-profile sub-flow gets its own short message:
        // the container was already at rest; we appended a single
        // inner profile and re-encrypted in place. No "no recovery"
        // warning, no daily-use cheat sheet — the operator already
        // knows them.
        if let Some(alias) = add_inner_alias {
            render_add_inner(area, buf, theme, alias, profile_path);
            return;
        }
        // Local-only profiles get a yellow panel ("⚠ no recovery");
        // the pushed case is the happy path (Mint).
        let accent = if pushed.is_some() {
            PanelAccent::Mint
        } else {
            PanelAccent::Yellow
        };
        let title = if pushed.is_some() {
            " done "
        } else {
            " local profile saved · no recovery "
        };
        let inner = panel::open(area, buf, theme, accent, title);
        if inner.height < 3 {
            return;
        }

        let mut lines = vec![Line::from("")];

        if let Some(p) = pushed {
            // The whole point of the wizard: this profile can now be
            // pulled from any other shell with `zz z <alias>`. Lead
            // with that.
            lines.push(Line::styled("  ✓ setup complete", theme.ok()));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    "  the profile is now retrievable from any shell with ",
                    theme.dim(),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("    zz z ", theme.cyan()),
                Span::styled(p.alias.clone(), theme.accent_bold()),
                Span::styled(
                    format!("    # {} bytes, version {}", p.blob_size, p.blob_version),
                    theme.dim(),
                ),
            ]));
        } else {
            // Degraded: profile is on disk but not pushed. Spell out
            // exactly what "no recovery" means so the operator can
            // make an informed call about whether to sign in to a
            // server before they walk away.
            lines.push(Line::styled(
                "  ⚠  profile saved locally — not on any server",
                theme.warn(),
            ));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  if you lose either",
                theme.body(),
            )),);
            lines.push(Line::from(vec![
                Span::styled("    • the file ", theme.body()),
                Span::styled("profiles-local.zz", theme.cyan()),
            ]));
            lines.push(Line::from(Span::styled(
                "    • the passphrase you just typed",
                theme.body(),
            )));
            lines.push(Line::from(vec![
                Span::styled("  the contents are gone — ", theme.body()),
                Span::styled("there is no recovery.", theme.warn()),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  to make it recoverable from any shell:",
                theme.dim(),
            )));
            lines.push(Line::from(vec![
                Span::styled("    • re-run ", theme.dim()),
                Span::styled("zz-tui", theme.cyan()),
                Span::styled(", choose ", theme.dim()),
                Span::styled("Open local profile", theme.accent_bold()),
                Span::styled(",", theme.dim()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("      then press p to push it to ", theme.dim()),
                Span::styled(server_label.to_string(), theme.cyan()),
                Span::styled(" under an alias.", theme.dim()),
            ]));
        }

        if let Some(p) = profile_path {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  profile at:  ", theme.dim()),
                Span::styled(p.to_string(), theme.cyan()),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::styled(
            "  daily use stays in the CLI:",
            theme.dim_bright(),
        ));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("    zz z", theme.cyan()),
            Span::styled("          # unlock the profile in the agent", theme.dim()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("    zz file.md", theme.cyan()),
            Span::styled("    # upload", theme.dim()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("    zz d file.md", theme.cyan()),
            Span::styled("  # download", theme.dim()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("    zz q", theme.cyan()),
            Span::styled("          # lock", theme.dim()),
        ]));
        lines.push(Line::from(""));
        // Optional one-off: install shell completion. Hint
        // shown verbatim so the operator can copy-paste; we
        // intentionally don't auto-write or shell-out from the
        // TUI ("mano sinistra": no extra prompts, no surprises).
        lines.push(Line::styled(
            "  optional · shell completion (one-off):",
            theme.dim_bright(),
        ));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("    zz --completions bash | source", theme.cyan()),
            Span::styled("                         # bash", theme.dim()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("    zz --completions zsh  > ~/.zfunc/_zz", theme.cyan()),
            Span::styled("                # zsh", theme.dim()),
        ]));
        lines.push(Line::from(vec![
            Span::styled(
                "    zz --completions fish > ~/.config/fish/completions/zz.fish",
                theme.cyan(),
            ),
            Span::styled(" # fish", theme.dim()),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  press ", theme.dim()),
            Span::styled("↵", theme.accent_bold()),
            Span::styled(" to go back, ", theme.dim()),
            Span::styled("q", theme.accent_bold()),
            Span::styled(" to exit.", theme.dim()),
        ]));

        let body = Paragraph::new(lines);
        let body_area =
            Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), inner.height);
        ratatui::widgets::Widget::render(body, body_area, buf);
    }
}

fn render_add_inner(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    alias: &str,
    container_path: Option<&str>,
) {
    let inner = panel::open(area, buf, theme, PanelAccent::Mint, " connection added ");
    if inner.height < 3 {
        return;
    }

    let mut lines = vec![Line::from("")];
    lines.push(Line::styled("  ✓ profile added to container", theme.ok()));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  alias  ", theme.dim()),
        Span::styled(alias.to_string(), theme.accent_bold()),
    ]));
    if let Some(p) = container_path {
        lines.push(Line::from(vec![
            Span::styled("  file   ", theme.dim()),
            Span::styled(p.to_string(), theme.cyan()),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  press ", theme.dim()),
        Span::styled("↵", theme.accent_bold()),
        Span::styled(" to return to welcome.", theme.dim()),
    ]));

    let body = Paragraph::new(lines);
    let body_area =
        Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), inner.height);
    ratatui::widgets::Widget::render(body, body_area, buf);
}
