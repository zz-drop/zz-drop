use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, panel, radio};
use crate::wizard::CollisionChoice;

pub struct CollisionScreen;

impl CollisionScreen {
    pub fn title() -> &'static str {
        "collision policy"
    }

    pub fn keybar_hint() -> Vec<KeyHint> {
        vec![
            KeyHint::new("↑↓", "choose"),
            KeyHint::new("↵", "continue"),
            KeyHint::new("esc", "back"),
        ]
    }

    pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme, current: CollisionChoice) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(8), Constraint::Min(1)])
            .split(area);

        // ── upper panel: choices ─────────────────────────────────
        let inner = panel::open(
            chunks[0],
            buf,
            theme,
            PanelAccent::Mint,
            " when a remote file with the same name exists ",
        );
        if inner.height >= 3 {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .split(inner);

            for (i, choice) in [
                CollisionChoice::Rename,
                CollisionChoice::Overwrite,
                CollisionChoice::Fail,
            ]
            .into_iter()
            .enumerate()
            {
                let focused = choice == current;
                let (label, hint) = match choice {
                    CollisionChoice::Rename => (
                        "Rename",
                        Some("appends ` (1)`, ` (2)`… · safe · default"),
                    ),
                    CollisionChoice::Overwrite => (
                        "Overwrite",
                        Some("replace remote · destructive"),
                    ),
                    CollisionChoice::Fail => {
                        ("Fail", Some("scriptable abort · for ci / pipelines"))
                    }
                };
                radio::render_row(rows[i], buf, theme, focused, focused, label, hint, false);
            }
        }

        // ── lower panel: preview ─────────────────────────────────
        let preview_inner =
            panel::open(chunks[1], buf, theme, PanelAccent::Dim, " preview ");
        if preview_inner.height >= 3 {
            let prev_rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .split(preview_inner);

            let cmd_line = Paragraph::new(Line::from(vec![
                Span::styled("  $ ", theme.dim()),
                Span::styled("zz readme.md", theme.body()),
            ]));
            ratatui::widgets::Widget::render(cmd_line, prev_rows[0], buf);

            let outcome_line = match current {
                CollisionChoice::Rename => Line::from(vec![
                    Span::styled("    ↻ rename ", theme.accent()),
                    Span::styled("→ /zz/readme (1).md", theme.dim()),
                ]),
                CollisionChoice::Overwrite => Line::from(vec![
                    Span::styled("    ! overwrite ", theme.warn()),
                    Span::styled("→ /zz/readme.md (replaces existing)", theme.dim()),
                ]),
                CollisionChoice::Fail => Line::from(vec![
                    Span::styled("    ✗ fail ", theme.danger()),
                    Span::styled("→ exit code 9, no upload performed", theme.dim()),
                ]),
            };
            let p = Paragraph::new(outcome_line);
            ratatui::widgets::Widget::render(p, prev_rows[1], buf);

            let extra = Paragraph::new(Line::from(Span::styled(
                "    ✓ uploaded · 2 KiB",
                theme.ok(),
            )));
            ratatui::widgets::Widget::render(extra, prev_rows[2], buf);
        }
    }
}
