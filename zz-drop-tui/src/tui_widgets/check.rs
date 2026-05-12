use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,
    Warn,
    Err,
    Skip,
    Busy,
}

pub fn render_row(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    status: CheckStatus,
    label: &str,
    detail: Option<&str>,
) {
    if area.width < 4 || area.height < 1 {
        return;
    }
    let (glyph, style) = match status {
        CheckStatus::Ok => ('✔', theme.ok()),
        CheckStatus::Warn => ('!', theme.warn()),
        CheckStatus::Err => ('✗', theme.danger()),
        CheckStatus::Skip => ('–', theme.dim()),
        CheckStatus::Busy => ('◌', theme.cyan()),
    };
    let glyph_char = if theme.colored {
        glyph
    } else {
        match status {
            CheckStatus::Ok => 'v',
            CheckStatus::Warn => '!',
            CheckStatus::Err => 'x',
            CheckStatus::Skip => '-',
            CheckStatus::Busy => '*',
        }
    };

    let mut spans = vec![
        Span::styled(format!("  {glyph_char} "), style),
        Span::styled(label.to_string(), theme.body()),
    ];
    if let Some(d) = detail {
        spans.push(Span::styled(format!("  {d}"), theme.dim()));
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

    fn into_str(width: u16) -> impl FnMut(CheckStatus, &str, Option<&str>) -> String {
        move |st, label, det| {
            let area = Rect::new(0, 0, width, 1);
            let mut buf = Buffer::empty(area);
            render_row(area, &mut buf, &t(), st, label, det);
            let mut s = String::new();
            for x in 0..width {
                s.push_str(buf[(x, 0)].symbol());
            }
            s.trim_end().to_string()
        }
    }

    #[test]
    fn ok_row_contains_label() {
        let mut r = into_str(40);
        let s = r(CheckStatus::Ok, "DNS", Some("ok"));
        assert!(s.contains("DNS"));
        assert!(s.contains("ok"));
    }

    #[test]
    fn err_row_contains_x_glyph() {
        let mut r = into_str(40);
        let s = r(CheckStatus::Err, "auth", None);
        // ASCII fallback (theme is no-color in this test)
        assert!(s.contains('x') || s.contains('✗'));
    }
}
