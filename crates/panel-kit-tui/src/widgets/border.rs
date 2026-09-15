//! Shared terminal border drawing for TUI composition parts.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::Frame;

use crate::Charset;

/// Draw a charset-specific single-cell border into the frame buffer.
pub(crate) fn draw_border(frame: &mut Frame, area: Rect, charset: Charset, style: Style) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let border = charset.border();
    let left = area.x;
    let right = area.right().saturating_sub(1);
    let top = area.y;
    let bottom = area.bottom().saturating_sub(1);

    for x in left..=right {
        set_symbol(frame, x, top, border.horizontal_top, style);
        set_symbol(frame, x, bottom, border.horizontal_bottom, style);
    }

    for y in top..=bottom {
        set_symbol(frame, left, y, border.vertical_left, style);
        set_symbol(frame, right, y, border.vertical_right, style);
    }

    set_symbol(frame, left, top, border.top_left, style);
    set_symbol(frame, right, top, border.top_right, style);
    set_symbol(frame, left, bottom, border.bottom_left, style);
    set_symbol(frame, right, bottom, border.bottom_right, style);
}

/// Write the first cell-width character from a ratatui border symbol.
fn set_symbol(frame: &mut Frame, x: u16, y: u16, symbol: &str, style: Style) {
    if let Some(ch) = symbol.chars().next() {
        frame.buffer_mut()[(x, y)].set_char(ch).set_style(style);
    }
}
