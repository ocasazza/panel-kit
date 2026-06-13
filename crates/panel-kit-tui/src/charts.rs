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
                .label(Span::styled(
                    item.text.clone(),
                    Style::default().fg(t.fg),
                ))
                .gauge_style(Style::default().fg(fill).bg(t.line))
                .use_unicode(true),
            bar,
        );
    }
}
