use panel_kit_core::widgets::charts::{flame_bands, FlameBand, FlameSpanModel};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::Frame;

use crate::ResolvedTuiTheme;

use super::{contrast_ink, mix, rgb_color, series_colors};

/// Render a flamegraph (icicle) into `area` from shared core span models.
///
/// Row 0 (the root) spans the full width at the top; deeper frames stack
/// downward, each laid out within the horizontal extent of its parent. Cell
/// color defaults to a depth-cycled theme hue, but a span may override it.
/// Frames narrower than one cell are dropped.
pub fn flame(f: &mut Frame, area: Rect, t: &ResolvedTuiTheme, spans: &[FlameSpanModel]) {
    if area.width == 0 || area.height == 0 || spans.is_empty() {
        return;
    }

    let colors = series_colors(t);
    for (span, band) in spans.iter().zip(flame_bands(spans, area.width as f64)) {
        paint_span(f, area, t, &colors, span, band);
    }
}

fn paint_span(
    f: &mut Frame,
    area: Rect,
    t: &ResolvedTuiTheme,
    colors: &[Color],
    span: &FlameSpanModel,
    band: FlameBand,
) {
    let frame_area = f.area();
    let row = area.y + span.depth;
    if row >= area.bottom() || row >= frame_area.bottom() {
        return;
    }

    let cell_x0 = area.x + band.x0.floor() as u16;
    if cell_x0 >= area.right() || cell_x0 >= frame_area.right() {
        return;
    }
    let cell_w = (band.x1.floor() - band.x0.floor()).max(0.0) as u16;
    let cell_w = cell_w
        .min(area.right().saturating_sub(cell_x0))
        .min(frame_area.right().saturating_sub(cell_x0));
    if cell_w == 0 {
        return;
    }
    let color = span.color.map(rgb_color).unwrap_or_else(|| {
        let base = colors[span.depth as usize % colors.len()];
        mix(base, t.background, (span.depth as f64 * 0.08).min(0.45))
    });
    paint_background(f, row, cell_x0, cell_w, color);
    paint_label(
        f,
        row,
        cell_x0,
        cell_w,
        color,
        contrast_ink(color, t),
        &span.label,
    );
}

fn paint_background(f: &mut Frame, row: u16, x0: u16, width: u16, color: Color) {
    let frame_area = f.area();
    if row >= frame_area.bottom() {
        return;
    }
    let max_x = (x0 + width).min(frame_area.right());
    let style = Style::default().bg(color);
    for x in x0..max_x {
        f.buffer_mut()[(x, row)].set_char(' ').set_style(style);
    }
}

fn paint_label(f: &mut Frame, row: u16, x0: u16, width: u16, bg: Color, fg: Color, label: &str) {
    let frame_area = f.area();
    if row >= frame_area.bottom() {
        return;
    }
    let max_width = width.min(frame_area.right().saturating_sub(x0));
    let style = Style::default().fg(fg).bg(bg);
    for (offset, ch) in label.chars().take(max_width as usize).enumerate() {
        let x = x0 + offset as u16;
        if x < frame_area.right() {
            f.buffer_mut()[(x, row)]
                .set_char(ch)
                .set_style(style);
        }
    }
}
