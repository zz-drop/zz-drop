use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::glyphs;
use crate::theme::Theme;

/// Strength meter: ` █████░░░ label`. `score` and `max` are integer
/// counts; `bar_width` is the cell-count of the bar itself (label
/// rendered after).
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    score: u8,
    max: u8,
    bar_width: u16,
    label: &str,
) {
    if area.width < bar_width + 4 || area.height < 1 {
        return;
    }
    let max = max.max(1);
    let score = score.min(max);
    let filled = (score as u32 * bar_width as u32 / max as u32) as u16;
    let empty = bar_width.saturating_sub(filled);

    let fg_style = if score as u32 * 100 / max as u32 >= 60 {
        theme.ok()
    } else if score as u32 * 100 / max as u32 >= 30 {
        theme.warn()
    } else {
        theme.danger()
    };

    let full_glyph = glyphs::block_full(theme.colored);
    let empty_glyph = glyphs::block_light(theme.colored);

    let line = Line::from(vec![
        Span::styled(
            full_glyph.to_string().repeat(filled as usize),
            fg_style,
        ),
        Span::styled(
            empty_glyph.to_string().repeat(empty as usize),
            theme.dim(),
        ),
        Span::styled(format!("  {label}"), fg_style),
    ]);

    let p = Paragraph::new(line);
    ratatui::widgets::Widget::render(p, area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::MockEnv;

    fn t() -> Theme {
        Theme::from_parts(&MockEnv::empty(), false)
    }

    fn line(width: u16, score: u8, max: u8, bar_w: u16, label: &str) -> String {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &t(), score, max, bar_w, label);
        let mut s = String::new();
        for x in 0..width {
            s.push_str(buf[(x, 0)].symbol());
        }
        s.trim_end().to_string()
    }

    #[test]
    fn full_bar_at_max_score() {
        // ASCII fallback uses '#' for filled, '.' for empty
        let s = line(30, 4, 4, 8, "strong");
        assert!(s.starts_with("########"), "got `{s}`");
        assert!(s.contains("strong"));
    }

    #[test]
    fn half_bar_at_mid_score() {
        let s = line(30, 2, 4, 8, "fair");
        assert!(s.starts_with("####...."), "got `{s}`");
    }

    #[test]
    fn empty_bar_at_zero() {
        let s = line(30, 0, 4, 8, "empty");
        assert!(s.starts_with("........"), "got `{s}`");
    }
}
