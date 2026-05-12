use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::ProfileSource;
use crate::input::TextInput;
use crate::theme::{PanelAccent, Theme};
use crate::tui_widgets::{KeyHint, form_field, panel};
use crate::wizard::ManageStage;

pub struct ProfileUnlockScreen;

impl ProfileUnlockScreen {
    pub fn title() -> &'static str {
        "open profile"
    }

    pub fn keybar_hint(stage: &ManageStage) -> Vec<KeyHint> {
        match stage {
            ManageStage::Unlocking => vec![KeyHint::new("…", "decrypting")],
            _ => vec![
                KeyHint::new("↵", "unlock"),
                KeyHint::new("esc", "back"),
            ],
        }
    }

    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        profile_path: &str,
        passphrase: &TextInput,
        stage: &ManageStage,
        last_error: Option<&str>,
        source: ProfileSource,
    ) {
        let title = match source {
            ProfileSource::Local => " open local profile ",
            ProfileSource::Remote => " open synced profile ",
        };
        let inner = panel::open(area, buf, theme, PanelAccent::Mint, title);
        if inner.height < 7 {
            return;
        }
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Length(2),
                Constraint::Min(1),
            ])
            .split(inner);

        let intro = match source {
            ProfileSource::Local => {
                "  enter the passphrase to decrypt the local-only profiles-local.zz."
            }
            ProfileSource::Remote => {
                "  enter the passphrase you used when this alias was uploaded."
            }
        };
        let header = Paragraph::new(vec![
            Line::from(Span::styled(intro, theme.dim())),
            Line::from(vec![
                Span::styled("  file: ", theme.dim()),
                Span::styled(profile_path.to_string(), theme.cyan()),
            ]),
        ]);
        ratatui::widgets::Widget::render(header, rows[0], buf);

        form_field::render(rows[1], buf, theme, "passphrase", passphrase, true);

        let status = match (stage, last_error) {
            (ManageStage::Unlocking, _) => Line::from(Span::styled(
                "  …  decrypting (Argon2id, ~1 s)",
                theme.cyan(),
            )),
            (_, Some(err)) => Line::from(Span::styled(format!("  ✗ {err}"), theme.danger())),
            _ => Line::from(Span::styled(
                "  the passphrase is typed locally and never leaves this machine.",
                theme.dim(),
            )),
        };
        ratatui::widgets::Widget::render(Paragraph::new(status), rows[2], buf);
    }
}
