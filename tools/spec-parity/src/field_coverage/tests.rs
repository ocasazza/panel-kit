use std::collections::BTreeMap;

use panel_kit_core::spec::{BackendKind, WorkspaceSpec};
use serde_json::json;

use super::fixtures::{binding_manifest_for, reference_spec_value};
use super::{
    compare_required_schema_leaves, require_backend_field_dispositions, schema_leaf_pointers,
    tui_field_dispositions, web_field_dispositions, FieldDisposition,
};

#[test]
fn rust_only_field_is_rejected() {
    let complete = reference_spec_value();
    let mut nix_output = complete.clone();
    nix_output["chrome"]
        .as_object_mut()
        .expect("fixture chrome is an object")
        .remove("dock");
    let schema_leaf_pointers = schema_leaf_pointers(&complete);
    let pointer_refs = pointer_refs(&schema_leaf_pointers);

    let report = compare_required_schema_leaves(&nix_output, &pointer_refs)
        .expect_err("Nix output missing a Rust-required schema leaf must fail");

    assert!(
        report
            .to_string()
            .contains("/chrome/dock: required by Rust schema, missing from Nix output"),
        "{report}"
    );
}

#[test]
fn nix_only_field_is_rejected() {
    let mut nix_output = reference_spec_value();
    nix_output["chrome"]
        .as_object_mut()
        .expect("fixture chrome is an object")
        .insert("new_field".to_owned(), json!(true));

    let report = WorkspaceSpec::from_json_value(nix_output)
        .expect_err("extra Nix output fields must fail strict serde/schema validation");

    assert!(
        report
            .to_string()
            .contains("/chrome/new_field: unknown field"),
        "{report}"
    );
}

#[test]
fn backend_ignored_field_is_rejected() {
    let spec = WorkspaceSpec::from_json_value(reference_spec_value())
        .expect("reference spec should decode");
    let providers = binding_manifest_for(&spec, BackendKind::Web);
    let resolved = spec
        .resolve(&providers)
        .expect("providers match fixture spec");
    let web_plan = panel_kit::spec_plan::lower_spec(&resolved);
    let mut dispositions = web_field_dispositions(&web_plan);
    dispositions.remove("/chrome/dock");
    let schema_leaf_pointers = schema_leaf_pointers(&reference_spec_value());
    let pointer_refs = pointer_refs(&schema_leaf_pointers);
    let disposition_refs = pointer_refs_from_map(&dispositions);

    let report = require_backend_field_dispositions(
        "workspace-spec-reference",
        "web-wasm",
        &pointer_refs,
        &disposition_refs,
    )
    .expect_err("backend plans must account for every schema leaf they decode");

    assert!(
        report
            .to_string()
            .contains("workspace-spec-reference × web-wasm: backend did not lower /chrome/dock"),
        "{report}"
    );
}

/// Leaves the external-component contract adds to a workspace spec.
const EXPECTED_EXTERNAL_LEAVES: &[&str] = &[
    "/components/0/contract",
    "/components/0/events/0",
    "/components/0/fallback",
    "/components/0/id",
    "/components/0/keep_mounted",
    "/components/0/min_size/0",
    "/components/0/min_size/1",
    "/components/0/requires/0",
    "/panels/1/content/component",
    "/panels/1/content/layout/algorithm",
    "/panels/1/content/layout/group_by",
    "/panels/1/content/layout/params/rank_sep",
    "/panels/1/content/source/id",
];

/// Every leaf the external-component contract adds reaches both concrete
/// backend plans, and a plan that ignored the declaration is caught.
#[test]
fn external_component_leaves_are_lowered_by_both_backends() {
    let spec = WorkspaceSpec::from_json_value(reference_spec_value())
        .expect("reference spec with an external panel decodes");
    let providers = binding_manifest_for(&spec, BackendKind::Web);
    let resolved = spec
        .resolve(&providers)
        .expect("providers match fixture spec");

    let fixture_leaves = schema_leaf_pointers(&reference_spec_value());
    let leaf_refs = pointer_refs(&fixture_leaves);

    for (backend, plan) in [
        (
            "web-wasm",
            web_field_dispositions(&panel_kit::spec_plan::lower_spec(&resolved)),
        ),
        (
            "tui-native",
            tui_field_dispositions(&panel_kit_tui::spec_plan::lower_spec(&resolved)),
        ),
    ] {
        for leaf in EXPECTED_EXTERNAL_LEAVES {
            assert!(leaf_refs.contains(leaf), "{leaf} is not a fixture leaf");
            assert!(plan.contains_key(*leaf), "{backend} did not lower {leaf}");
        }

        let mut without_declarations = plan.clone();
        without_declarations.retain(|pointer, _| !pointer.starts_with("/components/"));
        let report = require_backend_field_dispositions(
            "workspace-spec-reference",
            backend,
            &leaf_refs,
            &pointer_refs_from_map(&without_declarations),
        )
        .expect_err("dropping the component declaration must fail coverage");

        assert!(
            report
                .to_string()
                .contains("backend did not lower /components/0/id"),
            "{backend}: {report}"
        );
    }
}

#[test]
fn tui_typography_and_density_are_explicit_approximations() {
    let spec = WorkspaceSpec::from_json_value(reference_spec_value())
        .expect("reference spec should decode");
    let providers = binding_manifest_for(&spec, BackendKind::Tui);
    let resolved = spec
        .resolve(&providers)
        .expect("providers match fixture spec");
    let tui_plan = panel_kit_tui::spec_plan::lower_spec(&resolved);
    let dispositions = tui_field_dispositions(&tui_plan);

    assert_explicit_approximation(&dispositions, "/theme/typography/family");
    assert_explicit_approximation(&dispositions, "/theme/typography/body_size");
    assert_explicit_approximation(&dispositions, "/theme/density/panel_radius");
    assert_explicit_approximation(&dispositions, "/theme/density/spacing_md");
}

fn assert_explicit_approximation(dispositions: &BTreeMap<String, FieldDisposition>, pointer: &str) {
    match dispositions.get(pointer) {
        Some(FieldDisposition::Approximated { reason, .. }) if !reason.trim().is_empty() => {}
        other => panic!("{pointer} should be an explicit approximation, got {other:?}"),
    }
}

fn pointer_refs(pointers: &[String]) -> Vec<&str> {
    pointers.iter().map(String::as_str).collect()
}

fn pointer_refs_from_map(dispositions: &BTreeMap<String, FieldDisposition>) -> Vec<&str> {
    dispositions.keys().map(String::as_str).collect()
}
