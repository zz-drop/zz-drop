use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::Theme;

#[derive(Clone, Copy, Debug)]
pub struct KeyHint {
    pub key: &'static str,
    pub label: &'static str,
}

impl KeyHint {
    pub const fn new(key: &'static str, label: &'static str) -> Self {
        Self { key, label }
    }
}

pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme, items: &[KeyHint]) {
    if area.width < 4 || area.height < 1 {
        return;
    }
    let chip = theme.keybar_chip();
    let label = theme.keybar_label();

    let mut spans: Vec<Span<'_>> = Vec::with_capacity(items.len() * 3);
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("   ", label));
        }
        spans.push(Span::styled(format!(" {} ", item.key), chip));
        spans.push(Span::styled(format!(" {}", item.label), label));
    }
    let line = Line::from(spans);
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

    #[test]
    fn renders_chip_and_label() {
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        let items = [KeyHint::new("n", "next"), KeyHint::new("q", "quit")];
        render(area, &mut buf, &t(), &items);
        let mut s = String::new();
        for x in 0..60 {
            s.push_str(buf[(x, 0)].symbol());
        }
        assert!(s.contains(" n "), "key chip missing in `{s}`");
        assert!(s.contains("next"));
        assert!(s.contains(" q "));
        assert!(s.contains("quit"));
    }

    #[test]
    fn empty_items_renders_nothing() {
        let area = Rect::new(0, 0, 30, 1);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &t(), &[]);
        // Should not panic
    }
}
