use std::collections::BTreeMap;

use panel_kit::spec_plan::WebSpecPlan;
use panel_kit_tui::spec_plan::TuiSpecPlan;
use panel_kit_tui::TuiThemeDisposition;
use serde_json::{json, Value};

use super::leaf_pointers::schema_leaf_pointers;

/// Backend disposition for one strict spec leaf.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FieldDisposition {
    /// The backend plan applies the value directly.
    Applied(Value),
    /// The backend observes the concrete value at runtime, using this as fallback policy.
    ObservedAtRuntime(Value),
    /// The host/provider supplies the runtime value behind this binding.
    BoundByHost(Value),
    /// The backend cannot express the value exactly and names the native limitation.
    Approximated {
        /// Authored value being approximated.
        value: Value,
        /// Non-empty reason explaining the native backend limitation.
        reason: String,
    },
}

/// Derive web dispositions from the concrete web plan fields.
pub(crate) fn web_field_dispositions(plan: &WebSpecPlan) -> BTreeMap<String, FieldDisposition> {
    let value = serde_json::to_value(plan).expect("web plan is serializable");
    field_dispositions_from_value(&value, |_| None)
}

/// Derive TUI dispositions from the concrete TUI plan fields.
pub(crate) fn tui_field_dispositions(plan: &TuiSpecPlan) -> BTreeMap<String, FieldDisposition> {
    let value = json!({
        "spec_version": plan.spec_version,
        "id": plan.id,
        "layout": plan.layout,
        "surface": plan.surface,
        "chrome": plan.chrome,
        "input": plan.input,
        "glyphs": plan.glyphs,
        "theme": plan.theme,
        "persistence": plan.persistence,
        "panels": plan.panels,
    });

    field_dispositions_from_value(&value, |pointer| tui_approximation_reason(plan, pointer))
}

fn field_dispositions_from_value(
    value: &Value,
    approximation_reason: impl Fn(&str) -> Option<String>,
) -> BTreeMap<String, FieldDisposition> {
    schema_leaf_pointers(value)
        .into_iter()
        .filter_map(|pointer| {
            let leaf = value.pointer(&pointer)?.clone();
            let disposition = if let Some(reason) = approximation_reason(&pointer) {
                FieldDisposition::Approximated {
                    value: leaf,
                    reason,
                }
            } else if is_host_bound_pointer(&pointer) {
                FieldDisposition::BoundByHost(leaf)
            } else if pointer.starts_with("/surface/fallback_capabilities/") {
                FieldDisposition::ObservedAtRuntime(leaf)
            } else {
                FieldDisposition::Applied(leaf)
            };
            Some((pointer, disposition))
        })
        .collect()
}

fn is_host_bound_pointer(pointer: &str) -> bool {
    pointer == "/persistence/key"
        || (pointer.starts_with("/panels/")
            && (pointer.ends_with("/binding") || pointer.ends_with("/source/id")))
}

fn tui_approximation_reason(plan: &TuiSpecPlan, pointer: &str) -> Option<String> {
    let disposition = match pointer {
        "/theme/typography/family" => plan.resolved_theme.typography.family,
        "/theme/typography/body_size" => plan.resolved_theme.typography.body_size,
        "/theme/typography/body_line_height" => plan.resolved_theme.typography.body_line_height,
        "/theme/typography/label_size" => plan.resolved_theme.typography.label_size,
        "/theme/typography/label_weight" => plan.resolved_theme.typography.label_weight,
        "/theme/typography/label_tracking" => plan.resolved_theme.typography.label_tracking,
        "/theme/density/panel_radius" => plan.resolved_theme.density.panel_radius,
        "/theme/density/badge_radius" => plan.resolved_theme.density.badge_radius,
        "/theme/density/spacing_xs" => plan.resolved_theme.density.spacing_xs,
        "/theme/density/spacing_sm" => plan.resolved_theme.density.spacing_sm,
        "/theme/density/spacing_md" => plan.resolved_theme.density.spacing_md,
        _ => return None,
    };

    match disposition {
        TuiThemeDisposition::Approximated(reason) => Some(reason.to_owned()),
        TuiThemeDisposition::Applied => None,
    }
}
