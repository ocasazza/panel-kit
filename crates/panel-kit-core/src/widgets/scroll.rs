//! Scroll policy, per-content scroll state, and text wrapping math.

use crate::reducer::WheelDisposition;

/// The maximum line offset for `total` lines in a `view`-row viewport.
pub fn max_offset(total: usize, view: u16) -> usize {
    total.saturating_sub(view as usize)
}

/// Hard-wrap `text` into lines of at most `width` characters.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if width == 0 || chars.len() <= width {
        return vec![text.to_string()];
    }

    chars
        .chunks(width)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

/// One panel body's scroll offsets plus the maxima implied by the last
/// measured content/viewport extents.
///
/// Renderer-neutral so every backend resolves one wheel precedence: a wheel
/// moves the offsets while they still have room in that direction and only
/// bubbles to the workspace once the content edge is reached, mirroring the web
/// shell's native-overflow chaining.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentScroll {
    offset_x: u16,
    offset_y: u16,
    max_x: u16,
    max_y: u16,
}

impl ContentScroll {
    /// Create a scrolled-to-top-left state with no known overflow.
    pub const fn new() -> Self {
        Self {
            offset_x: 0,
            offset_y: 0,
            max_x: 0,
            max_y: 0,
        }
    }

    /// Current `(x, y)` offsets in cells.
    pub const fn offset(&self) -> (u16, u16) {
        (self.offset_x, self.offset_y)
    }

    /// Current `(x, y)` scrollable maxima in cells.
    pub const fn max(&self) -> (u16, u16) {
        (self.max_x, self.max_y)
    }

    /// Whether the measured content overflows its viewport on either axis.
    pub const fn is_scrollable(&self) -> bool {
        self.max_x > 0 || self.max_y > 0
    }

    /// Record the content and viewport extents, recompute the maxima, and
    /// clamp the offsets into them.
    ///
    /// Returns whether clamping moved either offset, which happens when a
    /// panel shrank since the previous frame.
    pub fn set_extent(
        &mut self,
        content_w: u16,
        content_h: u16,
        view_w: u16,
        view_h: u16,
    ) -> bool {
        self.max_x = max_offset(content_w as usize, view_w) as u16;
        self.max_y = max_offset(content_h as usize, view_h) as u16;
        let clamped_x = self.offset_x.min(self.max_x);
        let clamped_y = self.offset_y.min(self.max_y);
        let changed = clamped_x != self.offset_x || clamped_y != self.offset_y;
        self.offset_x = clamped_x;
        self.offset_y = clamped_y;
        changed
    }

    /// Apply integer line/column steps from keyboard or page scrolling.
    ///
    /// Returns whether either offset moved.
    pub fn scroll_by(&mut self, delta_x: i32, delta_y: i32) -> bool {
        let before = (self.offset_x, self.offset_y);
        self.offset_x = stepped(self.offset_x, self.max_x, f64::from(delta_x));
        self.offset_y = stepped(self.offset_y, self.max_y, f64::from(delta_y));
        (self.offset_x, self.offset_y) != before
    }

    /// Apply wheel deltas in cells (positive is right/down) and report whether
    /// the content consumed them.
    ///
    /// Deltas are rounded to whole cells; non-finite deltas are ignored. A
    /// wheel that cannot move either axis returns
    /// [`WheelDisposition::BubbleToWorkspace`], which is the content edge or a
    /// body whose content fits.
    pub fn absorb_wheel(&mut self, delta_x: f64, delta_y: f64) -> WheelDisposition {
        let before = (self.offset_x, self.offset_y);
        self.offset_x = stepped(self.offset_x, self.max_x, delta_x);
        self.offset_y = stepped(self.offset_y, self.max_y, delta_y);
        if (self.offset_x, self.offset_y) == before {
            WheelDisposition::BubbleToWorkspace
        } else {
            WheelDisposition::ContentConsumed
        }
    }

    /// Return to the top-left origin, keeping the measured maxima.
    pub fn reset(&mut self) {
        self.offset_x = 0;
        self.offset_y = 0;
    }
}

/// Move `offset` by `delta`, clamped into `0..=max`, rounding to whole cells.
fn stepped(offset: u16, max: u16, delta: f64) -> u16 {
    if !delta.is_finite() {
        return offset;
    }

    let next = f64::from(offset) + delta;
    if next <= 0.0 {
        0
    } else if next >= f64::from(max) {
        max
    } else {
        next.round() as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scrolling(content_w: u16, content_h: u16, view_w: u16, view_h: u16) -> ContentScroll {
        let mut scroll = ContentScroll::new();
        scroll.set_extent(content_w, content_h, view_w, view_h);
        scroll
    }

    #[test]
    fn max_offset_matches_tui_saturating_math() {
        assert_eq!(max_offset(10, 4), 6);
        assert_eq!(max_offset(4, 4), 0);
        assert_eq!(max_offset(2, 4), 0);
    }

    #[test]
    fn wrap_matches_tui_character_chunks() {
        assert_eq!(wrap("abcdef", 2), vec!["ab", "cd", "ef"]);
        assert_eq!(wrap("abc", 0), vec!["abc"]);
        assert_eq!(wrap("abc", 5), vec!["abc"]);
    }

    #[test]
    fn set_extent_computes_maxima_and_reports_clamping() {
        let mut scroll = ContentScroll::new();
        assert!(!scroll.set_extent(40, 20, 10, 6));
        assert_eq!(scroll.max(), (30, 14));
        assert!(scroll.is_scrollable());

        assert_eq!(
            scroll.absorb_wheel(0.0, 14.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset(), (0, 14));

        assert!(scroll.set_extent(40, 8, 10, 6));
        assert_eq!(scroll.offset(), (0, 2));
    }

    #[test]
    fn content_that_fits_bubbles_in_every_direction() {
        let mut scroll = scrolling(6, 3, 10, 6);
        assert!(!scroll.is_scrollable());
        for delta in [(0.0, 3.0), (0.0, -3.0), (3.0, 0.0), (-3.0, 0.0)] {
            assert_eq!(
                scroll.absorb_wheel(delta.0, delta.1),
                WheelDisposition::BubbleToWorkspace
            );
        }
        assert_eq!(scroll.offset(), (0, 0));
    }

    #[test]
    fn bottom_and_right_edges_bubble_but_the_opposite_directions_do_not() {
        let mut scroll = scrolling(12, 10, 4, 4);
        assert_eq!(scroll.max(), (8, 6));

        assert_eq!(
            scroll.absorb_wheel(0.0, 3.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset().1, 3);
        assert_eq!(
            scroll.absorb_wheel(0.0, 6.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset().1, 6);
        assert_eq!(
            scroll.absorb_wheel(0.0, 1.0),
            WheelDisposition::BubbleToWorkspace
        );

        assert_eq!(
            scroll.absorb_wheel(2.0, 0.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset().0, 2);
        assert_eq!(
            scroll.absorb_wheel(99.0, 0.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset().0, 8);
        assert_eq!(
            scroll.absorb_wheel(1.0, 0.0),
            WheelDisposition::BubbleToWorkspace
        );

        assert_eq!(
            scroll.absorb_wheel(0.0, -2.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset().1, 4);
        assert_eq!(
            scroll.absorb_wheel(0.0, -99.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset(), (8, 0));
        assert_eq!(
            scroll.absorb_wheel(0.0, -1.0),
            WheelDisposition::BubbleToWorkspace
        );
        assert_eq!(
            scroll.absorb_wheel(-1.0, 0.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset(), (7, 0));
        assert_eq!(
            scroll.absorb_wheel(-1.0, -1.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset(), (6, 0));
        assert_eq!(
            scroll.absorb_wheel(-99.0, 0.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset(), (0, 0));
        assert_eq!(
            scroll.absorb_wheel(-1.0, -1.0),
            WheelDisposition::BubbleToWorkspace
        );
        assert_eq!(
            scroll.absorb_wheel(-1.0, 1.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset(), (0, 1));
    }

    #[test]
    fn a_diagonal_wheel_consumes_when_either_axis_has_room() {
        let mut scroll = scrolling(12, 10, 4, 4);
        assert_eq!(
            scroll.absorb_wheel(0.0, -1.0),
            WheelDisposition::BubbleToWorkspace
        );
        assert_eq!(
            scroll.absorb_wheel(0.0, 2.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(
            scroll.absorb_wheel(-1.0, -1.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset(), (0, 1));

        let mut at_left_edge = scrolling(12, 10, 12, 4);
        assert_eq!(at_left_edge.max().0, 0);
        assert_eq!(
            at_left_edge.absorb_wheel(-1.0, 1.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(at_left_edge.offset(), (0, 1));
    }

    #[test]
    fn fractional_and_non_finite_deltas_clamp_without_panicking() {
        let mut scroll = scrolling(12, 10, 4, 4);
        assert_eq!(
            scroll.absorb_wheel(0.0, f64::NAN),
            WheelDisposition::BubbleToWorkspace
        );
        assert_eq!(
            scroll.absorb_wheel(f64::INFINITY, 0.0),
            WheelDisposition::BubbleToWorkspace
        );
        assert_eq!(
            scroll.absorb_wheel(0.0, -f64::INFINITY),
            WheelDisposition::BubbleToWorkspace
        );
        assert_eq!(
            scroll.absorb_wheel(99.0, -99.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset(), (8, 0));
        assert_eq!(
            scroll.absorb_wheel(1.2, 1.2),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(scroll.offset(), (8, 1));
    }

    #[test]
    fn keyboard_steps_share_the_wheel_clamping() {
        let mut scroll = scrolling(12, 10, 4, 4);
        assert!(scroll.scroll_by(0, 20));
        assert_eq!(scroll.offset().1, 6);
        assert!(!scroll.scroll_by(0, 20));
        assert!(!scroll.scroll_by(-99, 0));
        assert!(scroll.scroll_by(6, -6));
        assert_eq!(scroll.offset(), (6, 0));
        assert!(scroll.scroll_by(1, 1));
        assert_eq!(scroll.offset(), (7, 1));
        assert!(scroll.scroll_by(0, -1));
        assert!(scroll.scroll_by(-99, 0));
        assert_eq!(scroll.offset(), (0, 0));
        assert!(scroll.scroll_by(0, 6));
        assert!(scroll.scroll_by(99, 0));
        assert_eq!(scroll.offset(), (8, 6));
        assert!(!scroll.scroll_by(1, 1));

        scroll.scroll_by(4, 4);
        scroll.reset();
        assert_eq!(scroll.offset(), (0, 0));
        assert_eq!(scroll.max(), (8, 6));
    }
}
