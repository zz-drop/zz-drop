use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::Theme;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentPill {
    /// No profile is active in this TUI session.
    NoProfile,
    /// `profile-local.zz` is the active profile (decrypted in RAM
    /// and/or just saved by the wizard in CreateLocal mode). The
    /// optional alias is the profile's own `alias` field — empty
    /// during early wizard screens before username is set.
    Local { alias: String },
    /// `profile-remote.zz` is the active profile. Carries the server
    /// label so the chip can show `remote · zz-drop.net · casa-nc`.
    Remote { server: String, alias: String },
    #[allow(dead_code)] // TASK 20+
    Locked,
    #[allow(dead_code)] // TASK 20+
    Running,
}

impl AgentPill {
    fn parts(&self, colored: bool) -> (char, String) {
        let glyph_active = if colored { '●' } else { '*' };
        let glyph_idle = if colored { '○' } else { 'o' };
        let with_alias = |head: String, alias: &str| -> String {
            if alias.is_empty() {
                head
            } else {
                format!("{head} · {alias}")
            }
        };
        match self {
            Self::NoProfile => (glyph_idle, "no profile".into()),
            Self::Local { alias } => (glyph_active, with_alias("local".into(), alias)),
            Self::Remote { server, alias } => {
                (glyph_active, with_alias(format!("remote · {server}"), alias))
            }
            Self::Locked => (glyph_active, "locked".into()),
            Self::Running => (glyph_active, "running".into()),
        }
    }

    fn chip_style(&self, theme: &Theme) -> ratatui::style::Style {
        match self {
            Self::NoProfile => theme.pill_idle(),
            Self::Local { .. } => theme.pill_ready(),
            Self::Remote { .. } => theme.pill_ready(),
            Self::Locked => theme.pill_warn(),
            Self::Running => theme.pill_running(),
        }
    }
}

/// Render a pill chip right-aligned at `area.right()`. Returns the
/// number of columns drawn so the title bar can compose the rest.
///
/// The chip is rendered as `█ ● label █` with a coloured background
/// filling all cells, mimicking a CSS `background-color` pill.
pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme, pill: &AgentPill) -> u16 {
    if area.width < 6 || area.height < 1 {
        return 0;
    }
    let (glyph, label) = pill.parts(theme.colored);
    let style = pill.chip_style(theme);
    // Two-cell padding on each side so the chip reads as a chip even on
    // dark terminals where `bg` is subtle. ` ● label ` totals
    // `1 + 1 + 1 + label.len() + 1 = label.len() + 4`.
    let inner = format!("  {glyph} {label}  ");
    let width = inner.chars().count() as u16;
    let width = width.min(area.width);
    let x = area.right().saturating_sub(width);
    let pill_area = Rect::new(x, area.y, width, 1);

    let line = Line::from(Span::styled(inner, style));
    let p = Paragraph::new(line);
    ratatui::widgets::Widget::render(p, pill_area, buf);
    width
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::MockEnv;

    fn t() -> Theme {
        Theme::from_parts(&MockEnv::empty(), false)
    }

    #[test]
    fn renders_no_profile_pill_right_aligned() {
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        let w = render(area, &mut buf, &t(), &AgentPill::NoProfile);
        assert!(w > 0);
        let mut s = String::new();
        for x in 0..60 {
            s.push_str(buf[(x, 0)].symbol());
        }
        // pill should be at the right end (with right-pad)
        assert!(s.ends_with("no profile  "), "got `{s}`");
        // and contain the glyph
        assert!(s.contains("no profile"), "got `{s}`");
    }

    #[test]
    fn renders_remote_pill_with_server_and_alias() {
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        let pill = AgentPill::Remote {
            server: "zz-drop.net".into(),
            alias: "casa-nc".into(),
        };
        let w = render(area, &mut buf, &t(), &pill);
        assert!(w > 0);
        let mut s = String::new();
        for x in 0..80 {
            s.push_str(buf[(x, 0)].symbol());
        }
        assert!(s.contains("remote · zz-drop.net · casa-nc"), "got `{s}`");
    }

    #[test]
    fn renders_local_pill_with_alias() {
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        let pill = AgentPill::Local {
            alias: "lavoro".into(),
        };
        let w = render(area, &mut buf, &t(), &pill);
        assert!(w > 0);
        let mut s = String::new();
        for x in 0..80 {
            s.push_str(buf[(x, 0)].symbol());
        }
        assert!(s.contains("local · lavoro"), "got `{s}`");
    }

    #[test]
    fn renders_local_pill_without_alias_when_empty() {
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        let pill = AgentPill::Local { alias: String::new() };
        let _ = render(area, &mut buf, &t(), &pill);
        let mut s = String::new();
        for x in 0..60 {
            s.push_str(buf[(x, 0)].symbol());
        }
        // No trailing separator when alias is empty.
        assert!(!s.contains("local ·"), "got `{s}`");
        assert!(s.contains("local"), "got `{s}`");
    }

    #[test]
    fn returns_zero_on_tiny_area() {
        let area = Rect::new(0, 0, 3, 1);
        let mut buf = Buffer::empty(area);
        let w = render(
            area,
            &mut buf,
            &t(),
            &AgentPill::Local { alias: "x".into() },
        );
        assert_eq!(w, 0);
    }
}
