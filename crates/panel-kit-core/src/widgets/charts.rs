//! Chart, gauge, flamegraph, and boxplot data models.

use serde::{Deserialize, Serialize};

use crate::badge::Rgb;

/// One named time-series of `(x, y)` points.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SeriesModel {
    /// Legend label.
    pub name: String,
    /// Owned points in x order.
    pub points: Vec<(f64, f64)>,
}

/// Borrowed time-series view supplied by a runtime provider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeriesView<'a> {
    /// Legend label.
    pub name: &'a str,
    /// Borrowed points in x order.
    pub points: &'a [(f64, f64)],
}

/// One horizontal capacity gauge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GaugeModel {
    /// Left-hand label.
    pub label: String,
    /// Fill fraction; painters clamp before drawing.
    pub ratio: f64,
    /// Usage text rendered with the bar.
    pub text: String,
}

/// One flattened preorder flamegraph span.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FlameSpanModel {
    /// Cell label.
    pub label: String,
    /// Stack depth; zero is the root frame.
    pub depth: u16,
    /// Width weight such as elapsed time.
    pub value: f64,
    /// Optional renderer-neutral cell color.
    pub color: Option<Rgb>,
}

/// Horizontal extent of one laid-out flamegraph span, in the caller's width
/// units (cells, percent, pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlameBand {
    /// Left edge.
    pub x0: f64,
    /// Right edge.
    pub x1: f64,
}

/// Icicle layout for preorder `spans` across `width`: roots share the full
/// width by value, and each span's children share its extent by value, left
/// to right. One band per span, in input order; the row is the span's depth.
pub fn flame_bands(spans: &[FlameSpanModel], width: f64) -> Vec<FlameBand> {
    struct Open {
        depth: u16,
        x1: f64,
        width: f64,
        cursor: f64,
        children_total: f64,
    }

    let mut children_totals = vec![0.0f64; spans.len()];
    let mut stack: Vec<usize> = Vec::new();
    for (index, span) in spans.iter().enumerate() {
        while stack.last().is_some_and(|&parent| spans[parent].depth >= span.depth) {
            stack.pop();
        }
        if let Some(&parent) = stack.last() {
            children_totals[parent] += flame_weight(span.value);
        }
        stack.push(index);
    }
    let root_total = spans
        .iter()
        .filter(|span| span.depth == 0)
        .map(|span| flame_weight(span.value))
        .sum::<f64>()
        .max(f64::MIN_POSITIVE);

    let mut open: Vec<Open> = Vec::new();
    let mut root_cursor = 0.0;
    spans
        .iter()
        .zip(children_totals)
        .map(|(span, children_total)| {
            while open.last().is_some_and(|parent| parent.depth >= span.depth) {
                open.pop();
            }
            let (cursor, right, total, extent) = match open.last() {
                Some(parent) => (parent.cursor, parent.x1, parent.children_total, parent.width),
                None => (root_cursor, width, root_total, width),
            };
            let share = if total > 0.0 {
                flame_weight(span.value) / total
            } else {
                0.0
            };
            let x0 = cursor;
            let x1 = (x0 + share * extent).min(right);
            match open.last_mut() {
                Some(parent) => parent.cursor = x1,
                None => root_cursor = x1,
            }
            open.push(Open {
                depth: span.depth,
                x1,
                width: x1 - x0,
                cursor: x0,
                children_total,
            });
            FlameBand { x0, x1 }
        })
        .collect()
}

/// A span's layout weight: negative and non-finite values weigh nothing.
fn flame_weight(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// One box-and-whisker distribution with raw samples.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BoxItemModel {
    /// Box label.
    pub label: String,
    /// Raw samples; summaries are computed when data changes.
    pub samples: Vec<f64>,
    /// Optional renderer-neutral box color.
    pub color: Option<Rgb>,
}

/// Borrowed boxplot summary view for hot paint paths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxItemView<'a> {
    /// Box label.
    pub label: &'a str,
    /// Precomputed five-number summary.
    pub summary: FiveNum,
    /// Optional renderer-neutral box color.
    pub color: Option<Rgb>,
}

/// Five-number summary of a sample set: min, Q1, median, Q3, max.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiveNum {
    /// Minimum sample.
    pub min: f64,
    /// First quartile.
    pub q1: f64,
    /// Median.
    pub median: f64,
    /// Third quartile.
    pub q3: f64,
    /// Maximum sample.
    pub max: f64,
}

/// Compute a five-number summary using linear-interpolated percentiles.
pub fn five_num(samples: &[f64]) -> Option<FiveNum> {
    if samples.is_empty() {
        return None;
    }

    let mut values = samples.to_vec();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    Some(FiveNum {
        min: values[0],
        q1: percentile(&values, 0.25),
        median: percentile(&values, 0.5),
        q3: percentile(&values, 0.75),
        max: values[values.len() - 1],
    })
}

/// Compute the linear-interpolated percentile for a sorted, non-empty slice.
fn percentile(values: &[f64], p: f64) -> f64 {
    if values.len() == 1 {
        return values[0];
    }

    let rank = p * (values.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    let frac = rank - lo as f64;

    values[lo] + (values[hi] - values[lo]) * frac
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_num_matches_tui_fixed_vector_parity() {
        assert_eq!(
            five_num(&[7.0, 1.0, 3.0, 9.0, 5.0]),
            Some(FiveNum {
                min: 1.0,
                q1: 3.0,
                median: 5.0,
                q3: 7.0,
                max: 9.0
            })
        );
        assert_eq!(
            five_num(&[10.0, 30.0, 20.0, 40.0]),
            Some(FiveNum {
                min: 10.0,
                q1: 17.5,
                median: 25.0,
                q3: 32.5,
                max: 40.0
            })
        );
        assert_eq!(five_num(&[]), None);
    }

    #[test]
    fn chart_models_keep_renderer_neutral_data() {
        let series = SeriesModel {
            name: "latency".into(),
            points: vec![(0.0, 10.0), (1.0, 12.5)],
        };
        let gauge = GaugeModel {
            label: "queue".into(),
            ratio: 0.75,
            text: "75%".into(),
        };
        let flame = FlameSpanModel {
            label: "root".into(),
            depth: 0,
            value: 12.0,
            color: None,
        };
        let box_item = BoxItemModel {
            label: "p95".into(),
            samples: vec![1.0, 2.0],
            color: Some((1, 2, 3)),
        };

        assert_eq!(series.points.len(), 2);
        assert_eq!(gauge.ratio, 0.75);
        assert_eq!(flame.depth, 0);
        assert_eq!(box_item.color, Some((1, 2, 3)));
    }
}
