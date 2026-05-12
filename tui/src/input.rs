use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::theme::Theme;

#[derive(Debug, Clone, Default)]
pub struct TextInput {
    value: String,
    masked: bool,
    cursor: usize,
}

impl TextInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn masked() -> Self {
        Self {
            masked: true,
            ..Self::default()
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn is_masked(&self) -> bool {
        self.masked
    }

    pub fn set_value(&mut self, s: &str) {
        self.value = s.to_string();
        self.cursor = self.value.chars().count();
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn push_char(&mut self, c: char) {
        let byte_idx = self.byte_idx_at_cursor();
        self.value.insert(byte_idx, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let new_cursor = self.cursor - 1;
        let from = self.byte_idx_at(new_cursor);
        let to = self.byte_idx_at(self.cursor);
        self.value.drain(from..to);
        self.cursor = new_cursor;
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, label: &str, theme: &Theme) {
        let display: String = if self.masked {
            "*".repeat(self.value.chars().count())
        } else {
            self.value.clone()
        };

        let line = Line::from(vec![
            Span::styled(format!("{label}: "), theme.dim()),
            Span::styled(display, Style::default()),
        ]);

        let paragraph = Paragraph::new(line).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(theme.dim()),
        );
        ratatui::widgets::Widget::render(paragraph, area, buf);
    }

    fn byte_idx_at(&self, char_idx: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.value.len())
    }

    fn byte_idx_at_cursor(&self) -> usize {
        self.byte_idx_at(self.cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_appends() {
        let mut t = TextInput::new();
        t.push_char('a');
        t.push_char('b');
        assert_eq!(t.value(), "ab");
    }

    #[test]
    fn backspace_removes_last() {
        let mut t = TextInput::new();
        t.push_char('a');
        t.push_char('b');
        t.backspace();
        assert_eq!(t.value(), "a");
    }

    #[test]
    fn backspace_on_empty_is_noop() {
        let mut t = TextInput::new();
        t.backspace();
        assert_eq!(t.value(), "");
    }

    #[test]
    fn handles_unicode() {
        let mut t = TextInput::new();
        t.push_char('日');
        t.push_char('本');
        t.backspace();
        assert_eq!(t.value(), "日");
    }

    #[test]
    fn masked_does_not_change_value() {
        let mut t = TextInput::masked();
        t.push_char('s');
        t.push_char('e');
        t.push_char('c');
        assert_eq!(t.value(), "sec");
    }

    #[test]
    fn set_value_replaces() {
        let mut t = TextInput::new();
        t.push_char('x');
        t.set_value("hello");
        assert_eq!(t.value(), "hello");
    }
}
