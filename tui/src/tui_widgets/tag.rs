use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagKind {
    Ok,
    Warn,
    Err,
    Busy,
    Skip,
    Info,
}

impl TagKind {
    fn glyph(self, colored: bool) -> char {
        match (self, colored) {
            (Self::Ok, true) => '✓',
            (Self::Ok, false) => 'v',
            (Self::Warn, true) => '!',
            (Self::Warn, false) => '!',
            (Self::Err, true) => '✗',
            (Self::Err, false) => 'x',
            (Self::Busy, true) => '…',
            (Self::Busy, false) => '*',
            (Self::Skip, true) => '–',
            (Self::Skip, false) => '-',
            (Self::Info, true) => 'i',
            (Self::Info, false) => 'i',
        }
    }
}

pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme, kind: TagKind, msg: &str) {
    if area.width < 4 || area.height < 1 {
        return;
    }
    let style = match kind {
        TagKind::Ok => theme.ok(),
        TagKind::Warn => theme.warn(),
        TagKind::Err => theme.danger(),
        TagKind::Busy => theme.dim_bright(),
        TagKind::Skip => theme.dim(),
        TagKind::Info => theme.cyan(),
    };
    let line = Line::from(vec![
        Span::styled("[", style),
        Span::styled(format!("{} ", kind.glyph(theme.colored)), style),
        Span::styled(msg.to_string(), style),
        Span::styled("]", style),
    ]);
    let p = Paragraph::new(line);
    ratatui::widgets::Widget::render(p, area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::MockEnv;

    fn theme() -> Theme {
        Theme::from_parts(&MockEnv::empty(), false)
    }

    fn render_into(width: u16, kind: TagKind, msg: &str) -> String {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &theme(), kind, msg);
        let mut out = String::new();
        for x in 0..width {
            out.push_str(buf[(x, 0)].symbol());
        }
        out.trim_end().to_string()
    }

    #[test]
    fn renders_ok_tag() {
        let s = render_into(20, TagKind::Ok, "URL parses");
        assert!(s.starts_with('['), "got `{s}`");
        assert!(s.contains("URL parses"));
        assert!(s.ends_with(']'));
    }

    #[test]
    fn renders_err_tag() {
        let s = render_into(20, TagKind::Err, "bad");
        assert!(s.contains("bad"));
    }
}
