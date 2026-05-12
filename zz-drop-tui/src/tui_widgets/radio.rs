use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::glyphs;
use crate::theme::Theme;

pub fn render_row(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    focused: bool,
    selected: bool,
    label: &str,
    hint: Option<&str>,
    disabled: bool,
) {
    if area.width < 4 || area.height < 1 {
        return;
    }

    let arrow = if focused {
        format!("{} ", glyphs::focus(theme.colored))
    } else {
        "  ".to_string()
    };
    let bullet = if selected { "(•) " } else { "( ) " };

    let arrow_style = if focused {
        theme.accent_bold()
    } else {
        theme.dim()
    };
    let bullet_style = if selected {
        theme.accent()
    } else if disabled {
        theme.dim()
    } else {
        theme.dim_bright()
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
        Span::styled(bullet.to_string(), bullet_style),
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
    fn focused_selected_renders_arrow_and_bullet() {
        let area = Rect::new(0, 0, 50, 1);
        let mut buf = Buffer::empty(area);
        render_row(area, &mut buf, &t(), true, true, "Nextcloud", None, false);
        let mut s = String::new();
        for x in 0..50 {
            s.push_str(buf[(x, 0)].symbol());
        }
        let s = s.trim_end().to_string();
        // ASCII fallback under no-color theme
        assert!(s.starts_with('>'), "got `{s}`");
        assert!(s.contains("(•)"));
        assert!(s.contains("Nextcloud"));
    }

    #[test]
    fn unfocused_unselected_no_arrow() {
        let area = Rect::new(0, 0, 50, 1);
        let mut buf = Buffer::empty(area);
        render_row(area, &mut buf, &t(), false, false, "Proton", None, true);
        let mut s = String::new();
        for x in 0..50 {
            s.push_str(buf[(x, 0)].symbol());
        }
        let s = s.trim_end().to_string();
        assert!(!s.starts_with('>'));
        assert!(s.contains("( )"));
        assert!(s.contains("Proton"));
    }
}
