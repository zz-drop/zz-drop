use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::Theme;

/// Inline button row: `[c · copy]  [q · qr]  [o · open]`. Each button
/// is a square-bracketed chip in `accent` (or `dim` when disabled).
pub fn render_row(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    btns: &[(&str, &str, bool)],
) {
    if area.width < 4 || area.height < 1 {
        return;
    }
    let mut spans: Vec<Span<'_>> = Vec::with_capacity(btns.len() * 2);
    for (i, (key, label, disabled)) in btns.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", theme.dim()));
        }
        let style = if *disabled {
            theme.dim()
        } else {
            theme.accent()
        };
        spans.push(Span::styled(format!("[{key} · {label}]"), style));
    }
    let p = Paragraph::new(Line::from(spans));
    ratatui::widgets::Widget::render(p, area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::MockEnv;

    fn t() -> Theme {
        Theme::from_parts(&MockEnv::empty(), false)
    }

    #[test]
    fn renders_three_buttons() {
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        render_row(
            area,
            &mut buf,
            &t(),
            &[
                ("c", "copy", false),
                ("q", "qr", false),
                ("o", "open", false),
            ],
        );
        let mut s = String::new();
        for x in 0..60 {
            s.push_str(buf[(x, 0)].symbol());
        }
        assert!(s.contains("[c · copy]"), "got `{s}`");
        assert!(s.contains("[q · qr]"));
        assert!(s.contains("[o · open]"));
    }
}
