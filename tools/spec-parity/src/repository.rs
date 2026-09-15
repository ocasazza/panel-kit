//! Repository-local default `spec-parity` check orchestration.

use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use panel_kit_core::spec::{BackendKind, BindingManifest, WorkspaceSpec};
use serde_json::Value;

use crate::field_coverage::check_workspace_plan_coverage;
use crate::layout_roundtrip::round_trip_saved_layout;
use crate::provider_manifest::{compare_provider_manifests, live_rust_provider_manifest};
use crate::schema::{check_committed_schema, decode_workspace_spec_text};

const SCHEMA_PATH_ENV: &str = "SPEC_PARITY_SCHEMA_PATH";
const WORKSPACE_CANARY_JSON_ENV: &str = "SPEC_PARITY_WORKSPACE_CANARY_JSON";
const WORKSPACE_REFERENCE_JSON_ENV: &str = "SPEC_PARITY_WORKSPACE_REFERENCE_JSON";
const WEB_WORKSPACE_JSON_ENV: &str = "SPEC_PARITY_WEB_WORKSPACE_JSON";
const WEB_WORKSPACE_PROVIDER_MANIFEST_JSON_ENV: &str =
    "SPEC_PARITY_WEB_WORKSPACE_PROVIDER_MANIFEST_JSON";
const FIXED_LAYOUT_JSON_ENV: &str = "SPEC_PARITY_FIXED_LAYOUT_JSON";

/// Run the repository-local schema, plan-coverage, layout, and drift checks.
pub(crate) fn check_repository() -> Result<(), Box<dyn Error>> {
    check_committed_schema(&committed_schema_path())?;
    check_workspace_spec_plan_fixture()?;
    check_workspace_canary_spec()?;
    check_web_workspace_spec()?;
    round_trip_saved_layout(&fixed_layout_json()?)?;

    Ok(())
}

/// Evaluate the authoritative Nix WorkspaceSpec canary to JSON.
fn evaluate_workspace_canary_spec() -> Result<String, Box<dyn Error>> {
    json_text_from_env_or_else(WORKSPACE_CANARY_JSON_ENV, evaluate_workspace_canary_spec_with_nix)
}

/// Prove the authoritative canary spec matches the Rust provider manifest.
fn check_workspace_canary_spec() -> Result<(), Box<dyn Error>> {
    let spec_json = evaluate_workspace_canary_spec()?;
    let rust_manifest = live_rust_provider_manifest();

    check_workspace_spec_against_manifest("workspace-canary", &spec_json, &rust_manifest)
}

/// Evaluate the full WorkspaceSpec fixture and prove concrete backend plan coverage.
fn check_workspace_spec_plan_fixture() -> Result<(), Box<dyn Error>> {
    let spec_json = evaluate_workspace_spec_fixture()?;
    let spec_value: Value = serde_json::from_str(&spec_json)?;
    let spec = decode_workspace_spec_text(&spec_json)?;
    let providers = binding_manifest_for(&spec, BackendKind::Web);

    check_workspace_plan_coverage("workspace-spec-reference", &spec, &providers, &spec_value)?;
    Ok(())
}

/// Evaluate the schema-driven `mkWorkspaceSpec` fixture to normalized JSON.
fn evaluate_workspace_spec_fixture() -> Result<String, Box<dyn Error>> {
    json_text_from_env_or_else(WORKSPACE_REFERENCE_JSON_ENV, evaluate_workspace_spec_fixture_with_nix)
}

/// Evaluate the web WorkspaceSpec canary to JSON from Nix or an env override.
fn evaluate_web_workspace_spec() -> Result<String, Box<dyn Error>> {
    json_text_from_env_or_else(WEB_WORKSPACE_JSON_ENV, evaluate_web_workspace_spec_with_nix)
}

/// Prove the web WorkspaceSpec matches its provider manifest and backend plans.
fn check_web_workspace_spec() -> Result<(), Box<dyn Error>> {
    let spec_json = evaluate_web_workspace_spec()?;
    let provider_manifest_json = web_workspace_provider_manifest_json()?;

    check_workspace_spec_against_provider_manifest(
        "web-workspace",
        &spec_json,
        &provider_manifest_json,
    )
}

/// Read the web WorkspaceSpec provider manifest from a store path or fixture.
fn web_workspace_provider_manifest_json() -> Result<String, Box<dyn Error>> {
    json_text_from_env_or_else(WEB_WORKSPACE_PROVIDER_MANIFEST_JSON_ENV, || {
        Ok(include_str!("../fixtures/web-workspace-provider-manifest.json").to_owned())
    })
}

/// Run strict decode, provider comparison, and backend-plan coverage for a spec.
fn check_workspace_spec_against_provider_manifest(
    workspace: &str,
    spec_json: &str,
    provider_manifest_json: &str,
) -> Result<(), Box<dyn Error>> {
    let value = serde_json::from_str(provider_manifest_json)?;
    let provider_manifest = BindingManifest::from_json_value(value)?;

    check_workspace_spec_against_manifest(workspace, spec_json, &provider_manifest)
}

/// Run strict decode, provider comparison, and backend-plan coverage for a spec.
fn check_workspace_spec_against_manifest(
    workspace: &str,
    spec_json: &str,
    provider_manifest: &BindingManifest,
) -> Result<(), Box<dyn Error>> {
    let spec_value: Value = serde_json::from_str(spec_json)?;
    let spec = decode_workspace_spec_text(spec_json)?;
    let spec_manifest = binding_manifest_for(&spec, provider_manifest.backend);

    compare_provider_manifests(workspace, &spec_manifest, provider_manifest)?;
    check_workspace_plan_coverage(workspace, &spec, provider_manifest, &spec_value)?;
    Ok(())
}

fn binding_manifest_for(spec: &WorkspaceSpec, backend: BackendKind) -> BindingManifest {
    BindingManifest {
        backend,
        panels: spec
            .panels
            .iter()
            .map(|panel| panel.provider_declaration())
            .collect(),
    }
}

/// Resolve the committed schema path used by `spec-parity check`.
fn committed_schema_path() -> PathBuf {
    env::var_os(SCHEMA_PATH_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("nix/schema/workspace-spec.schema.json"))
}

/// Read the fixed layout fixture from a store-path override when one is provided.
fn fixed_layout_json() -> Result<String, Box<dyn Error>> {
    json_text_from_env_or_else(FIXED_LAYOUT_JSON_ENV, || {
        Ok(include_str!("../fixtures/fixed-nine-layout.json").to_owned())
    })
}

/// Read JSON from an env override, otherwise use the repository-local fallback.
fn json_text_from_env_or_else(
    env_name: &str,
    fallback: impl FnOnce() -> Result<String, Box<dyn Error>>,
) -> Result<String, Box<dyn Error>> {
    match env::var_os(env_name) {
        Some(value) => json_text_from_override(env_name, &value),
        None => fallback(),
    }
}

/// Read a JSON override from a path, or accept inline JSON for cargo-only runs.
fn json_text_from_override(env_name: &str, value: &OsStr) -> Result<String, Box<dyn Error>> {
    let text = value
        .to_str()
        .ok_or_else(|| format!("{env_name} must be valid UTF-8"))?;
    let path = Path::new(text);

    if path.exists() {
        return Ok(fs::read_to_string(path)?);
    }

    if looks_like_inline_json(text) {
        return Ok(text.to_owned());
    }

    Err(format!("{env_name} points to missing JSON file: {text}").into())
}

/// Return whether an override value is inline JSON rather than a filesystem path.
fn looks_like_inline_json(text: &str) -> bool {
    matches!(text.trim_start().as_bytes().first(), Some(b'{' | b'['))
}

/// Evaluate the authoritative Nix WorkspaceSpec canary to JSON from the checkout.
fn evaluate_workspace_canary_spec_with_nix() -> Result<String, Box<dyn Error>> {
    evaluate_nix_json("nix/specs/workspace-canary.nix")
}

/// Evaluate the web WorkspaceSpec canary to JSON from the checkout.
fn evaluate_web_workspace_spec_with_nix() -> Result<String, Box<dyn Error>> {
    evaluate_nix_json("nix/specs/web-workspace.nix")
}

/// Evaluate the schema-driven `mkWorkspaceSpec` fixture from the checkout.
fn evaluate_workspace_spec_fixture_with_nix() -> Result<String, Box<dyn Error>> {
    evaluate_nix_json("nix/tests/workspace-spec.nix")
}

/// Evaluate one Nix file's `json` attribute with consistent diagnostics.
fn evaluate_nix_json(path: &str) -> Result<String, Box<dyn Error>> {
    let output = Command::new("nix")
        .args(["eval", "--raw", "--file", path, "json"])
        .output()?;

    if output.status.success() {
        return Ok(String::from_utf8(output.stdout)?);
    }

    Err(format!(
        "failed to evaluate {path}: {}",
        String::from_utf8_lossy(&output.stderr)
    )
    .into())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use panel_kit_core::spec::BackendKind;
    use serde_json::json;

    use super::{
        check_workspace_spec_against_provider_manifest, json_text_from_override,
        web_workspace_provider_manifest_json,
    };

    #[test]
    fn json_override_reads_store_path_text() {
        let path = std::env::temp_dir().join(format!(
            "spec-parity-override-{}-store-path.json",
            std::process::id()
        ));
        fs::write(&path, r#"{"source":"store-path"}"#).expect("test JSON can be written");

        let actual =
            json_text_from_override("SPEC_PARITY_WORKSPACE_REFERENCE_JSON", path.as_os_str())
                .expect("store-path override should be readable");

        fs::remove_file(path).expect("test JSON can be removed");
        assert_eq!(actual, r#"{"source":"store-path"}"#);
    }

    #[test]
    fn json_override_rejects_missing_path_like_value() {
        let path = std::env::temp_dir().join(format!(
            "spec-parity-missing-{}-store-path.json",
            std::process::id()
        ));

        let error =
            json_text_from_override("SPEC_PARITY_WORKSPACE_REFERENCE_JSON", path.as_os_str())
                .expect_err("missing path-like override should not be treated as JSON");

        let message = error.to_string();
        assert!(
            message.contains("SPEC_PARITY_WORKSPACE_REFERENCE_JSON points to missing JSON file"),
            "{message}"
        );
    }

    #[test]
    fn web_workspace_provider_manifest_fixture_decodes_as_web_manifest() {
        let manifest_json = web_workspace_provider_manifest_json()
            .expect("web workspace provider fixture can be read");
        let manifest_value =
            serde_json::from_str(&manifest_json).expect("web workspace provider JSON parses");
        let manifest = panel_kit_core::spec::BindingManifest::from_json_value(manifest_value)
            .expect("web workspace provider fixture decodes");
        assert_eq!(manifest.backend, BackendKind::Web);
        assert_eq!(manifest.panels.len(), 4);
        assert_eq!(manifest.panels[0].panel_id, "editor");
    }

    #[test]
    fn workspace_spec_check_rejects_provider_manifest_drift() {
        let spec_json = include_str!("../fixtures/workspace-spec-reference.json");
        let mut manifest: serde_json::Value = serde_json::from_str(include_str!(
            "../fixtures/workspace-spec-reference-provider-manifest.json"
        ))
        .expect("reference provider manifest is valid JSON");
        manifest["panels"][0]["content_kind"] = json!("spinner");
        let manifest_json =
            serde_json::to_string(&manifest).expect("mutated provider manifest serializes");

        let error = check_workspace_spec_against_provider_manifest(
            "workspace-spec-reference",
            spec_json,
            &manifest_json,
        )
        .expect_err("repository check must fail when provider content kind drifts");

        let message = error.to_string();
        assert!(message.contains("/panels/0/content_kind"), "{message}");
    }
}
