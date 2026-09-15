//! Exact provider-manifest comparison for the repository canary.

use panel_kit_core::spec::{BackendKind, BindingManifest, PanelProviderDeclaration};

use crate::diagnostics::DiagnosticReport;
use crate::panel_compare::compare_panel_ids;

#[allow(dead_code)]
#[path = "../../../crates/panel-kit-tui/examples/workspace_canary.rs"]
mod workspace_canary;

/// Extract the live Rust TUI canary provider manifest by compiling its data module.
pub(crate) fn live_rust_provider_manifest() -> BindingManifest {
    workspace_canary::provider_manifest(BackendKind::Tui)
}

/// Compare Nix-authored provider declarations with Rust provider declarations.
pub(crate) fn compare_provider_manifests(
    workspace: &str,
    nix: &BindingManifest,
    rust: &BindingManifest,
) -> Result<(), DiagnosticReport> {
    let nix_ids = panel_ids(nix);
    let rust_ids = panel_ids(rust);
    compare_panel_ids(workspace, &nix_ids, &rust_ids)?;

    let mut lines = vec![
        format!("spec-parity: {workspace}"),
        "provider mismatch: provider declarations differ".to_owned(),
    ];

    for (index, (nix_provider, rust_provider)) in
        nix.panels.iter().zip(rust.panels.iter()).enumerate()
    {
        append_provider_field_diffs(&mut lines, index, nix_provider, rust_provider);
    }

    if lines.len() == 2 {
        Ok(())
    } else {
        Err(DiagnosticReport::new(lines))
    }
}

fn panel_ids(manifest: &BindingManifest) -> Vec<String> {
    manifest
        .panels
        .iter()
        .map(|provider| provider.panel_id.clone())
        .collect()
}

fn append_provider_field_diffs(
    lines: &mut Vec<String>,
    index: usize,
    nix: &PanelProviderDeclaration,
    rust: &PanelProviderDeclaration,
) {
    if nix.content_kind != rust.content_kind {
        lines.push(format!(
            "/panels/{index}/content_kind: nix={:?}, rust={:?}",
            nix.content_kind, rust.content_kind
        ));
    }

    if nix.binding != rust.binding {
        lines.push(format!(
            "/panels/{index}/binding: nix={:?}, rust={:?}",
            nix.binding, rust.binding
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_manifest_comes_from_workspace_canary_provider_source() {
        assert_eq!(
            live_rust_provider_manifest(),
            workspace_canary::provider_manifest(BackendKind::Tui),
            "spec-parity must compare against declarations owned by the executable canary source"
        );
    }
}
