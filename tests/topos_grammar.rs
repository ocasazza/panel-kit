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
use panel_kit_core::widgets::table::TableCell;
use panel_kit_grammar::{assembly, CompiledGrammar, GrammarPackage, Limits, Section, Stage};
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

/// Row count of a binding that resolves to a table.
fn table_rows(demo: &ToposDemo, binding: &str) -> usize {
    match demo.resolve(binding) {
        Some(Section::Table(table)) => table.rows.len(),
        other => panic!("{binding} is not a table: {other:?}"),
    }
}

/// Text of a binding that resolves to a text section.
fn text_of(demo: &ToposDemo, binding: &str) -> String {
    match demo.resolve(binding) {
        Some(Section::Text(model)) => model.text.clone(),
        other => panic!("{binding} is not text: {other:?}"),
    }
}

#[test]
fn toggling_the_subobject_restricts_every_panel() {
    let mut demo = ToposDemo::new();
    let full_rows = table_rows(&demo, "agentic.status");
    let planner = demo.site().objects.iter().filter(|object| object.kind == "planner").count();
    assert!(planner > 0 && planner < full_rows, "{planner} planner of {full_rows} sessions");

    demo.set_active(["planner".to_owned()]);

    // The Sessions table is Γ over the {planner} subsite, not the whole site.
    assert_eq!(table_rows(&demo, "agentic.status"), planner);

    // The Assembly table lists exactly the subsite's connected components.
    let components = {
        let topos = demo.current_topos();
        let subsite = demo.site().restrict_objects(&topos.object_sorting, &sorts(["planner"]));
        assembly(&subsite, &topos.morphism_sorting, &sorts(["continues", "shares"])).len()
    };
    assert_eq!(table_rows(&demo, "agentic.assembly"), components);

    // Empty U renders an explicit empty state, never stale data.
    demo.set_active(std::iter::empty::<String>());
    assert_eq!(table_rows(&demo, "agentic.status"), 0);
}

#[test]
fn transport_moves_the_subobject_and_the_membrane_panels() {
    let mut demo = ToposDemo::new();
    demo.set_active(["planner".to_owned()]);

    // agentic → membrane transports {planner} by ∃f to {head}.
    assert!(demo.set_topos(TOPOS_MEMBRANE));
    assert_eq!(demo.active_set(), &sorts(["head"]));

    // The membrane Physics panel is Γ over the {head} subsite: the planner and
    // reviewer objects f merges into head.
    let head = demo
        .site()
        .objects
        .iter()
        .filter(|object| object.kind == "planner" || object.kind == "reviewer")
        .count();
    assert!(head > 0);
    assert_eq!(table_rows(&demo, "membrane.physics"), head);

    // membrane → agentic transports {head} by f* back to the whole fiber.
    assert!(demo.set_topos(TOPOS_AGENTIC));
    assert_eq!(demo.active_set(), &sorts(["planner", "reviewer"]));
}

#[test]
fn the_morphism_panel_shows_the_adjoint_triple() {
    let mut demo = ToposDemo::new();

    demo.set_active(["planner".to_owned()]);
    let text = text_of(&demo, "agentic.morphism");
    assert!(text.contains("∀f(U) = ∅"), "{text}");

    demo.set_active(["planner".to_owned(), "reviewer".to_owned()]);
    let text = text_of(&demo, "agentic.morphism");
    assert!(text.contains("∀f(U) = {head}"), "{text}");
}

#[test]
fn the_invariants_panel_holds_under_both_regimes() {
    let mut demo = ToposDemo::new();
    for topos in [TOPOS_AGENTIC, TOPOS_MEMBRANE] {
        assert!(demo.set_topos(topos));
        let binding = format!("{}.invariants", topos.trim_start_matches("topos-"));
        let Some(Section::Table(table)) = demo.resolve(&binding) else {
            panic!("{binding} resolves to a table");
        };
        assert_eq!(table.rows.len(), 3, "{binding} has one row per invariant");
        for row in &table.rows {
            assert!(
                matches!(&row.cells[1], TableCell::Status { label, .. } if label == "ok"),
                "{binding}: an invariant does not hold"
            );
        }
    }
}

/// A demo grammar compiled from the same Nix-authored package the host compiles.
fn compiled(grammar: &str) -> CompiledGrammar {
    let package = match grammar {
        GRAMMAR_LINES => include_str!(env!("PANEL_KIT_GRAMMAR_LINES")),
        GRAMMAR_JSON => include_str!(env!("PANEL_KIT_GRAMMAR_JSON")),
        other => panic!("no grammar {other}"),
    };
    GrammarPackage::from_json_str(package).unwrap().compile().unwrap()
}

#[test]
fn the_site_stays_the_parse_of_the_source_through_appends_rejections_and_resets() {
    for grammar in [GRAMMAR_LINES, GRAMMAR_JSON] {
        let compiled = compiled(grammar);
        let mut demo = ToposDemo::new();
        assert!(demo.set_grammar(grammar));
        let records = demo.record_count();
        let check = |demo: &ToposDemo, step: &str| {
            let parsed = compiled.parse(demo.source()).unwrap_or_else(|error| panic!("{grammar} {step}: {error}"));
            assert_eq!(&parsed, demo.site(), "{grammar} {step}");
        };
        check(&demo, "fresh");
        for step in ["valid", "valid", "malformed", "valid", "malformed", "malformed", "valid"] {
            match step {
                "valid" => demo.append_valid_line(),
                _ => demo.append_malformed_line(),
            }
            check(&demo, step);
        }
        // The JSON document takes no appended line, so its valid append adds nothing.
        let grown = if grammar == GRAMMAR_LINES { 4 } else { 0 };
        assert_eq!(demo.record_count(), records + grown, "{grammar}");
        assert_eq!(demo.diagnostic_count(), 3, "{grammar}");
        demo.reset_source();
        check(&demo, "reset");
        assert_eq!(demo.record_count(), records, "{grammar}");
    }
}

#[test]
fn appended_lines_count_against_the_limits_of_the_whole_site() {
    let limits = Limits { grammar_bytes: 8192, input_bytes: 65536, nodes: 50, edges: 5000 };
    let mut demo = ToposDemo::with_limits(limits);
    let room = 50 - demo.record_count();
    for _ in 0..room + 5 {
        demo.append_valid_line();
    }
    assert_eq!(demo.record_count(), 50, "the site stops at the node limit");
    assert_eq!(demo.diagnostic_count(), 5, "each line over the limit is refused");
    assert_eq!(compiled(GRAMMAR_LINES).parse(demo.source()).map(|site| site.objects.len()), Ok(50));
}

#[test]
fn the_line_grammar_declares_line_records_and_the_json_document_does_not() {
    assert!(compiled(GRAMMAR_LINES).appends_incrementally(), "appended lines are parsed apart");
    assert!(!compiled(GRAMMAR_JSON).appends_incrementally(), "a JSON document is parsed whole");
}

/// Timing probe, not an assertion: the host's cost of each append as the input
/// grows. `cargo test --release --test topos_grammar -- --ignored --nocapture
/// append_cost`; `APPEND_LINES` sets the line count (default 10 000).
#[test]
#[ignore = "timing probe"]
fn append_cost_by_input_size() {
    let lines: usize = std::env::var("APPEND_LINES").ok().and_then(|text| text.parse().ok()).unwrap_or(10_000);
    let limits = Limits { grammar_bytes: 8192, input_bytes: 64 << 20, nodes: 100_000, edges: 100_000 };
    let mut demo = ToposDemo::with_limits(limits);
    let step = (lines / 10).max(1);
    let started = std::time::Instant::now();
    let mut lap = started;
    for line in 1..=lines {
        demo.append_valid_line();
        if line % step == 0 {
            println!(
                "{line:>6} lines: {:>9.3?} elapsed, {:>9.3?} per append over the last {step}",
                started.elapsed(),
                lap.elapsed() / step as u32
            );
            lap = std::time::Instant::now();
        }
    }
    assert_eq!(demo.record_count(), 43 + lines);
}
