//! Host-contract test for the topos/grammar façade. With the real Nix-authored
//! JSON: every topos-workspace binding resolves under both regimes and both
//! grammars; the physics section is identical under both regimes (same engine,
//! different UI/UX) while their sheaf sections differ; the Assembly table lists
//! exactly the five stages chain/ring/sheet/tube/membrane in increasing `t`;
//! switching regimes transports the active subobject by `∃f` then `f*`; and a
//! malformed appended line keeps prior content while adding a diagnostic.

#[allow(dead_code)]
#[path = "../examples/support/topos_demo.rs"]
mod topos_demo;

use std::collections::BTreeSet;

use panel_kit_core::spec::WorkspaceSpec;
use panel_kit_grammar::{assembly, Section, Stage};
use topos_demo::{ToposDemo, GRAMMAR_JSON, GRAMMAR_LINES, TOPOS_AGENTIC, TOPOS_MEMBRANE};

/// Binding id and declared content kind of every sheaf-backed panel in a
/// topos's workspace.
fn panel_bindings(demo: &ToposDemo, topos: &str) -> Vec<(String, &'static str)> {
    let json = demo.workspace_json(topos).expect("topos has a workspace spec");
    let spec = WorkspaceSpec::from_json_str(json).expect("workspace spec parses");
    spec.panels
        .iter()
        .filter_map(|panel| {
            panel
                .content
                .binding_id()
                .map(|id| (id.to_owned(), panel.content.kind()))
        })
        .collect()
}

/// Every panel's resolved content, in panel order.
fn resolved_contents(demo: &ToposDemo, topos: &str) -> Vec<Section> {
    panel_bindings(demo, topos)
        .into_iter()
        .filter_map(|(id, _)| demo.resolve(&id).cloned())
        .collect()
}

/// A set of owned sort names from string literals.
fn sorts(items: impl IntoIterator<Item = &'static str>) -> BTreeSet<String> {
    items.into_iter().map(str::to_owned).collect()
}

#[test]
fn every_binding_resolves_under_both_topoi_and_grammars() {
    let mut demo = ToposDemo::new();
    for grammar in [GRAMMAR_LINES, GRAMMAR_JSON] {
        assert!(demo.set_grammar(grammar));
        for topos in [TOPOS_AGENTIC, TOPOS_MEMBRANE] {
            assert!(demo.set_topos(topos));
            let bindings = panel_bindings(&demo, topos);
            assert!(!bindings.is_empty(), "{topos} has no panel bindings");
            for (id, kind) in bindings {
                let bound = demo
                    .resolve(&id)
                    .unwrap_or_else(|| panic!("{id} unresolved under {topos}/{grammar}"));
                assert_eq!(bound.kind(), kind, "{id} kind mismatch under {topos}");
            }
        }
    }
}

#[test]
fn physics_section_is_equal_under_both_regimes_but_sheaves_differ() {
    let mut demo = ToposDemo::new();

    assert!(demo.set_topos(TOPOS_AGENTIC));
    let agentic_ids: Vec<String> = panel_bindings(&demo, TOPOS_AGENTIC)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let agentic_physics = demo.physics_section();
    let agentic = resolved_contents(&demo, TOPOS_AGENTIC);

    assert!(demo.set_topos(TOPOS_MEMBRANE));
    let membrane_physics = demo.physics_section();
    let membrane = resolved_contents(&demo, TOPOS_MEMBRANE);

    // Same underlying physics: one base topos, pulled back along each p_R.
    assert_eq!(
        agentic_physics, membrane_physics,
        "the physics section must be identical under both regimes"
    );
    assert!(!agentic_physics.objects.is_empty());
    assert!(!agentic_physics.morphisms.is_empty());

    // Different UI/UX: the agentic ids do not bind under the membrane regime, and
    // the two regimes paint different content from the same site.
    for id in &agentic_ids {
        assert!(
            demo.resolve(id).is_none(),
            "{id} must be unbound under the membrane topos"
        );
    }
    assert_ne!(agentic, membrane);
}

#[test]
fn assembly_lists_the_five_stages_in_increasing_t() {
    let demo = ToposDemo::new();
    assert_eq!(demo.topos_id(), TOPOS_AGENTIC);

    // The agentic assembly builds its 1-skeleton from continues + shares.
    let selected = sorts(["continues", "shares"]);
    let components = assembly(demo.site(), &demo.current_topos().morphism_sorting, &selected);

    let stages: Vec<Stage> = components.iter().map(|component| component.stage).collect();
    assert_eq!(
        stages,
        vec![
            Stage::Chain,
            Stage::Ring,
            Stage::Sheet,
            Stage::Tube,
            Stage::Membrane,
        ]
    );

    // Each component's representative appears in strictly increasing assembly time.
    let t_of = |id: &str| {
        demo.site()
            .objects
            .iter()
            .find(|object| object.id == id)
            .and_then(|object| object.number("t"))
            .unwrap_or_else(|| panic!("representative {id} carries a t field"))
    };
    let times: Vec<f64> = components
        .iter()
        .map(|component| t_of(&component.representative))
        .collect();
    assert!(
        times.windows(2).all(|window| window[0] < window[1]),
        "components must be listed in increasing t: {times:?}"
    );

    // The rendered Assembly table carries one row per stage.
    let Some(Section::Table(table)) = demo.resolve("agentic.assembly") else {
        panic!("agentic.assembly resolves to a table");
    };
    assert_eq!(table.rows.len(), 5, "one assembly row per component");
}

#[test]
fn switching_transports_the_active_subobject_by_image_then_pullback() {
    let mut demo = ToposDemo::new();
    assert_eq!(demo.topos_id(), TOPOS_AGENTIC);

    // Start from the single agentic role {planner}.
    demo.set_active(["planner".to_owned()]);
    assert_eq!(demo.active_set(), &sorts(["planner"]));

    // agentic → membrane transports by the image ∃f: planner ↦ head.
    assert!(demo.set_topos(TOPOS_MEMBRANE));
    assert_eq!(demo.active_set(), &sorts(["head"]));
    assert!(
        demo.status_line().contains("∃f agentic→membrane"),
        "status must name the morphism: {}",
        demo.status_line()
    );

    // membrane → agentic transports by the inverse image f*: head ↦ {planner, reviewer}.
    assert!(demo.set_topos(TOPOS_AGENTIC));
    assert_eq!(demo.active_set(), &sorts(["planner", "reviewer"]));
    assert!(
        demo.status_line().contains("f* membrane→agentic"),
        "status must name the morphism: {}",
        demo.status_line()
    );
}

#[test]
fn a_malformed_line_keeps_prior_content_and_adds_a_diagnostic() {
    for grammar in [GRAMMAR_LINES, GRAMMAR_JSON] {
        let mut demo = ToposDemo::new();
        assert!(demo.set_grammar(grammar));
        let topos = demo.topos_id();
        let before = resolved_contents(&demo, topos);
        let diagnostics = demo.diagnostic_count();
        let records = demo.record_count();

        demo.append_malformed_line();

        assert_eq!(
            demo.diagnostic_count(),
            diagnostics + 1,
            "{grammar}: a malformed line must add one diagnostic"
        );
        assert_eq!(
            demo.record_count(),
            records,
            "{grammar}: a malformed line must not change the site"
        );
        assert_eq!(
            resolved_contents(&demo, topos),
            before,
            "{grammar}: a malformed line must keep prior content"
        );
    }
}
