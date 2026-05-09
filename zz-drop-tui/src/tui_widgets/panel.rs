use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use super::glyphs;
use crate::theme::{PanelAccent, Theme};

/// Open a panel: paint a coloured title strip on `area.y` and a bordered
/// `Block` from `area.y + 1`. Returns the inner `Rect` the caller can
/// draw into.
///
/// Layout:
/// ```text
/// ▍ login flow v2                 ← bar glyph + title in accent
/// ╭─────────────────────╮         ← top border in accent
/// │                     │
/// │  body lines here    │
/// │                     │
/// ╰─────────────────────╯
/// ```
pub fn open(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    accent: PanelAccent,
    title: &str,
) -> Rect {
    if area.width < 4 || area.height < 3 {
        return Rect::ZERO;
    }

    // Title strip on top row: bar glyph + title text. The bar mirrors
    // the wordmark in the title bar so panels feel anchored to the chrome.
    let title_area = Rect::new(area.x, area.y, area.width, 1);
    let bar = glyphs::bar_left(theme.colored).to_string();
    let trimmed = title.trim();
    let title_p = Paragraph::new(Line::from(vec![
        Span::styled(format!("{bar} "), theme.border_accent(accent)),
        Span::styled(trimmed.to_string(), theme.panel_title(accent)),
    ]));
    ratatui::widgets::Widget::render(title_p, title_area, buf);

    // Block borders below.
    let block_area = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_accent(accent));
    let inner = block.inner(block_area);
    ratatui::widgets::Widget::render(block, block_area, buf);
    inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::MockEnv;

    fn t() -> Theme {
        Theme::from_parts(&MockEnv::empty(), false)
    }

    #[test]
    fn returns_inner_rect_smaller_than_outer() {
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        let inner = open(area, &mut buf, &t(), PanelAccent::Mint, " test ");
        assert!(inner.width < area.width);
        assert!(inner.height < area.height);
        // inner should start below the title strip
        assert!(inner.y > area.y);
    }

    #[test]
    fn renders_title_text_on_top_row() {
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        open(area, &mut buf, &t(), PanelAccent::Cyan, " login flow v2 ");
        let mut s = String::new();
        for x in 0..40 {
            s.push_str(buf[(x, 0)].symbol());
        }
        assert!(s.contains("login flow v2"), "got `{s}`");
    }

    #[test]
    fn tiny_area_returns_zero_rect() {
        let area = Rect::new(0, 0, 3, 2);
        let mut buf = Buffer::empty(area);
        let inner = open(area, &mut buf, &t(), PanelAccent::Mint, "x");
        assert_eq!(inner, Rect::ZERO);
    }
}
