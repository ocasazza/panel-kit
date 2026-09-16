//! Panel/provider parity comparison for repository canaries.

use std::collections::HashSet;

use panel_kit_core::spec::BindingManifest;
use panel_kit_core::SavedLayoutV2;

use crate::diagnostics::DiagnosticReport;

/// Extract panel IDs from a retained `mkLayout` JSON document.
pub(crate) fn panel_ids_from_saved_layout(text: &str) -> Result<Vec<String>, serde_json::Error> {
    let layout: SavedLayoutV2<String> = serde_json::from_str(text)?;

    Ok(layout.panels.into_iter().map(|panel| panel.kind).collect())
}

/// Extract provider panel IDs from a strict provider-manifest JSON document.
pub(crate) fn panel_ids_from_manifest_json(
    text: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let value = serde_json::from_str(text)?;
    let manifest = BindingManifest::from_json_value(value)?;

    Ok(manifest
        .panels
        .into_iter()
        .map(|provider| provider.panel_id)
        .collect())
}

/// Compare Nix-authored panel IDs with Rust provider IDs in both directions.
pub(crate) fn compare_panel_ids(
    workspace: &str,
    nix_ids: &[String],
    rust_ids: &[String],
) -> Result<(), DiagnosticReport> {
    let mut lines = vec![
        format!("spec-parity: {workspace}"),
        "provider mismatch: panel manifest differs".to_owned(),
    ];

    append_ordered_panel_diffs(&mut lines, nix_ids, rust_ids);
    append_set_summary(&mut lines, "missing from Nix", rust_ids, nix_ids);
    append_set_summary(&mut lines, "missing from Rust", nix_ids, rust_ids);

    let report = DiagnosticReport::new(lines);
    if report_has_panel_drift(nix_ids, rust_ids) {
        Err(report)
    } else {
        Ok(())
    }
}

/// Append ordered index-level JSON-pointer differences.
fn append_ordered_panel_diffs(lines: &mut Vec<String>, nix_ids: &[String], rust_ids: &[String]) {
    let max_len = nix_ids.len().max(rust_ids.len());

    for index in 0..max_len {
        let nix = nix_ids.get(index);
        let rust = rust_ids.get(index);
        if nix != rust {
            lines.push(format!(
                "/panels/{index}/id: nix={}, rust={}",
                format_optional_id(nix),
                format_optional_id(rust)
            ));
        }
    }

    if nix_ids.len() != rust_ids.len() {
        lines.push(format!(
            "/panels/length: nix={}, rust={}",
            nix_ids.len(),
            rust_ids.len()
        ));
    }
}

/// Append a deterministic bidirectional set summary in source order.
fn append_set_summary(
    lines: &mut Vec<String>,
    label: &str,
    expected: &[String],
    actual: &[String],
) {
    let actual_set: HashSet<&str> = actual.iter().map(String::as_str).collect();
    let missing: Vec<&str> = expected
        .iter()
        .map(String::as_str)
        .filter(|id| !actual_set.contains(id))
        .collect();

    if !missing.is_empty() {
        lines.push(format!("{label}: {}", missing.join(", ")));
    }
}

/// Format a present-or-missing panel ID for operator diagnostics.
fn format_optional_id(id: Option<&String>) -> String {
    match id {
        Some(id) => format!("{id:?}"),
        None => "<missing>".to_owned(),
    }
}

/// Return true when either ordered values or set membership differs.
fn report_has_panel_drift(nix_ids: &[String], rust_ids: &[String]) -> bool {
    nix_ids != rust_ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_manifest::live_rust_provider_manifest;

    /// Extract live executable canary provider IDs for historical drift tests.
    fn live_rust_provider_ids() -> Vec<String> {
        live_rust_provider_manifest()
            .panels
            .into_iter()
            .map(|provider| provider.panel_id)
            .collect()
    }

    #[test]
    fn spec_parity_rejects_live_seven_vs_nine_canary() {
        let nix = panel_ids_from_saved_layout(include_str!("../fixtures/live-seven-layout.json"))
            .expect("captured mkLayout canary decodes");
        let rust = live_rust_provider_ids();

        let report = compare_panel_ids("workspace-canary", &nix, &rust)
            .expect_err("historical seven-vs-nine drift must be rejected");

        assert_eq!(
            report.to_string(),
            include_str!("../fixtures/initial-red.txt")
        );
        assert!(report
            .to_string()
            .contains("missing from Nix: Flame, Distribution"));
        assert!(report
            .to_string()
            .contains("/panels/2/id: nix=\"Notes\", rust=\"Flame\""));
        assert!(report
            .to_string()
            .contains("/panels/7/id: nix=<missing>, rust=\"Distribution\""));
    }

    #[test]
    fn hand_fixed_nine_panel_layout_matches_provider_manifest() {
        let nix = panel_ids_from_saved_layout(include_str!("../fixtures/fixed-nine-layout.json"))
            .expect("hand-fixed mkLayout canary decodes");
        let rust = live_rust_provider_ids();

        compare_panel_ids("workspace-canary", &nix, &rust)
            .expect("matching panel IDs should pass without drift");
    }

    #[test]
    fn provider_binding_drift_reports_missing_and_unreferenced_ids() {
        let spec = vec!["Workspace".to_owned(), "Nodes".to_owned()];
        let providers = vec!["Workspace".to_owned(), "Flame".to_owned()];

        let report = compare_panel_ids("workspace-canary", &spec, &providers)
            .expect_err("bidirectional provider drift must be rejected");

        assert!(report
            .to_string()
            .contains("/panels/1/id: nix=\"Nodes\", rust=\"Flame\""));
        assert!(report.to_string().contains("missing from Nix: Flame"));
        assert!(report.to_string().contains("missing from Rust: Nodes"));
    }
}
