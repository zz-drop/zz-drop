use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{glyphs, pill};
use crate::theme::Theme;
use pill::AgentPill;

/// Render the top title bar:
/// `▍ zz-tui  ›  breadcrumb`            with the agent pill right-aligned.
pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme, breadcrumb: &str, agent: &AgentPill) {
    if area.width < 12 || area.height < 1 {
        return;
    }

    // Pill first so we know how much space to leave on the right.
    let pill_w = pill::render(area, buf, theme, agent);
    let left_room = area.width.saturating_sub(pill_w + 1);
    if left_room < 8 {
        return;
    }

    let bar = format!("{} ", glyphs::bar_left(theme.colored));
    let line = Line::from(vec![
        Span::styled(bar, theme.accent_bold()),
        Span::styled("zz-tui", theme.header()),
        Span::styled("  ›  ", theme.dim()),
        Span::styled(breadcrumb.to_string(), theme.header()),
    ]);

    let left_area = Rect::new(area.x, area.y, left_room, 1);
    let p = Paragraph::new(line);
    ratatui::widgets::Widget::render(p, left_area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::MockEnv;

    fn t() -> Theme {
        Theme::from_parts(&MockEnv::empty(), false)
    }

    #[test]
    fn renders_wordmark_and_breadcrumb() {
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &t(), "welcome", &AgentPill::NoProfile);
        let mut s = String::new();
        for x in 0..80 {
            s.push_str(buf[(x, 0)].symbol());
        }
        assert!(s.contains("zz-tui"), "got `{s}`");
        assert!(s.contains("welcome"));
        assert!(s.contains("no profile"));
    }
}
