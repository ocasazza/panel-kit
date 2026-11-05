//! Exact provider-manifest comparison for the repository canary.

use panel_kit_core::spec::{BackendKind, BindingManifest, PanelProviderDeclaration};

use crate::diagnostics::DiagnosticReport;
use crate::panel_compare::compare_panel_ids;

#[allow(dead_code)]
#[path = "../../../examples/support/canary_content.rs"]
mod canary_content;

/// Backends whose executable canary renders `workspace-canary` with the
/// shared content providers.
const CANARY_BACKENDS: [BackendKind; 3] =
    [BackendKind::Tui, BackendKind::BrowserTui, BackendKind::Bevy];

/// The live provider manifest of every backend canary, compiled from the
/// shared content module.
pub(crate) fn live_rust_provider_manifests() -> Vec<BindingManifest> {
    CANARY_BACKENDS
        .into_iter()
        .map(canary_content::provider_manifest)
        .collect()
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

