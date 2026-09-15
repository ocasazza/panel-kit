//! Private field-drift checks used by the spec-parity binary.

mod backend_disposition;
#[cfg(test)]
mod fixtures;
mod leaf_pointers;
#[cfg(test)]
mod tests;

use std::collections::HashSet;

use panel_kit_core::spec::{BackendKind, BindingManifest, WorkspaceSpec};
use serde_json::Value;

pub(crate) use backend_disposition::{tui_field_dispositions, web_field_dispositions};
#[cfg(test)]
pub(crate) use backend_disposition::FieldDisposition;
pub(crate) use leaf_pointers::schema_leaf_pointers;

use crate::diagnostics::DiagnosticReport;

/// Reject a value missing leaves that the Rust schema marks as required.
pub(crate) fn compare_required_schema_leaves(
    nix_output: &Value,
    required_pointers: &[&str],
) -> Result<(), DiagnosticReport> {
    let missing: Vec<String> = required_pointers
        .iter()
        .filter(|pointer| nix_output.pointer(pointer).is_none())
        .map(|pointer| format!("{pointer}: required by Rust schema, missing from Nix output"))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(DiagnosticReport::new(missing))
    }
}

/// Require a concrete backend plan to account for every decoded schema leaf.
pub(crate) fn require_backend_field_dispositions(
    workspace: &str,
    backend: &str,
    schema_leaf_pointers: &[&str],
    lowered_leaf_pointers: &[&str],
) -> Result<(), DiagnosticReport> {
    let lowered: HashSet<&str> = lowered_leaf_pointers.iter().copied().collect();
    let missing: Vec<String> = schema_leaf_pointers
        .iter()
        .filter(|pointer| !lowered.contains(**pointer))
        .map(|pointer| format!("{workspace} × {backend}: backend did not lower {pointer}"))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(DiagnosticReport::new(missing))
    }
}

/// Check concrete web and TUI plans against every leaf in a decoded spec value.
pub(crate) fn check_workspace_plan_coverage(
    workspace: &str,
    spec: &WorkspaceSpec,
    providers: &BindingManifest,
    spec_value: &Value,
) -> Result<(), DiagnosticReport> {
    let schema_leaf_pointers = schema_leaf_pointers(spec_value);
    let pointer_refs: Vec<&str> = schema_leaf_pointers.iter().map(String::as_str).collect();
    compare_required_schema_leaves(spec_value, &pointer_refs)?;

    let web_resolved = spec
        .clone()
        .resolve(&backend_manifest(providers, BackendKind::Web))
        .map_err(|error| DiagnosticReport::new(vec![error.to_string().trim().to_owned()]))?;
    let web_plan = panel_kit::spec_plan::lower_spec(&web_resolved);
    let web_dispositions = web_field_dispositions(&web_plan);
    let web_refs: Vec<&str> = web_dispositions.keys().map(String::as_str).collect();
    require_backend_field_dispositions(workspace, "web-wasm", &pointer_refs, &web_refs)?;

    let tui_resolved = spec
        .clone()
        .resolve(&backend_manifest(providers, BackendKind::Tui))
        .map_err(|error| DiagnosticReport::new(vec![error.to_string().trim().to_owned()]))?;
    let tui_plan = panel_kit_tui::spec_plan::lower_spec(&tui_resolved);
    let tui_dispositions = tui_field_dispositions(&tui_plan);
    let tui_refs: Vec<&str> = tui_dispositions.keys().map(String::as_str).collect();
    require_backend_field_dispositions(workspace, "tui-native", &pointer_refs, &tui_refs)
}

fn backend_manifest(providers: &BindingManifest, backend: BackendKind) -> BindingManifest {
    BindingManifest {
        backend,
        panels: providers.panels.clone(),
    }
}
