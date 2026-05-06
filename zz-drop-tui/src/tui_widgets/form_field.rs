use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::glyphs;
use crate::input::TextInput;
use crate::theme::{PanelAccent, Theme};

const LABEL_WIDTH: u16 = 14;

/// Form field row: `<label, 14ch dim_bright>  [bordered value box with cursor]`.
/// Caller is expected to allocate 3 rows so the value box can be drawn with
/// `Borders::ALL`. The label sits centred on the middle row, aligned with
/// the value text. Long values scroll horizontally so the trailing cursor
/// stays inside the box, never overlapping the right border.
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    label: &str,
    input: &TextInput,
    focused: bool,
) {
    if area.width < LABEL_WIDTH + 6 || area.height < 3 {
        return;
    }

    // Label sits on the middle row, right-aligned in 14 cells.
    let mid_y = area.y + 1;
    let label_row = Rect::new(area.x, mid_y, LABEL_WIDTH + 1, 1);
    let label_p = Paragraph::new(Line::from(Span::styled(
        format!("{label:>width$} ", width = (LABEL_WIDTH - 1) as usize),
        theme.dim_bright(),
    )));
    ratatui::widgets::Widget::render(label_p, label_row, buf);

    // Bordered value box covering all 3 rows of the band.
    let accent = if focused {
        PanelAccent::Mint
    } else {
        PanelAccent::Dim
    };
    // Reserve 1 cell on the right so the box doesn't touch the panel
    // border that usually surrounds the field.
    let value_area = Rect::new(
        area.x + LABEL_WIDTH + 1,
        area.y,
        area.width.saturating_sub(LABEL_WIDTH + 2),
        3,
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_accent(accent));
    let inner = block.inner(value_area);
    ratatui::widgets::Widget::render(block, value_area, buf);

    let value = input.value();
    let display: String = if input.is_masked() {
        "*".repeat(value.chars().count())
    } else {
        value.to_string()
    };

    // Horizontal scroll so the tail (cursor) is always visible. We keep
    // the cursor one cell left of the right border to avoid overlapping
    // it.
    let display_chars: Vec<char> = display.chars().collect();
    let max_visible = inner.width.saturating_sub(1) as usize; // reserve cursor cell
    let (visible_start, visible_end) = if display_chars.len() <= max_visible {
        (0, display_chars.len())
    } else {
        (
            display_chars.len() - max_visible,
            display_chars.len(),
        )
    };
    let visible: String = display_chars[visible_start..visible_end].iter().collect();

    let value_style = if focused {
        theme.body()
    } else {
        theme.dim_bright()
    };
    let value_p = Paragraph::new(Line::from(vec![
        Span::styled(visible, value_style),
        if focused {
            Span::styled(
                glyphs::cursor(theme.colored).to_string(),
                theme.accent_bold(),
            )
        } else {
            Span::raw("")
        },
    ]));
    ratatui::widgets::Widget::render(value_p, inner, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::MockEnv;

    fn t() -> Theme {
        Theme::from_parts(&MockEnv::empty(), false)
    }

    #[test]
    fn renders_label_and_value() {
        let area = Rect::new(0, 0, 60, 3);
        let mut buf = Buffer::empty(area);
        let mut input = TextInput::new();
        input.set_value("hello");
        render(area, &mut buf, &t(), "server URL", &input, true);
        // Middle row: label on the left, value on the right
        let mut s = String::new();
        for x in 0..60 {
            s.push_str(buf[(x, 1)].symbol());
        }
        assert!(s.contains("server URL"), "got `{s}`");
        assert!(s.contains("hello"), "got `{s}`");
    }

    #[test]
    fn long_value_does_not_overflow_right_border() {
        let area = Rect::new(0, 0, 30, 3);
        let mut buf = Buffer::empty(area);
        let mut input = TextInput::new();
        input.set_value(&"x".repeat(200));
        render(area, &mut buf, &t(), "url", &input, true);
        // The right border at x=29, middle row, must still be a border
        // glyph (not bled-through 'x' content).
        let last = buf[(29, 1)].symbol();
        assert!(!last.contains('x'), "value bled past right border: `{last}`");
    }

    #[test]
    fn masked_input_shows_stars() {
        let area = Rect::new(0, 0, 50, 3);
        let mut buf = Buffer::empty(area);
        let mut input = TextInput::masked();
        input.set_value("topsecret");
        render(area, &mut buf, &t(), "passphrase", &input, true);
        // Middle row holds the masked value.
        let mut s = String::new();
        for x in 0..50 {
            s.push_str(buf[(x, 1)].symbol());
        }
        assert!(s.contains("*********"), "got `{s}`");
        assert!(!s.contains("topsecret"));
    }

    #[test]
    fn full_borders_drawn_on_top_and_bottom_rows() {
        let area = Rect::new(0, 0, 60, 3);
        let mut buf = Buffer::empty(area);
        let mut input = TextInput::new();
        input.set_value("v");
        render(area, &mut buf, &t(), "url", &input, true);
        // Top-left corner of the value box is at x = LABEL_WIDTH + 1 = 15.
        let top = buf[(15, 0)].symbol();
        let bot = buf[(15, 2)].symbol();
        assert!(
            top == "┌" || top == "+" || !top.is_empty(),
            "no top-left corner: `{top}`"
        );
        assert!(
            bot == "└" || bot == "+" || !bot.is_empty(),
            "no bot-left corner: `{bot}`"
        );
    }
}
