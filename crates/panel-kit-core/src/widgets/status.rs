//! Status value model for dots, table cells, and summaries.

use serde::{Deserialize, Serialize};

use crate::badge::Rgb;

/// Coarse status state used by semantic specs and backend plans.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum StatusState {
    /// Successful/healthy state.
    Ok,
    /// Warning or degraded state.
    Warning,
    /// Error or failed state.
    Error,
    /// Informational/neutral state.
    Info,
    /// Unknown/pending state.
    Unknown,
}

/// One labeled status value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StatusModel {
    /// Status label.
    pub label: String,
    /// Coarse status state.
    pub state: StatusState,
    /// Renderer-neutral display color.
    pub color: Rgb,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_model_carries_label_state_and_color() {
        let model = StatusModel {
            label: "build".into(),
            state: StatusState::Warning,
            color: (255, 200, 0),
        };

        assert_eq!(model.label, "build");
        assert_eq!(model.state, StatusState::Warning);
        assert_eq!(model.color, (255, 200, 0));
    }
}
