//! Time-series chart component — the terminal seed of the planned
//! panel-kit-charts crate (TimeSeries is its flagship there too). Multiple
//! named series, theme-derived colors, auto-scaled axes, braille line
//! rendering, legend.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::symbols;
use ratatui::text::Span;
use ratatui::widgets::{Axis, Chart, Dataset, Gauge, GraphType, LegendPosition};
use ratatui::Frame;

use crate::Theme;

/// Blend two colors by `t` in `0..=1` (0 = `a`, 1 = `b`). Non-RGB colors
/// fall back to `a`.
fn mix(a: Color, b: Color, t: f64) -> Color {
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            let t = t.clamp(0.0, 1.0);
            let l = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
            Color::Rgb(l(ar, br), l(ag, bg), l(ab, bb))
        }
        _ => a,
    }
}

/// One named series of `(x, y)` points (x typically seconds).
pub struct Series<'a> {
    /// Legend label.
    pub name: String,
    /// The data, in x order.
    pub points: &'a [(f64, f64)],
}

/// The categorical series palette, derived from the theme.
pub fn series_colors(t: &Theme) -> [Color; 6] {
    [t.accent, t.blue, t.pink, t.yellow, t.badge_info, t.red]
}

/// Render a multi-series time chart into `area`: x bounds span the data,
/// y auto-scales from zero with 15% headroom, `unit` labels the y max.
pub fn time_series(f: &mut Frame, area: Rect, t: &Theme, unit: &str, series: &[Series]) {
    let colors = series_colors(t);
    let (mut x_min, mut x_max, mut y_max) = (f64::MAX, f64::MIN, 0.0f64);
    for s in series {
        for (x, y) in s.points {
            x_min = x_min.min(*x);
            x_max = x_max.max(*x);
            y_max = y_max.max(*y);
        }
    }
    if x_min > x_max {
        (x_min, x_max) = (0.0, 1.0);
    }
    if (x_max - x_min) < 1.0 {
        x_max = x_min + 1.0;
    }
    let y_top = (y_max * 1.15).max(1.0);

    let datasets: Vec<Dataset> = series
        .iter()
        .enumerate()
        .map(|(i, s)| {
            Dataset::default()
                .name(s.name.clone())
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(colors[i % colors.len()]))
                .data(s.points)
        })
        .collect();

    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .bounds([x_min, x_max])
                .labels([
                    Span::styled(format!("{x_min:.0}s"), Style::default().fg(t.dim)),
                    Span::styled(format!("{x_max:.0}s"), Style::default().fg(t.dim)),
                ])
                .style(Style::default().fg(t.line2)),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, y_top])
                .labels([
                    Span::styled("0", Style::default().fg(t.dim)),
                    Span::styled(format!("{:.0}", y_top / 2.0), Style::default().fg(t.dim)),
                    Span::styled(format!("{y_top:.0} {unit}"), Style::default().fg(t.dim)),
                ])
                .style(Style::default().fg(t.line2)),
        )
        .legend_position(Some(LegendPosition::TopRight));
    f.render_widget(chart, area);
}

/// One horizontal capacity gauge: a label, a fill ratio, and the
/// `current/max` text shown inside the bar.
pub struct GaugeItem {
    /// Left-hand label (e.g. a node id).
    pub label: String,
    /// Fill fraction, clamped to 0..=1. The fill turns warning-yellow
    /// above 75% and red above 92%.
    pub ratio: f64,
    /// Usage text rendered inside the bar (e.g. `"38 MB / 512 MB · 7 bufs"`).
    pub text: String,
}

/// Stack horizontal bar gauges, one row each — the "current vs max"
/// allocation idiom (memory buffers, queue depth, disk).
pub fn gauges(f: &mut Frame, area: Rect, t: &Theme, items: &[GaugeItem]) {
    let label_w = items
        .iter()
        .map(|i| i.label.chars().count() as u16)
        .max()
        .unwrap_or(0)
        .min(area.width / 3);
    for (row, item) in items.iter().enumerate() {
        if row as u16 >= area.height {
            break;
        }
        let y = area.y + row as u16;
        f.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                item.label.clone(),
                Style::default().fg(t.fg),
            )),
            Rect::new(area.x, y, label_w, 1),
        );
        let ratio = item.ratio.clamp(0.0, 1.0);
        let fill = if ratio > 0.92 {
            t.red
        } else if ratio > 0.75 {
            t.yellow
        } else {
            t.accent
        };
        let bar = Rect::new(
            area.x + label_w + 1,
            y,
            area.width.saturating_sub(label_w + 1),
            1,
        );
        f.render_widget(
            Gauge::default()
                .ratio(ratio)
                .label(Span::styled(item.text.clone(), Style::default().fg(t.fg)))
                // Unfilled track = panel background so an empty gauge reads
                // empty; only the filled fraction shows the fill color.
                .gauge_style(Style::default().fg(fill).bg(t.bg))
                .use_unicode(true),
            bar,
        );
    }
}

/// One span in a [`flame`] graph: a label, a depth (0 = root), a value
/// (width weight, e.g. seconds), and an optional color override.
///
/// Spans are consumed in flattened pre-order (depth-first): each parent is
/// immediately followed by its descendants, the same invariant the icicle
/// trace format relies on. A child's `depth` is its parent's `depth + 1`;
/// sibling order is array order. Widths are derived from `value` shares of
/// the visible siblings, so callers don't need to pre-normalize.
pub struct FlameSpan {
    /// Label drawn inside the cell (truncated to fit).
    pub label: String,
    /// Stack depth: 0 is the root frame, deeper frames nest below it.
    pub depth: u16,
    /// Width weight (e.g. duration in seconds). Must be >= 0.
    pub value: f64,
    /// Optional cell color; falls back to a depth-cycled theme hue.
    pub color: Option<Color>,
}

/// Render a flamegraph (icicle) into `area`: stacked rows by depth, each
/// frame's width proportional to its `value` share of its siblings.
///
/// Row 0 (the root) spans the full width at the top; deeper frames stack
/// downward, each laid out within the horizontal extent of its parent. Cell
/// color defaults to a depth-cycled hue from [`series_colors`] but a span
/// may override it (matching apple-notes' per-stage colors). Frames narrower
/// than one cell are dropped. This is a stateless renderer; animate it by
/// feeding fresh [`FlameSpan`] slices per frame.
pub fn flame(f: &mut Frame, area: Rect, t: &Theme, spans: &[FlameSpan]) {
    if area.width == 0 || area.height == 0 || spans.is_empty() {
        return;
    }
    let colors = series_colors(t);
    // Each span resolves to a fractional [x0, x1] window within its parent.
    // Walk pre-order with a stack of open ancestors; a child claims a slice
    // of the parent's window proportional to its value share of siblings.
    struct Open {
        depth: u16,
        x0: f64,
        x1: f64,
        cursor: f64,
        siblings_total: f64,
    }
    // Sum of each span's direct children's values (its sibling-total source).
    let mut child_total = vec![0.0f64; spans.len()];
    let mut idx_stack: Vec<usize> = Vec::new();
    for (i, s) in spans.iter().enumerate() {
        while let Some(&p) = idx_stack.last() {
            if spans[p].depth < s.depth {
                break;
            }
            idx_stack.pop();
        }
        if let Some(&p) = idx_stack.last() {
            child_total[p] += s.value.max(0.0);
        }
        idx_stack.push(i);
    }
    let root_total: f64 = spans
        .iter()
        .filter(|s| s.depth == 0)
        .map(|s| s.value.max(0.0))
        .sum::<f64>()
        .max(f64::MIN_POSITIVE);

    let w = area.width as f64;
    let mut open: Vec<Open> = Vec::new();
    for (i, s) in spans.iter().enumerate() {
        while let Some(o) = open.last() {
            if o.depth < s.depth {
                break;
            }
            open.pop();
        }
        // Window this span occupies within its parent (or the full area).
        let (px0, px1, ptotal) = match open.last() {
            Some(p) => (p.cursor, p.x1, p.siblings_total),
            None => (0.0, w, root_total),
        };
        let share = if ptotal > 0.0 {
            s.value.max(0.0) / ptotal
        } else {
            0.0
        };
        let parent_span = match open.last() {
            Some(p) => p.x1 - p.x0,
            None => w,
        };
        let x0 = px0;
        let x1 = (x0 + share * parent_span).min(px1);
        // Advance the parent's cursor so the next sibling starts after us.
        if let Some(p) = open.last_mut() {
            p.cursor = x1;
        }
        open.push(Open {
            depth: s.depth,
            x0,
            x1,
            cursor: x0,
            siblings_total: child_total[i],
        });

        let row = area.y + s.depth;
        if row >= area.bottom() {
            continue;
        }
        let cell_x0 = area.x + x0.floor() as u16;
        let cell_w = (x1.floor() - x0.floor()).max(0.0) as u16;
        if cell_w == 0 {
            continue;
        }
        let cell_w = cell_w.min(area.right().saturating_sub(cell_x0));
        let color = s.color.unwrap_or_else(|| {
            let base = colors[s.depth as usize % colors.len()];
            mix(base, t.bg, (s.depth as f64 * 0.08).min(0.45))
        });
        let style = Style::default().bg(color);
        for cx in cell_x0..cell_x0 + cell_w {
            f.buffer_mut()[(cx, row)].set_char(' ').set_style(style);
        }
        let ink = contrast_ink(color, t);
        let label: String = s.label.chars().take(cell_w as usize).collect();
        for (off, ch) in label.chars().enumerate() {
            f.buffer_mut()[(cell_x0 + off as u16, row)]
                .set_char(ch)
                .set_style(Style::default().fg(ink).bg(color));
        }
    }
}

/// Pick readable ink (theme fg or bg) for text on a colored cell.
fn contrast_ink(bg: Color, t: &Theme) -> Color {
    match bg {
        Color::Rgb(r, g, b) => {
            // Rec. 601 luma; light cells get dark ink and vice versa.
            let luma = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
            if luma > 140.0 {
                t.bg
            } else {
                t.fg
            }
        }
        _ => t.fg,
    }
}

/// One vertical box-and-whisker for a named distribution.
pub struct BoxItem {
    /// Box label (e.g. a stage name), drawn under the whisker.
    pub label: String,
    /// Raw samples; quartiles are computed by linear interpolation.
    pub samples: Vec<f64>,
    /// Optional box color; falls back to the categorical palette.
    pub color: Option<Color>,
}

/// Five-number summary of a sample set: min, Q1, median, Q3, max.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiveNum {
    /// Minimum sample.
    pub min: f64,
    /// First quartile (25th percentile).
    pub q1: f64,
    /// Median (50th percentile).
    pub median: f64,
    /// Third quartile (75th percentile).
    pub q3: f64,
    /// Maximum sample.
    pub max: f64,
}

/// Compute the five-number summary of `samples` (linear-interpolated
/// percentiles). Returns `None` if empty.
pub fn five_num(samples: &[f64]) -> Option<FiveNum> {
    if samples.is_empty() {
        return None;
    }
    let mut v: Vec<f64> = samples.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pct = |p: f64| {
        let n = v.len();
        if n == 1 {
            return v[0];
        }
        let rank = p * (n - 1) as f64;
        let lo = rank.floor() as usize;
        let hi = rank.ceil() as usize;
        let frac = rank - lo as f64;
        v[lo] + (v[hi] - v[lo]) * frac
    };
    Some(FiveNum {
        min: v[0],
        q1: pct(0.25),
        median: pct(0.5),
        q3: pct(0.75),
        max: v[v.len() - 1],
    })
}

/// Render side-by-side vertical box-and-whisker plots into `area`.
///
/// One box per [`BoxItem`], laid out across the width; the shared y-axis
/// auto-scales across all boxes with a small headroom. Each box draws the
/// IQR (Q1..Q3) as a filled column, the median as a bright band, and
/// whiskers from min to max with caps. Colors come from the categorical
/// palette unless overridden. Stateless: feed fresh samples per frame for a
/// live distribution.
pub fn boxplot(f: &mut Frame, area: Rect, t: &Theme, items: &[BoxItem]) {
    if area.width == 0 || area.height < 2 || items.is_empty() {
        return;
    }
    let colors = series_colors(t);
    let stats: Vec<Option<FiveNum>> = items.iter().map(|i| five_num(&i.samples)).collect();
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for s in stats.iter().flatten() {
        lo = lo.min(s.min);
        hi = hi.max(s.max);
    }
    if lo > hi {
        (lo, hi) = (0.0, 1.0);
    }
    if (hi - lo).abs() < f64::EPSILON {
        hi = lo + 1.0;
    }
    let span = hi - lo;
    // Reserve the bottom row for labels.
    let plot_h = area.height.saturating_sub(1).max(1);
    let plot_bottom = area.y + plot_h; // first label row
    let n = items.len() as u16;
    let slot_w = (area.width / n).max(1);
    // Map a value to a screen row inside the plot band (higher = up).
    let row_of = |val: f64| -> u16 {
        let frac = ((val - lo) / span).clamp(0.0, 1.0);
        let from_bottom = (frac * (plot_h.saturating_sub(1)) as f64).round() as u16;
        plot_bottom.saturating_sub(1).saturating_sub(from_bottom)
    };

    for (i, item) in items.iter().enumerate() {
        let Some(s) = stats[i] else { continue };
        let color = item.color.unwrap_or(colors[i % colors.len()]);
        let slot_x = area.x + i as u16 * slot_w;
        let cx = slot_x + slot_w / 2;
        let box_w = (slot_w.saturating_sub(2)).clamp(1, 7);
        let box_x = cx.saturating_sub(box_w / 2);

        let (r_min, r_q1, r_med, r_q3, r_max) = (
            row_of(s.min),
            row_of(s.q1),
            row_of(s.median),
            row_of(s.q3),
            row_of(s.max),
        );
        let dim = Style::default().fg(t.line2);
        // Whiskers: vertical line from max to min through the box center.
        for y in r_max..=r_min {
            if y >= plot_bottom {
                break;
            }
            f.buffer_mut()[(cx.min(area.right().saturating_sub(1)), y)]
                .set_char('│')
                .set_style(dim);
        }
        // Whisker caps at min and max.
        for &(cap, w) in &[(r_max, box_w), (r_min, box_w)] {
            for off in 0..w {
                let x = box_x + off;
                if x < area.right() && cap < plot_bottom {
                    f.buffer_mut()[(x, cap)].set_char('─').set_style(dim);
                }
            }
        }
        // IQR box: filled column from Q3 (top) to Q1 (bottom).
        let fill = Style::default().bg(color);
        for y in r_q3..=r_q1 {
            if y >= plot_bottom {
                break;
            }
            for off in 0..box_w {
                let x = box_x + off;
                if x < area.right() {
                    f.buffer_mut()[(x, y)].set_char(' ').set_style(fill);
                }
            }
        }
        // Median band: bright accent line across the box.
        if r_med < plot_bottom {
            let med = Style::default().fg(t.fg).bg(color);
            for off in 0..box_w {
                let x = box_x + off;
                if x < area.right() {
                    f.buffer_mut()[(x, r_med)].set_char('━').set_style(med);
                }
            }
        }
        // Label, centered under the slot, truncated to slot width.
        let label: String = item.label.chars().take(slot_w as usize).collect();
        let lw = label.chars().count() as u16;
        let lx = slot_x + slot_w.saturating_sub(lw) / 2;
        for (off, ch) in label.chars().enumerate() {
            let x = lx + off as u16;
            if x < area.right() && plot_bottom < area.bottom() {
                f.buffer_mut()[(x, plot_bottom)]
                    .set_char(ch)
                    .set_style(Style::default().fg(t.dim));
            }
        }
    }
}
