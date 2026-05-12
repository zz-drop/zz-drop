use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;
pub const TARGET_WIDTH: u16 = 100;
pub const TARGET_HEIGHT: u16 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutRects {
    pub title: Rect,
    pub steps: Rect,
    pub body: Rect,
    pub keybar: Rect,
    pub too_small: bool,
}

pub fn split(area: Rect) -> LayoutRects {
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        return LayoutRects {
            title: area,
            steps: area,
            body: area,
            keybar: area,
            too_small: true,
        };
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title bar
            Constraint::Length(1), // steps band
            Constraint::Min(1),    // body
            Constraint::Length(1), // keybar
        ])
        .split(area);

    LayoutRects {
        title: chunks[0],
        steps: chunks[1],
        body: chunks[2],
        keybar: chunks[3],
        too_small: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_steps_body_keybar_split_at_target() {
        let area = Rect::new(0, 0, TARGET_WIDTH, TARGET_HEIGHT);
        let r = split(area);
        assert!(!r.too_small);
        assert_eq!(r.title.height, 1);
        assert_eq!(r.steps.height, 1);
        assert_eq!(r.keybar.height, 1);
        assert!(r.body.height >= 20);
        // contiguous: title.y=0, steps.y=1, body.y=2, keybar.y=last
        assert_eq!(r.title.y, 0);
        assert_eq!(r.steps.y, 1);
        assert_eq!(r.body.y, 2);
        assert_eq!(r.keybar.y, area.height - 1);
    }

    #[test]
    fn min_size_still_splits() {
        let area = Rect::new(0, 0, MIN_WIDTH, MIN_HEIGHT);
        let r = split(area);
        assert!(!r.too_small);
        assert_eq!(r.title.height, 1);
        assert_eq!(r.steps.height, 1);
        assert_eq!(r.keybar.height, 1);
    }

    #[test]
    fn below_min_is_too_small() {
        let area = Rect::new(0, 0, 60, 20);
        let r = split(area);
        assert!(r.too_small);
    }

    #[test]
    fn narrow_terminal_is_too_small() {
        let area = Rect::new(0, 0, 70, 30);
        let r = split(area);
        assert!(r.too_small);
    }

    #[test]
    fn short_terminal_is_too_small() {
        let area = Rect::new(0, 0, 100, 20);
        let r = split(area);
        assert!(r.too_small);
    }
}
