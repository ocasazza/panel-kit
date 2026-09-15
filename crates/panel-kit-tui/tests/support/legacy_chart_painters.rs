#[path = "legacy_boxplot.rs"]
mod legacy_boxplot;
#[path = "legacy_flame.rs"]
mod legacy_flame;

pub use legacy_boxplot::boxplot;
pub use legacy_flame::flame;

use panel_kit_core::badge::Rgb;
use panel_kit_core::widgets::charts::{GaugeModel, SeriesView};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::symbols;
use ratatui::text::Span;
use ratatui::widgets::{Axis, Chart, Dataset, Gauge, GraphType, LegendPosition};
use ratatui::Frame;

use panel_kit_tui::ResolvedTuiTheme;

pub(super) fn mix(a: Color, b: Color, t: f64) -> Color {
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            let t = t.clamp(0.0, 1.0);
            let l = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
            Color::Rgb(l(ar, br), l(ag, bg), l(ab, bb))
        }
        _ => a,
    }
}

pub(super) fn series_colors(t: &ResolvedTuiTheme) -> [Color; 6] {
    [t.fg, t.blue, t.pink, t.yellow, t.badge_info, t.red]
}

pub(super) fn rgb_color((r, g, b): Rgb) -> Color {
    Color::Rgb(r, g, b)
}

pub fn time_series(
    f: &mut Frame,
    area: Rect,
    t: &ResolvedTuiTheme,
    unit: &str,
    series: &[SeriesView<'_>],
) {
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
                .name(s.name)
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

pub fn gauges(f: &mut Frame, area: Rect, t: &ResolvedTuiTheme, items: &[GaugeModel]) {
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
                item.label.as_str(),
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
            t.green
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
                .label(Span::styled(item.text.as_str(), Style::default().fg(t.fg)))
                .gauge_style(Style::default().fg(fill).bg(t.bg))
                .use_unicode(true),
            bar,
        );
    }
}
