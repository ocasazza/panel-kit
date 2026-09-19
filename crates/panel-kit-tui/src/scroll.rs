//! Scrollable panel content painter. panel-kit-tui panels are content-agnostic,
//! so the scroll offset lives with the consumer (it owns the content and knows
//! when it changes); this module owns the rendering: clamp the offset to the
//! core scroll policy math, draw the visible window, and paint a ratatui
//! scrollbar in the panel's right edge whenever the content overflows the body.
//!
//! Typical use, from a panel body callback:
//!
//! ```ignore
//! // store the clamped offset back so the wheel can't run off the end
//! app.scroll = panel_kit_tui::scroll::lines(f, rect, &theme, content, app.scroll);
//! ```

use panel_kit_core::widgets::scroll::max_offset;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::ResolvedTuiTheme;

/// Render `content` into `area` starting `offset` rows down, drawing a
/// vertical scrollbar in the rightmost column when the content overflows.
/// Returns the clamped offset so the caller can store it back.
pub fn lines(
    f: &mut Frame,
    area: Rect,
    t: &ResolvedTuiTheme,
    content: Vec<Line>,
    offset: usize,
) -> usize {
    let area = area.intersection(f.area());
    if area.width == 0 || area.height == 0 {
        return offset;
    }

    let total = content.len();
    let view = area.height;
    let max = max_offset(total, view);
    let off = offset.min(max);

    let overflow = total > view as usize;
    // Reserve the right column for the bar so it never paints over text.
    let body = if overflow {
        Rect {
            width: area.width.saturating_sub(1),
            ..area
        }
    } else {
        area
    };

    let visible_lines: Vec<Line> = content.into_iter().skip(off).take(view as usize).collect();
    f.render_widget(Paragraph::new(visible_lines), body);

    if overflow {
        let mut state = ScrollbarState::new(max).position(off);
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
    off
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_math_reuses_core_semantics() {
        assert_eq!(
            max_offset(10, 4),
            panel_kit_core::widgets::scroll::max_offset(10, 4)
        );
    }
}
