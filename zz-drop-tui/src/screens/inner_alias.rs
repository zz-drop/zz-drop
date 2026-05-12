//! `Screen::InnerAlias` — alias prompt for the "add new connection"
//! sub-flow. Reached after the operator has finished setting up a
//! new provider inside an already-unlocked container; the screen
//! collects only the alias name and confirms the add. The
//! container's KEK is cached, so no passphrase prompt happens here.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::input::TextInput;
use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, form_field, panel};

pub struct InnerAliasScreen;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InnerAliasState {
    Editing,
    Saving,
    Failed,
}

impl InnerAliasScreen {
    pub fn title() -> &'static str {
        "name this connection"
    }

    pub fn keybar_hint(state: InnerAliasState) -> Vec<KeyHint> {
        match state {
            InnerAliasState::Editing => vec![
                KeyHint::new("tab", "new suggestion"),
                KeyHint::new("↵", "add to container"),
                KeyHint::new("esc", "cancel"),
            ],
            InnerAliasState::Saving => vec![KeyHint::new("…", "saving")],
            InnerAliasState::Failed => vec![
                KeyHint::new("tab", "new suggestion"),
                KeyHint::new("↵", "retry"),
                KeyHint::new("esc", "cancel"),
            ],
        }
    }

    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        input: &TextInput,
        state: InnerAliasState,
        error: Option<&str>,
    ) {
        let inner = panel::open(area, buf, theme, PanelAccent::Mint, " new connection alias ");
        if inner.height < 8 {
            return;
        }
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(inner);

        let intro = Paragraph::new(vec![
            Line::from(Span::styled(
                "  pick a short mnemonic for this connection.",
                theme.dim(),
            )),
            Line::from(Span::styled(
                "  avoid passwords or anything sensitive — aliases are visible in the picker.",
                theme.dim(),
            )),
        ]);
        ratatui::widgets::Widget::render(intro, chunks[0], buf);

        // Bordered field with a visible cursor at the end of the
        // input value, scrolling horizontally on long values.
        form_field::render(chunks[1], buf, theme, "alias", input, true);

        // Status line.
        match state {
            InnerAliasState::Saving => {
                let p = Paragraph::new(Line::from(Span::styled(
                    "  …  appending to container",
                    theme.cyan(),
                )));
                ratatui::widgets::Widget::render(p, chunks[2], buf);
            }
            InnerAliasState::Failed => {
                let msg = error.unwrap_or("save failed");
                let p = Paragraph::new(Line::from(vec![
                    Span::styled("  ✗ ", theme.danger()),
                    Span::styled(msg.to_string(), theme.danger()),
                ]));
                ratatui::widgets::Widget::render(p, chunks[2], buf);
            }
            InnerAliasState::Editing => {
                let p = Paragraph::new(Line::from(Span::styled(
                    "  edit the suggestion or press tab for a new one.",
                    theme.dim(),
                )));
                ratatui::widgets::Widget::render(p, chunks[2], buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keybar_changes_per_state() {
        let editing = InnerAliasScreen::keybar_hint(InnerAliasState::Editing);
        assert!(editing.iter().any(|k| k.label.contains("add")));
        let saving = InnerAliasScreen::keybar_hint(InnerAliasState::Saving);
        assert!(saving.iter().any(|k| k.label.contains("saving")));
        let failed = InnerAliasScreen::keybar_hint(InnerAliasState::Failed);
        assert!(failed.iter().any(|k| k.label.contains("retry")));
    }
}
