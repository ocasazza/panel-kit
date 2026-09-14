//! Scrollable panel content. panel-kit-tui panels are content-agnostic, so
//! the scroll offset lives with the consumer (it owns the content and knows
//! when it changes); this module owns the rendering: clamp the offset to the
//! content, draw the visible window, and paint a ratatui scrollbar in the
//! panel's right edge whenever the content overflows the body.
//!
//! Typical use, from a panel body callback:
//!
//! ```ignore
//! // store the clamped offset back so the wheel can't run off the end
//! app.scroll = panel_kit_tui::scroll::lines(f, rect, &theme, content, app.scroll);
//! ```

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::Theme;

/// The maximum offset for `total` lines in a `view`-row viewport — the
/// consumer clamps wheel events against this so it never scrolls past the end.
pub fn max_offset(total: usize, view: u16) -> usize {
    total.saturating_sub(view as usize)
}

/// Render `content` into `area` starting `offset` rows down, drawing a
/// vertical scrollbar in the rightmost column when the content overflows.
/// Returns the clamped offset so the caller can store it back.
pub fn lines(f: &mut Frame, area: Rect, t: &Theme, content: Vec<Line>, offset: usize) -> usize {
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

    let visible: Vec<Line> = content.into_iter().skip(off).take(view as usize).collect();
    f.render_widget(Paragraph::new(visible), body);

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

/// Hard-wrap `s` into lines of at most `width` characters (character-based,
/// no word boundaries). Returns the string as a single line when it fits or
/// `width` is zero. Pairs with [`lines`] for laying out free text in a panel.
pub fn wrap(s: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    if width == 0 || chars.len() <= width {
        return vec![s.to_string()];
    }
    chars.chunks(width).map(|c| c.iter().collect()).collect()
}
