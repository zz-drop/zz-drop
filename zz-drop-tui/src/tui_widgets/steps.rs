use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Past,
    Active,
    Future,
    Disabled,
}

impl StepState {
    fn dot(self, colored: bool) -> char {
        match (self, colored) {
            (Self::Past, true) => '●',
            (Self::Active, true) => '◉',
            (Self::Future, true) => '○',
            (Self::Disabled, true) => '·',
            (Self::Past, false) => '*',
            (Self::Active, false) => '@',
            (Self::Future, false) => 'o',
            (Self::Disabled, false) => '.',
        }
    }
}

/// Render a step band with a leading dot per step. The active step's dot
/// uses the mint accent + bold, the active label is underlined.
pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme, labels: &[(&str, StepState)]) {
    if area.width < 8 || area.height < 1 {
        return;
    }

    let mut spans: Vec<Span<'_>> = Vec::with_capacity(labels.len() * 4);
    for (i, (label, state)) in labels.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", theme.dim()));
        }
        let dot = state.dot(theme.colored).to_string();
        let (dot_style, label_style) = match state {
            StepState::Active => (theme.step_active_dot(), theme.step_active()),
            StepState::Past => (theme.accent(), theme.step_past()),
            StepState::Future => (theme.dim(), theme.step_future()),
            StepState::Disabled => (theme.step_disabled(), theme.step_disabled()),
        };
        spans.push(Span::styled(dot, dot_style));
        spans.push(Span::styled(" ", label_style));
        spans.push(Span::styled(label.to_string(), label_style));
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
    fn renders_all_labels_in_order() {
        let area = Rect::new(0, 0, 100, 1);
        let mut buf = Buffer::empty(area);
        let labels = [
            ("welcome", StepState::Past),
            ("provider", StepState::Active),
            ("server", StepState::Future),
        ];
        render(area, &mut buf, &t(), &labels);
        let mut s = String::new();
        for x in 0..100 {
            s.push_str(buf[(x, 0)].symbol());
        }
        assert!(s.contains("welcome"));
        assert!(s.contains("provider"));
        assert!(s.contains("server"));
        // ASCII fallback dots: past=*, active=@, future=o
        assert!(s.contains("* welcome"), "got `{s}`");
        assert!(s.contains("@ provider"), "got `{s}`");
        assert!(s.contains("o server"), "got `{s}`");
    }
}
