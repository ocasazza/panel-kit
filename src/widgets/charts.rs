//! Semantic chart painters for borrowed core chart models.

use dioxus::prelude::*;
use panel_kit_core::badge::Rgb;
use panel_kit_core::widgets::charts::{BoxItemView, FlameSpanModel, GaugeModel, SeriesView};

/// Categorical series colours, in the same order the terminal chart painters
/// cycle through (`fg, blue, pink, yellow, badge-info, red`), expressed as
/// theme variables so a retheme restyles the charts without a repaint.
const SERIES_COLORS: [&str; 6] = [
    "var(--fg)",
    "var(--blue)",
    "var(--pink)",
    "var(--yellow)",
    "var(--badge-info)",
    "var(--red)",
];

/// Internal chart coordinate space; the SVG stretches it to its box.
const PLOT_W: f64 = 320.0;
const PLOT_H: f64 = 140.0;

fn percent(ratio: f64) -> u32 {
    (ratio.clamp(0.0, 1.0) * 100.0).round() as u32
}

fn rgb_style(color: Option<Rgb>) -> String {
    color
        .map(|(r, g, b)| format!("--widget-c:rgb({r},{g},{b});"))
        .unwrap_or_default()
}

fn points_text(points: &[(f64, f64)]) -> String {
    points
        .iter()
        .map(|(x, y)| format!("{x}:{y}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format an axis or summary value without a trailing `.00`.
fn value_text(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

/// Format a value with its declared unit, skipping the space when there is none.
fn unit_text(value: f64, unit: &str) -> String {
    if unit.is_empty() {
        value_text(value)
    } else {
        format!("{} {unit}", value_text(value))
    }
}

/// Shared y span across every point, widened when the data is flat.
fn y_span(series: &[SeriesView<'_>]) -> Option<(f64, f64)> {
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for item in series {
        for (_, y) in item.points {
            lo = lo.min(*y);
            hi = hi.max(*y);
        }
    }
    if lo > hi {
        return None;
    }
    if (hi - lo).abs() < f64::EPSILON {
        hi = lo + 1.0;
    }

    Some((lo, hi))
}

/// Shared x span across every point, widening a flat axis like the y span.
fn x_span(series: &[SeriesView<'_>]) -> Option<(f64, f64)> {
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for item in series {
        for (x, _) in item.points {
            lo = lo.min(*x);
            hi = hi.max(*x);
        }
    }
    if lo > hi {
        return None;
    }
    if (hi - lo).abs() < f64::EPSILON {
        hi = lo + 1.0;
    }

    Some((lo, hi))
}

/// Project one point into the chart's coordinate space (y grows downward).
fn project(x: f64, y: f64, xs: (f64, f64), ys: (f64, f64)) -> (f64, f64) {
    let px = (x - xs.0) / (xs.1 - xs.0) * PLOT_W;
    let py = (1.0 - (y - ys.0) / (ys.1 - ys.0)) * PLOT_H;

    (px, py)
}

/// One series' projected polyline, skipping empty series.
fn polyline_points(item: &SeriesView<'_>, xs: (f64, f64), ys: (f64, f64)) -> String {
    item.points
        .iter()
        .map(|(x, y)| {
            let (px, py) = project(*x, *y, xs, ys);
            format!("{px:.2},{py:.2}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Paint a borrowed time-series view as an inline SVG line chart.
///
/// Every series shares one y span so lines are comparable, matching the
/// terminal chart's shared bounds.
pub fn time_series(series: &[SeriesView<'_>], unit: &str) -> Element {
    let label = format!("time series chart: {} series, unit {unit}", series.len());
    let bounds = x_span(series).zip(y_span(series));

    rsx! {
        figure { class: "pk-widget pk-chart pk-time-series",
            figcaption { "Time series ({unit})" }
            div { class: "pk-chart-plot", role: "img", aria_label: "{label}",
                span { class: "pk-chart-scale pk-chart-scale-max",
                    "{bounds.map_or_else(String::new, |(_, ys)| unit_text(ys.1, unit))}"
                }
                span { class: "pk-chart-scale pk-chart-scale-min",
                    "{bounds.map_or_else(String::new, |(_, ys)| unit_text(ys.0, unit))}"
                }
                // role="img" on the wrapper already hides the marks from
                // assistive tech; the SVG namespace has no aria_hidden attr.
                svg { class: "pk-chart-svg", view_box: "0 0 {PLOT_W} {PLOT_H}",
                    preserve_aspect_ratio: "none",
                    for (index, item) in series.iter().enumerate() {
                        polyline {
                            class: "pk-chart-line",
                            points: "{bounds.map_or_else(String::new, |(xs, ys)| polyline_points(item, xs, ys))}",
                            fill: "none",
                            stroke: "{SERIES_COLORS[index % SERIES_COLORS.len()]}",
                            stroke_width: "1.5",
                            vector_effect: "non-scaling-stroke",
                        }
                    }
                }
            }
            div { class: "pk-chart-xaxis", aria_hidden: "true",
                span { "{bounds.map_or_else(String::new, |(xs, _)| value_text(xs.0))}" }
                span { "{bounds.map_or_else(String::new, |(xs, _)| value_text(xs.1))}" }
            }
            ul { class: "pk-chart-legend", role: "list", aria_label: "series legend",
                for (index, item) in series.iter().enumerate() {
                    li { class: "pk-chart-legend-item", aria_label: "series {item.name}",
                        span { class: "pk-chart-swatch", aria_hidden: "true",
                            style: "background:{SERIES_COLORS[index % SERIES_COLORS.len()]};"
                        }
                        span { class: "pk-chart-legend-name", "{item.name}" }
                    }
                }
            }
            ul { class: "pk-chart-data pk-visually-hidden",
                for item in series.iter() {
                    li { "series {item.name}: {points_text(item.points)}" }
                }
            }
        }
    }
}

/// Paint borrowed gauge models as web progress bars.
pub fn gauges(items: &[GaugeModel]) -> Element {
    rsx! {
        section { class: "pk-widget pk-gauges", role: "group", aria_label: "gauges",
            for item in items.iter() {
                div { class: "pk-gauge", role: "progressbar", aria_label: "gauge {item.label} {percent(item.ratio)}%", aria_valuemin: "0", aria_valuemax: "100", aria_valuenow: "{percent(item.ratio)}",
                    span { class: "pk-gauge-label", "{item.label}" }
                    span { class: "pk-gauge-track",
                        span { class: "pk-gauge-fill", style: "width:{percent(item.ratio)}%;" }
                    }
                    span { class: "pk-gauge-value", "{item.text}" }
                }
            }
        }
    }
}

/// Paint borrowed flamegraph spans as a semantic tree of stack frames.
///
/// Each frame's bar width is its value as a share of the summed root values.
pub fn flamegraph(spans: &[FlameSpanModel]) -> Element {
    let total: f64 = spans.iter().filter(|span| span.depth == 0).map(|span| span.value).sum();
    let frame_style = |span: &FlameSpanModel| {
        let share = if total > 0.0 { (span.value / total).clamp(0.0, 1.0) } else { 0.0 };
        format!(
            "{}--flame-depth:{};--flame-w:{:.1}%;",
            rgb_style(span.color),
            span.depth,
            share * 100.0
        )
    };
    rsx! {
        section { class: "pk-widget pk-flamegraph", role: "tree", aria_label: "flamegraph",
            for span in spans.iter() {
                div { class: "pk-flame-frame", role: "treeitem", "data-depth": "{span.depth}", aria_label: "root depth {span.depth} value {span.value}", style: "{frame_style(span)}",
                    span { class: "pk-flame-label", "{span.label}" }
                    span { class: "pk-flame-value", "{span.value}" }
                }
            }
        }
    }
}

/// One box row's geometry as track percentages over the shared scale.
struct BoxRow {
    min: f64,
    q1: f64,
    median: f64,
    q3: f64,
    max: f64,
}

impl BoxRow {
    fn style(&self) -> String {
        format!(
            "--box-min:{:.3}%;--box-q1:{:.3}%;--box-median:{:.3}%;--box-q3:{:.3}%;--box-max:{:.3}%;",
            self.min, self.q1, self.median, self.q3, self.max
        )
    }

    fn summary(item: &BoxItemView<'_>) -> String {
        let s = item.summary;
        format!(
            "min {}, q1 {}, median {}, q3 {}, max {}",
            value_text(s.min),
            value_text(s.q1),
            value_text(s.median),
            value_text(s.q3),
            value_text(s.max)
        )
    }
}

/// Shared value span across every box, widened when the data is flat.
fn box_span(items: &[BoxItemView<'_>]) -> Option<(f64, f64)> {
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for item in items {
        lo = lo.min(item.summary.min);
        hi = hi.max(item.summary.max);
    }
    if lo > hi {
        return None;
    }
    if (hi - lo).abs() < f64::EPSILON {
        hi = lo + 1.0;
    }

    Some((lo, hi))
}

/// Place one five-number summary on the shared span, in track percentages.
fn box_row(summary: &panel_kit_core::widgets::charts::FiveNum, span: (f64, f64)) -> BoxRow {
    let place = |value: f64| ((value - span.0) / (span.1 - span.0) * 100.0).clamp(0.0, 100.0);

    BoxRow {
        min: place(summary.min),
        q1: place(summary.q1),
        median: place(summary.median),
        q3: place(summary.q3),
        max: place(summary.max),
    }
}

/// Paint borrowed boxplot summaries as one shared-scale box-and-whisker row
/// per distribution, without sorting or cloning raw samples.
pub fn boxplot(items: &[BoxItemView<'_>]) -> Element {
    let label = format!("boxplot chart: {} distributions", items.len());
    let rows: Vec<BoxPaint> = items
        .iter()
        .enumerate()
        .map(|(index, item)| BoxPaint::new(item, index, box_span(items)))
        .collect();

    rsx! {
        figure { class: "pk-widget pk-chart pk-boxplot",
            figcaption { "Distribution" }
            ul { class: "pk-box-list", role: "list", aria_label: "{label}",
                for row in rows.iter() {
                    li { class: "pk-box-item", style: "{row.style}",
                        span { class: "pk-box-label", "{row.label}" }
                        span { class: "pk-box-track", role: "img", aria_label: "{row.aria_label}",
                            span { class: "pk-box-whisker", aria_hidden: "true" }
                            span { class: "pk-box-range", aria_hidden: "true" }
                            span { class: "pk-box-median", aria_hidden: "true" }
                        }
                        span { class: "pk-box-summary", aria_hidden: "true", "{row.summary}" }
                    }
                }
            }
        }
    }
}

/// One boxplot row's precomputed paint: labels, geometry, and inline vars.
struct BoxPaint {
    label: String,
    summary: String,
    aria_label: String,
    style: String,
}

impl BoxPaint {
    fn new(item: &BoxItemView<'_>, index: usize, span: Option<(f64, f64)>) -> Self {
        let summary = BoxRow::summary(item);
        let ink = item.color.map_or_else(
            || SERIES_COLORS[index % SERIES_COLORS.len()].to_string(),
            |(r, g, b)| format!("rgb({r},{g},{b})"),
        );
        let style = span.map_or_else(String::new, |s| {
            format!("{}--box-c:{ink};", box_row(&item.summary, s).style())
        });

        Self {
            label: item.label.to_string(),
            aria_label: format!("box plot {}: {summary}", item.label),
            summary,
            style,
        }
    }
}
