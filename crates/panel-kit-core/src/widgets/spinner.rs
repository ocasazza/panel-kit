//! Spinner model shared by renderer-specific painters.

use serde::{Deserialize, Serialize};

/// Braille ring frames used by backend spinner painters.
pub const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Authored spinner configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SpinnerModel {
    /// Optional label drawn beside the spinner.
    pub label: Option<String>,
}

/// Select a spinner frame for a monotonic tick counter.
pub fn spinner_frame(tick: u64) -> &'static str {
    FRAMES[(tick as usize) % FRAMES.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_frame_wraps_tick_counter() {
        assert_eq!(spinner_frame(0), "⠋");
        assert_eq!(spinner_frame(FRAMES.len() as u64), "⠋");
    }

    #[test]
    fn spinner_model_carries_optional_label() {
        let spinner = SpinnerModel { label: Some("loading".into()) };

        assert_eq!(spinner.label.as_deref(), Some("loading"));
    }
}
