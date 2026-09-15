//! Meter data and fill-bar math.

use serde::{Deserialize, Serialize};

use crate::badge::Rgb;

/// A single labeled meter value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MeterModel {
    /// Meter label.
    pub label: String,
    /// Fill fraction; drawing code clamps to `0.0..=1.0`.
    pub ratio: f64,
    /// Human-readable value text.
    pub text: String,
    /// Optional renderer-neutral meter color.
    pub color: Option<Rgb>,
}

/// Return a horizontal fill bar `width` cells wide.
pub fn bar(frac: f64, width: usize) -> String {
    let filled = (frac.clamp(0.0, 1.0) * width as f64).round() as usize;
    let filled = filled.min(width);

    "█".repeat(filled) + &"░".repeat(width - filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_matches_tui_rounding_and_clamping() {
        assert_eq!(bar(0.0, 4), "░░░░");
        assert_eq!(bar(0.5, 4), "██░░");
        assert_eq!(bar(0.76, 4), "███░");
        assert_eq!(bar(-1.0, 4), "░░░░");
        assert_eq!(bar(2.0, 4), "████");
    }

    #[test]
    fn meter_model_carries_semantic_fields() {
        let model = MeterModel { label: "cpu".into(), ratio: 0.42, text: "42%".into(), color: Some((10, 20, 30)) };

        assert_eq!(model.label, "cpu");
        assert_eq!(model.color, Some((10, 20, 30)));
    }
}
