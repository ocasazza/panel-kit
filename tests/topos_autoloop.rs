//! Host-contract test for the omp auto-loop regimes, with the Nix-authored
//! specs and fixtures the loop's own producers wrote (nix/data/autoloop-*):
//! the trace and control sites glue along `repo`; every binding resolves under
//! both regimes over the glued site; each control table offers its edits on
//! exactly its sort's rows; an edit's write intent is the record the loop's
//! `POST /api/site` reads, which re-parses to the revised object; and the web
//! host's façade offers edits only under the regime that declares them.
//!
//! Paths come from the environment at run time (the `topos-autoloop-test`
//! check sets them), so this target compiles under every check.

#[allow(dead_code)]
#[path = "../examples/support/autoloop_host.rs"]
mod autoloop_host;

use std::collections::BTreeMap;

use panel_kit_grammar::{
    BaseTopos, CompiledGrammar, EditError, EditInput, GeometricMorphism, GrammarPackage, Physics, Section, Site,
    Topos,
};

fn read(var: &str) -> String {
    let path = std::env::var(var).unwrap_or_else(|_| panic!("{var} is set by the topos-autoloop-test check"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{var}={path}: {error}"))
}

fn grammar(var: &str) -> CompiledGrammar {
    GrammarPackage::from_json_str(&read(var)).expect("grammar decodes").compile().expect("grammar compiles")
}

fn topos(var: &str) -> Topos {
    let topos = Topos::from_json_str(&read(var)).expect("topos decodes");
    topos.validate().expect("topos validates");
    topos
}

fn physics(regime: &Topos, morphism: &str) -> Physics {
    let base = BaseTopos::from_json_str(&read("PANEL_KIT_TOPOS_PHYSICS")).expect("base decodes");
    let p = GeometricMorphism::from_json_str(&read(morphism)).expect("morphism decodes");
    Physics::new(&base, &p, regime).expect("physics is total over the regime")
}

struct Fixture {
    control_grammar: CompiledGrammar,
    trace: Site,
    control: Site,
    glued: Site,
}

fn fixture() -> Fixture {
    let control_grammar = grammar("PANEL_KIT_AUTOLOOP_GRAMMAR_CONTROL");
    let trace = grammar("PANEL_KIT_AUTOLOOP_GRAMMAR_TRACE")
        .parse(&read("PANEL_KIT_AUTOLOOP_DATA_TRACE"))
        .expect("the loop's projection parses");
    let control = control_grammar.parse(&read("PANEL_KIT_AUTOLOOP_DATA_CONTROL")).expect("the loop's control site parses");
    let glued = trace.glue(&control, "repo").expect("the sites glue along repo");
    Fixture { control_grammar, trace, control, glued }
}

fn table<'a>(sections: &'a panel_kit_grammar::GlobalSections, id: &str) -> &'a panel_kit_core::widgets::table::TableModel {
    match sections.get(id) {
        Some(Section::Table(model)) => model,
        other => panic!("{id} is not a table: {other:?}"),
    }
}

fn host() -> autoloop_host::AutoloopHost {
    let specs = [
        "PANEL_KIT_TOPOS_PHYSICS",
        "PANEL_KIT_AUTOLOOP_TOPOS_TRACE",
        "PANEL_KIT_AUTOLOOP_TOPOS_CONTROL",
        "PANEL_KIT_AUTOLOOP_MORPHISM_TRACE_PHYSICS",
        "PANEL_KIT_AUTOLOOP_MORPHISM_CONTROL_PHYSICS",
        "PANEL_KIT_AUTOLOOP_GRAMMAR_TRACE",
        "PANEL_KIT_AUTOLOOP_GRAMMAR_CONTROL",
    ]
    .map(read);
    let mut host = autoloop_host::AutoloopHost::new(&autoloop_host::Specs {
        base: &specs[0],
        trace_topos: &specs[1],
        control_topos: &specs[2],
        trace_physics: &specs[3],
        control_physics: &specs[4],
        trace_grammar: &specs[5],
        control_grammar: &specs[6],
    })
    .expect("host builds from the specs");
    host.load(&read("PANEL_KIT_AUTOLOOP_DATA_TRACE"), &read("PANEL_KIT_AUTOLOOP_DATA_CONTROL"))
        .expect("host loads the loop's inputs");
    host
}

#[test]
fn trace_and_control_glue_along_repo_without_duplicating_a_repo() {
    let f = fixture();
    let repos = |site: &Site| site.objects.iter().filter(|o| o.kind == "repo").count();
    assert_eq!(repos(&f.trace), 2);
    assert_eq!(repos(&f.control), 2);
    assert_eq!(repos(&f.glued), 2, "identified repos appear once");
    assert_eq!(f.glued.objects.len(), f.trace.objects.len() + f.control.objects.len() - 2);
    assert_eq!(f.glued.morphisms.len(), f.trace.morphisms.len() + f.control.morphisms.len());
}

#[test]
fn both_regimes_resolve_every_binding_over_the_glued_site() {
    let f = fixture();
    for (var, morphism) in [
        ("PANEL_KIT_AUTOLOOP_TOPOS_TRACE", "PANEL_KIT_AUTOLOOP_MORPHISM_TRACE_PHYSICS"),
        ("PANEL_KIT_AUTOLOOP_TOPOS_CONTROL", "PANEL_KIT_AUTOLOOP_MORPHISM_CONTROL_PHYSICS"),
    ] {
        let regime = topos(var);
        let sections = regime.global_sections(&f.glued, &physics(&regime, morphism)).expect("every sheaf resolves");
        assert_eq!(sections.iter().count(), regime.sheaves.len(), "{var}");
    }
}

#[test]
fn sessions_assemble_under_the_trace_regime_and_events_are_solvent() {
    let f = fixture();
    let regime = topos("PANEL_KIT_AUTOLOOP_TOPOS_TRACE");
    let section = regime.physics_section(&f.glued, &physics(&regime, "PANEL_KIT_AUTOLOOP_MORPHISM_TRACE_PHYSICS"));
    let engine: BTreeMap<&str, &str> = section.objects.iter().map(|(id, engine, _, _)| (id.as_str(), engine.as_str())).collect();
    assert_eq!(engine["sa1"], "hydrophobic");
    assert_eq!(engine["rac6ec850"], "hydrophilic");
    assert_eq!(engine["ea1_1"], "solvent");

    let sessions = table(&regime.global_sections(&f.glued, &physics(&regime, "PANEL_KIT_AUTOLOOP_MORPHISM_TRACE_PHYSICS")).unwrap(), "autoloop.sessions").rows.len();
    assert_eq!(sessions, 3);
}

#[test]
fn each_control_table_offers_its_edits_on_exactly_its_sorts_rows() {
    let f = fixture();
    let regime = topos("PANEL_KIT_AUTOLOOP_TOPOS_CONTROL");
    let sections = regime
        .global_sections(&f.glued, &physics(&regime, "PANEL_KIT_AUTOLOOP_MORPHISM_CONTROL_PHYSICS"))
        .unwrap();
    for (binding, prefix, edits) in [
        ("autoloop.claims", "claim:", vec!["ratify"]),
        ("autoloop.repos", "repo_policy:", vec!["set_gates", "set_criteria"]),
        ("autoloop.queue", "queued_goal:", vec!["prioritize", "retire"]),
        ("autoloop.limits", "param:", vec!["set_value"]),
    ] {
        let rows = sections.row_edits(binding).unwrap_or_else(|| panic!("{binding} offers edits"));
        assert!(!rows.is_empty(), "{binding}");
        assert_eq!(rows.len(), table(&sections, binding).rows.len(), "{binding}: one entry per row");
        for row in rows {
            assert!(row.object.starts_with(prefix), "{binding}: {}", row.object);
            assert_eq!(row.edits, edits, "{binding}: {}", row.object);
        }
    }
}

#[test]
fn a_ratify_writes_the_record_the_loop_reads_and_it_reparses_to_the_revision() {
    let f = fixture();
    let regime = topos("PANEL_KIT_AUTOLOOP_TOPOS_CONTROL");
    let claim = "claim:reviewer#b1@2026-10-04T01:04:00.000Z";
    let input = EditInput {
        object: claim.to_owned(),
        values: BTreeMap::from([("decision".to_owned(), "overturned".to_owned()), ("note".to_owned(), "no evidence".to_owned())]),
    };
    let (revised, revision) = regime.revise(&f.glued, "ratify", &input).expect("ratify applies to a claim");
    let intent = regime.write_intent(&f.control_grammar, &revised, &revision).expect("the control grammar emits");
    assert_eq!(intent.grammar, "autoloop-control");

    // Re-parsing through the control grammar reads `/objects` with `sort` as
    // kind: the shape the loop's write route takes.
    let reparsed = f.control_grammar.parse(std::str::from_utf8(&intent.bytes).unwrap()).expect("emitted bytes parse");
    let after = revised.objects.iter().find(|o| o.id == claim).unwrap();
    assert_eq!(reparsed.objects, vec![after.clone()]);
    assert_eq!(after.kind, "claim");
    assert_eq!(after.fields["decision"], "overturned");
    assert_eq!(after.fields["note"], "no evidence");
    assert_eq!(after.fields["session"], "reviewer#b1", "unchanged fields travel so the loop sees no read-only change");
}

#[test]
fn a_control_edit_is_refused_on_a_trace_object_and_out_of_range_values() {
    let f = fixture();
    let regime = topos("PANEL_KIT_AUTOLOOP_TOPOS_CONTROL");
    let set = |object: &str, field: &str, value: &str| EditInput {
        object: object.to_owned(),
        values: BTreeMap::from([(field.to_owned(), value.to_owned())]),
    };
    assert!(matches!(
        regime.revise(&f.glued, "ratify", &set("sa1", "decision", "accepted")),
        Err(EditError::NotApplicable { .. })
    ));
    assert!(matches!(
        regime.revise(&f.glued, "ratify", &set("claim:reviewer#b1@2026-10-04T01:04:00.000Z", "decision", "maybe")),
        Err(EditError::InvalidValue { .. })
    ));
}

#[test]
fn the_host_offers_edits_only_under_the_regime_that_declares_them() {
    let mut host = host();
    let claim = "claim:reviewer#b1@2026-10-04T01:04:00.000Z";
    let accept = || BTreeMap::from([("decision".to_owned(), "accepted".to_owned())]);
    assert_eq!(host.active_id(), autoloop_host::TRACE);
    assert!(host.row_edits("autoloop.claims").is_none(), "the trace regime has no claims table");
    assert_eq!(host.write("ratify", claim, accept()), Err("no edit ratify".to_owned()));

    assert!(host.set_active(autoloop_host::CONTROL));
    let rows = host.row_edits("autoloop.claims").expect("claims rows offer edits");
    assert!(rows.iter().any(|row| row.object == claim && row.edits == ["ratify"]));
    assert!(host.edit("ratify").expect("declared").set.contains_key("decision"));

    let bytes = host.write("ratify", claim, accept()).expect("the host emits the revised claim");
    let written = grammar("PANEL_KIT_AUTOLOOP_GRAMMAR_CONTROL").parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
    assert_eq!(written.objects.len(), 1);
    assert_eq!(written.objects[0].id, claim);
    assert_eq!(written.objects[0].fields["decision"], "accepted");
}

#[test]
fn a_host_edit_that_changes_nothing_writes_nothing() {
    let mut host = host();
    host.set_active(autoloop_host::CONTROL);
    let param = host.object("param:maxContinuations").expect("the limits are loaded");
    let unchanged = BTreeMap::from([("value".to_owned(), param.fields["value"].clone())]);
    assert_eq!(host.write("set_value", "param:maxContinuations", unchanged), Err("nothing changed".to_owned()));
}
