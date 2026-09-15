use panel_kit_core::widgets::charts::FlameSpanModel;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::Frame;

use crate::ResolvedTuiTheme;

use super::{contrast_ink, mix, rgb_color, series_colors};

#[derive(Clone, Copy)]
struct OpenSpan {
    depth: u16,
    x0: f64,
    x1: f64,
    cursor: f64,
    siblings_total: f64,
}

#[derive(Clone, Copy)]
struct FlameBand {
    x0: f64,
    x1: f64,
}

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
    let child_totals = child_totals(spans);
    let root_total = root_total(spans);
    let mut open = Vec::new();

    for (index, span) in spans.iter().enumerate() {
        let band = layout_span(
            &mut open,
            span,
            child_totals[index],
            root_total,
            area.width as f64,
        );
        paint_span(f, area, t, &colors, span, band);
    }
}

fn child_totals(spans: &[FlameSpanModel]) -> Vec<f64> {
    let mut totals = vec![0.0f64; spans.len()];
    let mut stack: Vec<usize> = Vec::new();

    for (index, span) in spans.iter().enumerate() {
        close_index_ancestors(&mut stack, spans, span.depth);
        if let Some(&parent) = stack.last() {
            totals[parent] += span.value.max(0.0);
        }
        stack.push(index);
    }

    totals
}

fn close_index_ancestors(stack: &mut Vec<usize>, spans: &[FlameSpanModel], depth: u16) {
    while let Some(&parent) = stack.last() {
        if spans[parent].depth < depth {
            break;
        }
        stack.pop();
    }
}

fn root_total(spans: &[FlameSpanModel]) -> f64 {
    spans
        .iter()
        .filter(|span| span.depth == 0)
        .map(|span| span.value.max(0.0))
        .sum::<f64>()
        .max(f64::MIN_POSITIVE)
}

fn layout_span(
    open: &mut Vec<OpenSpan>,
    span: &FlameSpanModel,
    child_total: f64,
    root_total: f64,
    full_width: f64,
) -> FlameBand {
    close_open_ancestors(open, span.depth);

    let (parent_cursor, parent_right, parent_total, parent_width) = match open.last() {
        Some(parent) => (
            parent.cursor,
            parent.x1,
            parent.siblings_total,
            parent.x1 - parent.x0,
        ),
        None => (0.0, full_width, root_total, full_width),
    };
    let share = if parent_total > 0.0 {
        span.value.max(0.0) / parent_total
    } else {
        0.0
    };
    let x0 = parent_cursor;
    let x1 = (x0 + share * parent_width).min(parent_right);

    if let Some(parent) = open.last_mut() {
        parent.cursor = x1;
    }
    open.push(OpenSpan {
        depth: span.depth,
        x0,
        x1,
        cursor: x0,
        siblings_total: child_total,
    });

    FlameBand { x0, x1 }
}

fn close_open_ancestors(open: &mut Vec<OpenSpan>, depth: u16) {
    while let Some(parent) = open.last() {
        if parent.depth < depth {
            break;
        }
        open.pop();
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
    let row = area.y + span.depth;
    if row >= area.bottom() {
        return;
    }

    let cell_x0 = area.x + band.x0.floor() as u16;
    let cell_w = (band.x1.floor() - band.x0.floor()).max(0.0) as u16;
    let cell_w = cell_w.min(area.right().saturating_sub(cell_x0));
    if cell_w == 0 {
        return;
    }

    let color = span.color.map(rgb_color).unwrap_or_else(|| {
        let base = colors[span.depth as usize % colors.len()];
        mix(base, t.bg, (span.depth as f64 * 0.08).min(0.45))
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
    let style = Style::default().bg(color);
    for x in x0..x0 + width {
        f.buffer_mut()[(x, row)].set_char(' ').set_style(style);
    }
}

fn paint_label(
    f: &mut Frame,
    row: u16,
    x0: u16,
    width: u16,
    bg: Color,
    fg: Color,
    label: &str,
) {
    let style = Style::default().fg(fg).bg(bg);
    for (offset, ch) in label.chars().take(width as usize).enumerate() {
        f.buffer_mut()[(x0 + offset as u16, row)]
            .set_char(ch)
            .set_style(style);
    }
}

