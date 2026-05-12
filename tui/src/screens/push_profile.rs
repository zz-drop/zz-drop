use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::input::TextInput;
use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, form_field, panel};
use crate::wizard::{PushFlowMode, PushFlowState, PushStage};

pub struct PushProfileScreen;

impl PushProfileScreen {
    pub fn title() -> &'static str {
        "push profile"
    }

    pub fn keybar_hint(stage: &PushStage, mode: PushFlowMode, can_back: bool) -> Vec<KeyHint> {
        let (action_verb, sending_verb, fetching_verb) = match mode {
            PushFlowMode::Push => ("push", "uploading", "fetching aliases"),
            PushFlowMode::SignIn => ("download", "downloading", "listing aliases"),
        };
        match stage {
            PushStage::PushFetching => vec![KeyHint::new("…", fetching_verb)],
            PushStage::PushSending => vec![KeyHint::new("…", sending_verb)],
            PushStage::Done => vec![KeyHint::new("↵", "continue")],
            PushStage::Failed(_) => vec![
                KeyHint::new("r", "retry"),
                KeyHint::new("esc", "back"),
            ],
            _ => {
                let mut hints = vec![KeyHint::new("↑↓", "pick alias")];
                if matches!(mode, PushFlowMode::Push) {
                    hints.push(KeyHint::new("g", "suggest"));
                }
                hints.push(KeyHint::new("↵", action_verb));
                if can_back {
                    hints.push(KeyHint::new("esc", "cancel"));
                }
                hints
            }
        }
    }

    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        state: &PushFlowState,
        new_alias: &TextInput,
        api_base: &str,
    ) {
        if matches!(state.stage, PushStage::Done) {
            render_done(area, buf, theme, state, api_base);
            return;
        }
        if let PushStage::Failed(reason) = &state.stage {
            render_failed(area, buf, theme, reason, state.mode);
            return;
        }
        let title = match state.mode {
            PushFlowMode::Push => " choose an alias on the server ",
            PushFlowMode::SignIn => " pick an alias to download ",
        };
        let inner = panel::open(area, buf, theme, PanelAccent::Mint, title);
        if inner.height < 8 {
            return;
        }
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(3),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(inner);

        let header = if matches!(state.stage, PushStage::PushFetching) {
            Paragraph::new(Line::from(Span::styled(
                match state.mode {
                    PushFlowMode::Push => "  …  fetching your aliases from the server",
                    PushFlowMode::SignIn => "  …  listing aliases for this account",
                },
                theme.cyan(),
            )))
        } else {
            Paragraph::new(match state.mode {
                PushFlowMode::Push => vec![
                    Line::from(Span::styled(
                        "  pick one of your existing aliases or type a new one.",
                        theme.dim(),
                    )),
                    Line::from(Span::styled(
                        "  the same blob is uploaded as-is — the server never sees the passphrase.",
                        theme.dim(),
                    )),
                ],
                PushFlowMode::SignIn => vec![
                    Line::from(Span::styled(
                        "  pick the alias you want to download into this machine.",
                        theme.dim(),
                    )),
                    Line::from(Span::styled(
                        "  the encrypted blob is saved as profiles-remote.zz; you'll unlock it next.",
                        theme.dim(),
                    )),
                ],
            })
        };
        ratatui::widgets::Widget::render(header, rows[0], buf);

        // List of existing aliases.
        if state.remote_aliases.is_empty()
            && !matches!(state.stage, PushStage::PushFetching)
        {
            let msg = match state.mode {
                PushFlowMode::Push => "  (no aliases yet — type a new one below)",
                PushFlowMode::SignIn => "  (this account has no aliases on the server yet)",
            };
            let p = Paragraph::new(Line::from(Span::styled(msg, theme.dim())));
            ratatui::widgets::Widget::render(p, rows[1], buf);
        } else {
            let mut lines: Vec<Line<'_>> = Vec::with_capacity(state.remote_aliases.len());
            for (i, alias) in state.remote_aliases.iter().enumerate() {
                let selected = state.picker_index == Some(i);
                let prefix = if selected { "> " } else { "  " };
                let style = if selected {
                    theme.accent_bold()
                } else {
                    theme.body()
                };
                lines.push(Line::from(Span::styled(
                    format!("{prefix}{alias}"),
                    style,
                )));
            }
            ratatui::widgets::Widget::render(Paragraph::new(lines), rows[1], buf);
        }

        // SignIn mode hides the "type new alias" field — you can only
        // download aliases that exist on the server.
        if matches!(state.mode, PushFlowMode::Push) {
            let new_alias_focused = state.picker_index.is_none();
            form_field::render(rows[2], buf, theme, "new alias", new_alias, new_alias_focused);
        }

        // Only the in-flight indicator goes here — keyboard shortcuts
        // are covered by the standard keybar at the bottom of the
        // window so we don't duplicate them inside the panel.
        if matches!(state.stage, PushStage::PushSending) {
            let line = match state.mode {
                PushFlowMode::Push => {
                    Line::from(Span::styled("  …  uploading blob", theme.cyan()))
                }
                PushFlowMode::SignIn => {
                    Line::from(Span::styled("  …  downloading blob", theme.cyan()))
                }
            };
            ratatui::widgets::Widget::render(Paragraph::new(line), rows[3], buf);
        }
    }
}

fn render_done(area: Rect, buf: &mut Buffer, theme: &Theme, state: &PushFlowState, api_base: &str) {
    let inner = panel::open(area, buf, theme, PanelAccent::Mint, " profile pushed ");
    if inner.height < 4 {
        return;
    }
    let alias = state.pushed_alias.as_deref().unwrap_or("");
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ✓ pushed to ", theme.ok()),
            Span::styled(api_base.to_string(), theme.cyan()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  alias: ", theme.dim()),
            Span::styled(alias.to_string(), theme.accent_bold()),
        ]),
        Line::from(vec![
            Span::styled("  size:  ", theme.dim()),
            Span::styled(format!("{} bytes", state.pushed_size), theme.body()),
        ]),
        Line::from(vec![
            Span::styled("  ver:   ", theme.dim()),
            Span::styled(format!("{}", state.pushed_version), theme.body()),
        ]),
        Line::from(""),
        Line::from(Span::styled("  press enter to continue.", theme.dim())),
    ]);
    ratatui::widgets::Widget::render(p, inner, buf);
}

fn render_failed(area: Rect, buf: &mut Buffer, theme: &Theme, reason: &str, mode: PushFlowMode) {
    let title = match mode {
        PushFlowMode::Push => " push failed ",
        PushFlowMode::SignIn => " download failed ",
    };
    let inner = panel::open(area, buf, theme, PanelAccent::Red, title);
    if inner.height < 4 {
        return;
    }
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(format!("  ✗ {reason}"), theme.danger())),
        Line::from(""),
        Line::from(vec![
            Span::styled("  press ", theme.dim()),
            Span::styled("r", theme.accent_bold()),
            Span::styled(" to retry, ", theme.dim()),
            Span::styled("esc", theme.accent_bold()),
            Span::styled(" to go back.", theme.dim()),
        ]),
    ]);
    ratatui::widgets::Widget::render(p, inner, buf);
}
