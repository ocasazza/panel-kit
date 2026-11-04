//! Web mapping of the core badge detectors onto the theme's CSS variables.

use panel_kit_core::widgets::detect::{status_color as status_role, StatusColor};

/// The fixed accent blue the original mapping kept for draft/pending states.
/// The egui source had no theme counterpart for its accent blue, and
/// panel-kit's tokens have no dedicated bright-accent role either, so the
/// hex survives the port verbatim.
const DRAFT_BLUE: &str = "#3b9bff";

/// Status string → CSS colour variable. Case-insensitive; `None` for
/// statuses outside the mapping. Pass the result to [`crate::badge::Badge`]'s
/// `accent_color` for the status-pill treatment (semantic fill and text on
/// the shared badge chrome).
pub fn status_color(s: &str) -> Option<&'static str> {
    let role = status_role(s)?;
    Some(match role {
        StatusColor::Success => "var(--color-success)",
        StatusColor::Failure => "var(--color-failure)",
        StatusColor::Warning => "var(--color-warning)",
        StatusColor::Draft => DRAFT_BLUE,
        StatusColor::Neutral => "var(--color-text)",
    })
}
