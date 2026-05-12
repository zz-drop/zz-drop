use ratatui::layout::Rect;

/// Split `area` horizontally into a left column of `left_width` cells
/// and a right column for the rest, separated by `gap` empty cells.
/// When the area is too narrow to make the split sensible, returns
/// `(area, Rect::ZERO)` so the caller can fall back to a single-column
/// layout. Uses 30 cells as the default minimum right-pane width.
pub fn split(area: Rect, left_width: u16, gap: u16) -> (Rect, Rect) {
    split_with_min_right(area, left_width, gap, 30)
}

/// Like `split` but lets the caller decide how narrow the right column
/// can get before we fall back to single-column. Used by screens that
/// have a right-pane content (e.g. one-line URL host) which can survive
/// being squashed below the default threshold.
pub fn split_with_min_right(
    area: Rect,
    left_width: u16,
    gap: u16,
    min_right: u16,
) -> (Rect, Rect) {
    if area.width < left_width + gap + min_right {
        return (area, Rect::ZERO);
    }
    let left = Rect::new(area.x, area.y, left_width, area.height);
    let right_x = area.x + left_width + gap;
    let right = Rect::new(
        right_x,
        area.y,
        area.width.saturating_sub(left_width + gap),
        area.height,
    );
    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_target_width() {
        let area = Rect::new(0, 0, 100, 20);
        let (l, r) = split(area, 44, 2);
        assert_eq!(l.width, 44);
        assert_eq!(r.x, 46);
        assert_eq!(r.width, 54);
        assert_eq!(l.height, 20);
        assert_eq!(r.height, 20);
    }

    #[test]
    fn falls_back_to_single_column_when_narrow() {
        let area = Rect::new(0, 0, 70, 20);
        let (l, r) = split(area, 44, 2);
        assert_eq!(l, area);
        assert_eq!(r, Rect::ZERO);
    }
}
