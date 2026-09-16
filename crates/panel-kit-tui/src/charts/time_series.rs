use panel_kit_core::widgets::charts::SeriesView;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::symbols;
use ratatui::text::Span;
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, LegendPosition};
use ratatui::Frame;

use crate::ResolvedTuiTheme;

use super::series_colors;

#[derive(Clone, Copy)]
struct TimeBounds {
    x_min: f64,
    x_max: f64,
    y_top: f64,
}

/// Render a multi-series time chart into `area`.
///
/// The x bounds span the data, the y axis auto-scales from zero with 15%
/// headroom, and `unit` labels the y-axis maximum.
pub fn time_series(
    f: &mut Frame,
    area: Rect,
    t: &ResolvedTuiTheme,
    unit: &str,
    series: &[SeriesView<'_>],
) {
    let colors = series_colors(t);
    let bounds = time_bounds(series);
    let chart = Chart::new(time_series_datasets(series, &colors))
        .x_axis(time_x_axis(bounds, t))
        .y_axis(time_y_axis(bounds, unit, t))
        .legend_position(Some(LegendPosition::TopRight));

    f.render_widget(chart, area);
}

fn time_bounds(series: &[SeriesView<'_>]) -> TimeBounds {
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

    TimeBounds {
        x_min,
        x_max,
        y_top: (y_max * 1.15).max(1.0),
    }
}

fn time_series_datasets<'a>(series: &'a [SeriesView<'a>], colors: &[Color]) -> Vec<Dataset<'a>> {
    series
        .iter()
        .enumerate()
        .map(|(index, s)| {
            Dataset::default()
                .name(s.name)
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(colors[index % colors.len()]))
                .data(s.points)
        })
        .collect()
}

fn time_x_axis(bounds: TimeBounds, t: &ResolvedTuiTheme) -> Axis<'static> {
    Axis::default()
        .bounds([bounds.x_min, bounds.x_max])
        .labels([
            Span::styled(format!("{:.0}s", bounds.x_min), Style::default().fg(t.dim)),
            Span::styled(format!("{:.0}s", bounds.x_max), Style::default().fg(t.dim)),
        ])
        .style(Style::default().fg(t.line2))
}

fn time_y_axis(bounds: TimeBounds, unit: &str, t: &ResolvedTuiTheme) -> Axis<'static> {
    Axis::default()
        .bounds([0.0, bounds.y_top])
        .labels([
            Span::styled("0", Style::default().fg(t.dim)),
            Span::styled(
                format!("{:.0}", bounds.y_top / 2.0),
                Style::default().fg(t.dim),
            ),
            Span::styled(
                format!("{:.0} {unit}", bounds.y_top),
                Style::default().fg(t.dim),
            ),
        ])
        .style(Style::default().fg(t.line2))
}
