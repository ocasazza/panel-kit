//! Scrollable panel content registry and painters.
//!
//! panel-kit-tui panels are content-agnostic, so the host owns one
//! [`PanelScroll`] keyed by panel identity. Painters measure their content,
//! record the extent under their panel key, and draw the registered window; the
//! same registry then answers wheel routing through
//! [`crate::input::route_wheel`], which reproduces the web shell's precedence:
//! content consumes the wheel until its edge, then it bubbles to the workspace.
//!
//! Typical use, from a panel body callback:
//!
//! ```ignore
//! scroll::lines(f, rect, &theme, &key, &mut app.panels, content);
//! ```

use std::collections::HashMap;

use panel_kit_core::reducer::WheelDisposition;
use panel_kit_core::widgets::scroll::ContentScroll;
use panel_kit_core::PanelKey;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::ResolvedTuiTheme;

/// Per-panel scroll offsets keyed by panel identity.
///
/// One entry per panel that measured its content this frame. Panels without an
/// entry answer every wheel with [`WheelDisposition::BubbleToWorkspace`], so
/// bodies that never overflow never trap the wheel.
#[derive(Debug)]
pub struct PanelScroll<K: PanelKey> {
    panels: HashMap<K, ContentScroll>,
}

impl<K: PanelKey> PanelScroll<K> {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            panels: HashMap::new(),
        }
    }

    /// Create an empty registry sized for `capacity` panels.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            panels: HashMap::with_capacity(capacity),
        }
    }

    /// Current `(x, y)` offsets in cells for `key`, `(0, 0)` when unmeasured.
    pub fn offset(&self, key: &K) -> (u16, u16) {
        self.panels.get(key).map(ContentScroll::offset).unwrap_or((0, 0))
    }

    /// Current `(x, y)` scrollable maxima in cells for `key`.
    pub fn max(&self, key: &K) -> (u16, u16) {
        self.panels.get(key).map(ContentScroll::max).unwrap_or((0, 0))
    }

    /// Whether `key`'s measured content overflows its viewport on either axis.
    pub fn is_scrollable(&self, key: &K) -> bool {
        self.panels
            .get(key)
            .is_some_and(ContentScroll::is_scrollable)
    }

    /// Record the content/viewport extents a painter measured for `key` and
    /// return the offsets to draw with. Shrinking a panel clamps its offsets.
    pub fn set_extent(
        &mut self,
        key: &K,
        content_w: u16,
        content_h: u16,
        view_w: u16,
        view_h: u16,
    ) -> (u16, u16) {
        let scroll = self.panels.entry(*key).or_default();
        scroll.set_extent(content_w, content_h, view_w, view_h);
        scroll.offset()
    }

    /// Apply integer line/column steps from keyboard or page scrolling,
    /// returning whether either offset moved.
    pub fn scroll_by(&mut self, key: &K, delta_x: i32, delta_y: i32) -> bool {
        self.panels
            .entry(*key)
            .or_default()
            .scroll_by(delta_x, delta_y)
    }

    /// Apply a wheel gesture in cells to `key` and report whether that
    /// content consumed it.
    pub fn absorb(&mut self, key: &K, delta_x: f64, delta_y: f64) -> WheelDisposition {
        self.panels
            .entry(*key)
            .or_default()
            .absorb_wheel(delta_x, delta_y)
    }

    /// Drop entries for panels that were not projected this frame, returning
    /// how many were removed.
    pub fn retain_panels(&mut self, live: &[K]) -> usize {
        let before = self.panels.len();
        self.panels.retain(|key, _| live.contains(key));
        before - self.panels.len()
    }

    /// Forget every panel's offsets and extents.
    pub fn clear(&mut self) {
        self.panels.clear();
    }
}

impl<K: PanelKey> Default for PanelScroll<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// Render `content` into `area` at the offsets registered for `key`, drawing a
/// vertical scrollbar in the rightmost column while the content overflows, and
/// return the offsets that were drawn with.
pub fn lines<K: PanelKey>(
    f: &mut Frame,
    area: Rect,
    t: &ResolvedTuiTheme,
    key: &K,
    panels: &mut PanelScroll<K>,
    content: Vec<Line<'_>>,
) -> (u16, u16) {
    let area = area.intersection(f.area());
    if area.width == 0 || area.height == 0 {
        return panels.offset(key);
    }

    let content_w = content
        .iter()
        .map(Line::width)
        .max()
        .unwrap_or(0)
        .min(u16::MAX as usize) as u16;
    let content_h = content.len().min(u16::MAX as usize) as u16;
    let overflows = content_h > area.height;
    // Reserve the right column for the bar so it never paints over text.
    let body = match overflows {
        true => Rect {
            width: area.width.saturating_sub(1),
            ..area
        },
        false => area,
    };

    let offset = panels.set_extent(key, content_w, content_h, body.width, body.height);
    f.render_widget(Paragraph::new(content).scroll((offset.1, offset.0)), body);

    if overflows {
        vertical_scrollbar(f, area, t, content_h, offset.1);
    }

    offset
}

/// Paint the panel-kit vertical scrollbar for `area`.
pub(crate) fn vertical_scrollbar(
    f: &mut Frame,
    area: Rect,
    t: &ResolvedTuiTheme,
    content_h: u16,
    offset: u16,
) {
    let mut state = ScrollbarState::new(content_h as usize).position(offset as usize);
    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .thumb_symbol("█")
            .track_symbol(Some("│"))
            .begin_symbol(None)
            .end_symbol(None)
            .style(Style::default().fg(t.dim))
            .thumb_style(Style::default().fg(t.line2)),
        area,
        &mut state,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    enum Panel {
        One,
        Two,
    }

    impl panel_kit_core::PanelKey for Panel {}

    #[test]
    fn registry_records_offsets_per_panel_and_defaults_to_bubbling() {
        let mut panels = PanelScroll::new();
        assert!(!panels.is_scrollable(&Panel::One));
        assert_eq!(panels.offset(&Panel::One), (0, 0));

        panels.set_extent(&Panel::One, 4, 20, 10, 6);
        panels.set_extent(&Panel::Two, 4, 4, 10, 6);
        assert!(panels.is_scrollable(&Panel::One));
        assert!(!panels.is_scrollable(&Panel::Two));

        assert_eq!(panels.absorb(&Panel::One, 0.0, 3.0), WheelDisposition::ContentConsumed);
        assert_eq!(panels.offset(&Panel::One), (0, 3));
        assert_eq!(panels.absorb(&Panel::Two, 0.0, 3.0), WheelDisposition::BubbleToWorkspace);
        assert_eq!(panels.absorb(&Panel::Two, 3.0, 0.0), WheelDisposition::BubbleToWorkspace);
    }

    #[test]
    fn retain_drops_panels_that_stopped_projecting() {
        let mut panels = PanelScroll::with_capacity(2);
        panels.set_extent(&Panel::One, 4, 20, 10, 6);
        panels.set_extent(&Panel::Two, 4, 20, 10, 6);
        assert!(panels.scroll_by(&Panel::One, 0, 2));

        assert_eq!(panels.retain_panels(&[Panel::One]), 1);
        assert_eq!(panels.offset(&Panel::One), (0, 2));
        assert!(!panels.is_scrollable(&Panel::Two));
        panels.clear();
        assert_eq!(panels.offset(&Panel::One), (0, 0));
    }
}
