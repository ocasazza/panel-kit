//! Semantic chart painters for borrowed core chart models.

use dioxus::prelude::*;
use panel_kit_core::badge::Rgb;
use panel_kit_core::widgets::charts::{BoxItemView, FlameSpanModel, GaugeModel, SeriesView};

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

/// Paint a borrowed time-series view as semantic SVG-free chart markup.
pub fn time_series(series: &[SeriesView<'_>], unit: &str) -> Element {
    let label = format!("time series chart: {} series, unit {unit}", series.len());
    rsx! {
        figure { class: "pk-widget pk-chart pk-time-series", role: "img", aria_label: "{label}",
            figcaption { "Time series ({unit})" }
            ul { class: "pk-series-list",
                for item in series.iter() {
                    li { class: "pk-series", aria_label: "series {item.name}",
                        span { class: "pk-series-name", "{item.name}" }
                        span { class: "pk-series-points", "{points_text(item.points)}" }
                    }
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
pub fn flamegraph(spans: &[FlameSpanModel]) -> Element {
    rsx! {
        section { class: "pk-widget pk-flamegraph", role: "tree", aria_label: "flamegraph",
            for span in spans.iter() {
                div { class: "pk-flame-frame", role: "treeitem", "data-depth": "{span.depth}", aria_label: "{span.label} depth {span.depth} value {span.value}", style: "{rgb_style(span.color)}",
                    span { class: "pk-flame-label", "{span.label}" }
                    span { class: "pk-flame-value", "{span.value}" }
                }
            }
        }
    }
}

/// Paint borrowed boxplot summaries without sorting or cloning raw samples.
pub fn boxplot(items: &[BoxItemView<'_>]) -> Element {
    let label = format!("boxplot chart: {} distributions", items.len());
    rsx! {
        figure { class: "pk-widget pk-boxplot", role: "img", aria_label: "{label}",
            figcaption { "Distribution" }
            ul { class: "pk-box-list",
                for item in items.iter() {
                    li { class: "pk-box-item", style: "{rgb_style(item.color)}",
                        span { class: "pk-box-label", "{item.label}" }
                        span { class: "pk-box-summary", "min {item.summary.min}, q1 {item.summary.q1}, median {item.summary.median}, q3 {item.summary.q3}, max {item.summary.max}" }
                    }
                }
            }
        }
    }
}
