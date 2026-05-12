use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::glyphs;
use crate::theme::Theme;

/// Welcome-screen menu row: `▶ Configure new profile          hint`.
/// Same shape as `radio::render_row` minus the bullet.
pub fn render_row(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    focused: bool,
    label: &str,
    hint: Option<&str>,
    disabled: bool,
) {
    if area.width < 4 || area.height < 1 {
        return;
    }

    let arrow = if focused {
        format!("  {} ", glyphs::focus(theme.colored))
    } else {
        "    ".to_string()
    };
    let arrow_style = if focused {
        theme.accent_bold()
    } else {
        theme.dim()
    };
    let label_style = if disabled {
        theme.dim()
    } else if focused {
        theme.header()
    } else {
        theme.body()
    };

    let mut spans = vec![
        Span::styled(arrow, arrow_style),
        Span::styled(label.to_string(), label_style),
    ];
    if let Some(h) = hint {
        spans.push(Span::styled(format!("   {h}"), theme.dim()));
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
    fn focused_renders_arrow() {
        let area = Rect::new(0, 0, 50, 1);
        let mut buf = Buffer::empty(area);
        render_row(area, &mut buf, &t(), true, "Configure", None, false);
        let mut s = String::new();
        for x in 0..50 {
            s.push_str(buf[(x, 0)].symbol());
        }
        // ASCII fallback
        assert!(s.contains('>'), "got `{s}`");
        assert!(s.contains("Configure"));
    }
}
