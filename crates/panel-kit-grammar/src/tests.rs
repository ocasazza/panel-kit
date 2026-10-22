//! Behavior-level tests: both engines produce the same site; limits and
//! validation reject bad input; a records-only JSON grammar yields no
//! morphisms; the geometric morphism satisfies the triangle identities and
//! both Galois connections; validation rejects non-total/outside/non-commuting
//! sort maps; the physics section is regime-invariant; and assembly classifies
//! each topological stage.

use std::collections::{BTreeMap, BTreeSet};

use panel_kit_core::widgets::data_table::{
    natural_cmp, BuildOptions, ColumnKind, SortDir, SortSpec, TableDefaults, TableState, View,
};
use panel_kit_core::widgets::table::{TableCell, TableModel, TextAlign};
use proptest::prelude::{prop, prop_assert, prop_assert_eq, proptest, Strategy};

use crate::error::SheafError;
use crate::package::CompiledGrammar;
use crate::{
    assembly, status_table, BaseTopos, GeometricMorphism, GrammarError, GrammarPackage, Morphism,
    MorphismError, MorphismSide, Object, ParserSpec, Physics, Section, Site, Stage, Topos, ToposError,
};

/// Pest grammar package, byte-for-byte the Nix-produced `grammar-trace-lines`.
const GRAMMAR_LINES: &str = include_str!("../testdata/trace-lines.grammar.json");
/// JSON grammar package, byte-for-byte the Nix-produced `grammar-trace-json`.
const GRAMMAR_JSON: &str = include_str!("../testdata/trace-json.grammar.json");
/// JSON grammar package without an `edges` pointer (records-only).
const GRAMMAR_RECORDS: &str = include_str!("../testdata/grammar-records.json");
/// Minimal two-panel workspace (text `legend`, gauges `load`) for inline topoi.
const WS_MIN: &str = include_str!("../testdata/inline-workspace.json");

/// The same site authored two ways, parsed by the two engines in tests.
const DATA_LINES: &str = include_str!("../testdata/trace.lines");
const DATA_JSON: &str = include_str!("../testdata/trace.json");
/// Records-only JSON input (no edges).
const DATA_RECORDS: &str = include_str!("../testdata/records.json");

// --- Regime / base / morphism fixtures -------------------------------------

const AGENTIC_OSORT: &str =
    r##"{"planner":"planner","coder":"coder","reviewer":"reviewer","memory":"memory"}"##;
const AGENTIC_MSORT: &str =
    r##"{"continues":"continues","shares":"shares","consolidates":"consolidates"}"##;
const AGENTIC_STALKS: &str = r##"{"objects":{"planner":{"label":"Planner","badge":"status"},"coder":{"label":"Coder","badge":"entity"},"reviewer":{"label":"Reviewer","badge":"status"},"memory":{"label":"Memory","badge":"folder"}},"morphisms":{"continues":{"label":"continues"},"shares":{"label":"shares"},"consolidates":{"label":"consolidates"}},"stages":{"chain":"handoff chain","branched":"branched handoff","ring":"loop","sheet":"team sheet","tube":"pipeline tube","membrane":"closed org"}}"##;

const MEMBRANE_OSORT: &str =
    r##"{"planner":"head","coder":"tail","reviewer":"head","memory":"solvent"}"##;
const MEMBRANE_MSORT: &str =
    r##"{"continues":"tail_bond","shares":"lateral","consolidates":"solvation"}"##;
const MEMBRANE_STALKS: &str = r##"{"objects":{"head":{"label":"Head","badge":"status"},"tail":{"label":"Tail","badge":"entity"},"solvent":{"label":"Solvent","badge":"folder"}},"morphisms":{"tail_bond":{"label":"tail bond"},"lateral":{"label":"lateral"},"solvation":{"label":"solvation"}},"stages":{"chain":"chain","branched":"branched","ring":"ring","sheet":"bilayer sheet","tube":"flat tube","membrane":"membrane"}}"##;

const BASE_JSON: &str = r##"{"spec_version":1,"id":"topos-physics","regime":"Physics engine","stalks":{"objects":{"hydrophilic":{"label":"Hydrophilic","repulsion":9.0,"mass":1.0},"hydrophobic":{"label":"Hydrophobic","repulsion":3.0,"mass":2.0},"solvent":{"label":"Solvent","repulsion":1.0,"mass":1.0}},"morphisms":{"stiff":{"label":"Stiff","weight":5.0,"rest_length":1.0},"lateral":{"label":"Lateral","weight":2.0,"rest_length":1.5},"weak":{"label":"Weak","weight":1.0,"rest_length":2.0}}}}"##;

const F_JSON: &str = r##"{"id":"agentic-membrane","domain":"topos-agentic","codomain":"topos-membrane","object_sorts":{"planner":"head","coder":"tail","reviewer":"head","memory":"solvent"},"morphism_sorts":{"continues":"tail_bond","shares":"lateral","consolidates":"solvation"}}"##;
const PA_JSON: &str = r##"{"id":"agentic-physics","domain":"topos-agentic","codomain":"topos-physics","object_sorts":{"planner":"hydrophilic","coder":"hydrophobic","reviewer":"hydrophilic","memory":"solvent"},"morphism_sorts":{"continues":"stiff","shares":"lateral","consolidates":"weak"}}"##;
const PM_JSON: &str = r##"{"id":"membrane-physics","domain":"topos-membrane","codomain":"topos-physics","object_sorts":{"head":"hydrophilic","tail":"hydrophobic","solvent":"solvent"},"morphism_sorts":{"tail_bond":"stiff","lateral":"lateral","solvation":"weak"}}"##;
const PM_ALT_JSON: &str = r##"{"id":"membrane-physics","domain":"topos-membrane","codomain":"topos-physics","object_sorts":{"head":"hydrophobic","tail":"hydrophobic","solvent":"solvent"},"morphism_sorts":{"tail_bond":"stiff","lateral":"lateral","solvation":"weak"}}"##;

const E_OBJ: [&str; 4] = ["planner", "coder", "reviewer", "memory"];
const F_OBJ: [&str; 3] = ["head", "tail", "solvent"];

// --- Builders --------------------------------------------------------------

fn lines_grammar() -> CompiledGrammar {
    GrammarPackage::from_json_str(GRAMMAR_LINES).unwrap().compile().unwrap()
}

fn json_grammar() -> CompiledGrammar {
    GrammarPackage::from_json_str(GRAMMAR_JSON).unwrap().compile().unwrap()
}

fn site_from_lines() -> Site {
    lines_grammar().parse(DATA_LINES).unwrap()
}

fn topos_json(id: &str, regime: &str, osort: &str, msort: &str, stalks: &str, sheaves: &str) -> String {
    format!(
        r##"{{"spec_version":1,"id":"{id}","regime":"{regime}","object_sorting":{osort},"morphism_sorting":{msort},"stalks":{stalks},"sheaves":{sheaves},"workspace":{ws}}}"##,
        id = id,
        regime = regime,
        osort = osort,
        msort = msort,
        stalks = stalks,
        sheaves = sheaves,
        ws = WS_MIN,
    )
}

fn agentic_topos(sheaves: &str) -> Topos {
    Topos::from_json_str(&topos_json(
        "topos-agentic",
        "Agentic traces",
        AGENTIC_OSORT,
        AGENTIC_MSORT,
        AGENTIC_STALKS,
        sheaves,
    ))
    .unwrap()
}

fn membrane_topos(sheaves: &str) -> Topos {
    Topos::from_json_str(&topos_json(
        "topos-membrane",
        "Lipid self-assembly",
        MEMBRANE_OSORT,
        MEMBRANE_MSORT,
        MEMBRANE_STALKS,
        sheaves,
    ))
    .unwrap()
}

fn base() -> BaseTopos {
    BaseTopos::from_json_str(BASE_JSON).unwrap()
}

fn morphism(json: &str) -> GeometricMorphism {
    GeometricMorphism::from_json_str(json).unwrap()
}

fn physics_a() -> Physics {
    Physics::new(&base(), &morphism(PA_JSON), &agentic_topos("{}")).unwrap()
}

fn physics_b() -> Physics {
    Physics::new(&base(), &morphism(PM_JSON), &membrane_topos("{}")).unwrap()
}

fn object(id: &str, kind: &str, energy: &str) -> Object {
    Object {
        id: id.to_owned(),
        title: id.to_uppercase(),
        kind: kind.to_owned(),
        tags: Vec::new(),
        fields: BTreeMap::from([("energy".to_owned(), energy.to_owned())]),
    }
}

fn typed(domain: &str, codomain: &str, kind: &str) -> Morphism {
    Morphism { domain: domain.to_owned(), codomain: codomain.to_owned(), kind: Some(kind.to_owned()) }
}

/// Four objects (one per site kind) in a continues/shares/consolidates chain.
fn demo_site() -> Site {
    Site {
        objects: vec![
            object("p1", "planner", "30"),
            object("c1", "coder", "90"),
            object("r1", "reviewer", "40"),
            object("m1", "memory", "5"),
        ],
        morphisms: vec![
            typed("p1", "c1", "continues"),
            typed("c1", "r1", "shares"),
            typed("r1", "m1", "consolidates"),
        ],
    }
}

fn subsets(universe: &[&str]) -> Vec<BTreeSet<String>> {
    let n = universe.len();
    (0..(1u32 << n))
        .map(|mask| {
            universe
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, sort)| (*sort).to_owned())
                .collect()
        })
        .collect()
}

// --- Grammar + engine tests ------------------------------------------------

#[test]
fn both_engines_produce_equal_graphs() {
    let from_lines = site_from_lines();
    let from_json = json_grammar().parse(DATA_JSON).unwrap();

    assert_eq!(from_lines.objects.len(), 6);
    assert_eq!(from_lines.morphisms.len(), 5);
    assert_eq!(from_lines, from_json);

    // A spot check that discriminates a float-formatting mismatch between the
    // engines: a whole-valued latency must read `1840.0`, not `1840`.
    let code = from_lines.objects.iter().find(|object| object.id == "c1").unwrap();
    assert_eq!(code.fields.get("latency_ms").map(String::as_str), Some("1840.0"));
    assert_eq!(code.fields.get("status").map(String::as_str), Some("error"));
    assert_eq!(code.kind, "coder");
}

#[test]
fn records_only_json_parses_without_morphisms() {
    let grammar = GrammarPackage::from_json_str(GRAMMAR_RECORDS).unwrap().compile().unwrap();
    let site = grammar.parse(DATA_RECORDS).unwrap();
    assert_eq!(site.objects.len(), 1);
    assert!(site.morphisms.is_empty());
    assert_eq!(site.objects[0].id, "x1");
    assert_eq!(site.objects[0].kind, "planner");
}

#[test]
fn input_over_input_bytes_fails() {
    let mut package = GrammarPackage::from_json_str(GRAMMAR_LINES).unwrap();
    package.limits.input_bytes = 16;
    let grammar = package.compile().unwrap();
    assert!(matches!(grammar.parse(DATA_LINES), Err(GrammarError::InputTooLarge { .. })));
}

#[test]
fn node_limit_exact_passes_and_over_fails() {
    let mut package = GrammarPackage::from_json_str(GRAMMAR_LINES).unwrap();
    package.limits.nodes = 6;
    assert!(package.compile().unwrap().parse(DATA_LINES).is_ok());

    package.limits.nodes = 5;
    assert!(matches!(
        package.compile().unwrap().parse(DATA_LINES),
        Err(GrammarError::RecordLimit { kind: "nodes", actual: 6, max: 5 })
    ));
}

#[test]
fn edge_limit_exact_passes_and_over_fails() {
    let mut package = GrammarPackage::from_json_str(GRAMMAR_JSON).unwrap();
    package.limits.edges = 5;
    assert!(package.compile().unwrap().parse(DATA_JSON).is_ok());

    package.limits.edges = 4;
    assert!(matches!(
        package.compile().unwrap().parse(DATA_JSON),
        Err(GrammarError::RecordLimit { kind: "edges", actual: 5, max: 4 })
    ));
}

#[test]
fn dangling_edge_fails_in_both_engines() {
    let lines = lines_grammar();
    assert!(matches!(
        lines.parse("N|a|A|planner||x=1\nE|a|ghost|continues\n"),
        Err(GrammarError::DanglingEdge { missing: "target", .. })
    ));

    let json = json_grammar();
    let input = r#"{"nodes":[{"id":"a","title":"A","kind":"planner","tags":[]}],"edges":[{"source":"a","target":"ghost","kind":"continues"}]}"#;
    assert!(matches!(
        json.parse(input),
        Err(GrammarError::DanglingEdge { missing: "target", .. })
    ));
}

#[test]
fn unknown_capture_rule_fails_to_compile() {
    let mut package = GrammarPackage::from_json_str(GRAMMAR_LINES).unwrap();
    if let ParserSpec::Pest(spec) = &mut package.parser {
        spec.captures.node = "does_not_exist".to_owned();
    }
    assert!(matches!(package.compile(), Err(GrammarError::MissingRule { role: "node", .. })));
}

#[test]
fn invalid_grammar_fails_to_compile() {
    let mut package = GrammarPackage::from_json_str(GRAMMAR_LINES).unwrap();
    if let ParserSpec::Pest(spec) = &mut package.parser {
        spec.grammar = "this is not a valid pest grammar @@@".to_owned();
    }
    assert!(matches!(package.compile(), Err(GrammarError::InvalidGrammar(_))));
}

#[test]
fn unknown_json_keys_fail_to_decode() {
    let error = GrammarPackage::from_json_str(r#"{"format_version":1,"bogus":1}"#).unwrap_err();
    assert!(matches!(error, GrammarError::Json(_)));
    assert!(error.to_string().contains("unknown field"), "got: {error}");
}

#[test]
fn unsupported_version_fails_to_compile() {
    let mut package = GrammarPackage::from_json_str(GRAMMAR_LINES).unwrap();
    package.format_version = 2;
    assert!(matches!(
        package.compile(),
        Err(GrammarError::UnsupportedVersion { found: 2, supported: 1 })
    ));
}

// --- Topos validation ------------------------------------------------------

#[test]
fn two_topoi_differ_for_the_same_binding_id() {
    let sheaves = r##"{"legend":{"kind":"text","heading":"Legend","include_stalks":true},"load":{"kind":"gauges","scope":"kinds","value":"weighted:energy","scale":10000.0}}"##;
    let agentic = agentic_topos(sheaves);
    let membrane = membrane_topos(sheaves);
    agentic.validate().unwrap();
    membrane.validate().unwrap();

    let site = demo_site();
    let a = agentic.global_sections(&site, &physics_a()).unwrap();
    let b = membrane.global_sections(&site, &physics_b()).unwrap();

    // Same binding id, different content: the regime labels differ.
    let legend_a = a.get("legend").unwrap();
    let legend_b = b.get("legend").unwrap();
    assert_ne!(legend_a, legend_b);
    match (legend_a, legend_b) {
        (Section::Text(ta), Section::Text(tb)) => {
            assert!(ta.text.contains("Planner"), "A: {}", ta.text);
            assert!(tb.text.contains("Head"), "B: {}", tb.text);
        }
        _ => panic!("legend is not text"),
    }

    // Gauges group by regime sort: agentic has 4 groups, membrane merges
    // planner + reviewer into head for 3.
    let load_a = a.get("load").unwrap();
    let load_b = b.get("load").unwrap();
    assert_ne!(load_a, load_b);
    match (load_a, load_b) {
        (Section::Gauges(ga), Section::Gauges(gb)) => {
            assert_eq!(ga.len(), 4);
            assert_eq!(gb.len(), 3);
        }
        _ => panic!("load is not gauges"),
    }
}

#[test]
fn validate_rejects_missing_sheaf() {
    let sheaves = r##"{"legend":{"kind":"text","heading":"Legend"}}"##;
    let topos = agentic_topos(sheaves);
    assert!(matches!(
        topos.validate(),
        Err(ToposError::MissingSheaf { binding }) if binding == "load"
    ));
}

#[test]
fn validate_rejects_unreferenced_sheaf() {
    let sheaves = r##"{"legend":{"kind":"text","heading":"Legend"},"load":{"kind":"gauges","scope":"kinds","value":"field:energy","scale":1000.0},"extra":{"kind":"text","heading":"Extra"}}"##;
    let topos = agentic_topos(sheaves);
    assert!(matches!(
        topos.validate(),
        Err(ToposError::UnreferencedSheaf { id }) if id == "extra"
    ));
}

#[test]
fn validate_rejects_kind_mismatch() {
    let sheaves = r##"{"legend":{"kind":"badges","group":"kinds"},"load":{"kind":"gauges","scope":"kinds","value":"field:energy","scale":1000.0}}"##;
    let topos = agentic_topos(sheaves);
    assert!(matches!(
        topos.validate(),
        Err(ToposError::KindMismatch { binding, content_kind, sheaf_kind: "badges" })
            if binding == "legend" && content_kind == "text"
    ));
}

#[test]
fn global_sections_rejects_unsorted_kind() {
    let agentic = agentic_topos("{}");
    let site = Site {
        objects: vec![object("x", "ghost", "1")],
        morphisms: Vec::new(),
    };
    assert!(matches!(
        agentic.check_site(&site),
        Err(SheafError::UnsortedObjectKind(kind)) if kind == "ghost"
    ));
    assert!(matches!(
        agentic.global_sections(&site, &physics_a()),
        Err(SheafError::UnsortedObjectKind(_))
    ));
}

// --- Geometric morphism laws ----------------------------------------------

#[test]
fn adjunction_triangle_identities_hold_over_all_subsets() {
    let phi = morphism(F_JSON);

    // Triangle 1: ε_{f*G} ∘ f*(η_G) = id, over the indicator of each F-subset.
    for subset in subsets(&F_OBJ) {
        let g: BTreeMap<String, bool> =
            F_OBJ.iter().map(|t| ((*t).to_owned(), subset.contains(*t))).collect();
        let f_g = phi.inverse_image(&g);
        let round = phi.counit(&phi.inverse_image(&phi.unit(&g)));
        assert_eq!(round, f_g, "triangle 1 failed for G={subset:?}");
    }

    // Triangle 2: f_*(ε_H) ∘ η_{f_*H} = id, over the indicator of each E-subset.
    for subset in subsets(&E_OBJ) {
        let h: BTreeMap<String, bool> =
            E_OBJ.iter().map(|s| ((*s).to_owned(), subset.contains(*s))).collect();
        let f_h = phi.direct_image(&h);
        let round = phi.direct_image(&phi.counit(&phi.inverse_image(&f_h)));
        assert_eq!(round, f_h, "triangle 2 failed for H={subset:?}");
    }
}

#[test]
fn galois_connections_hold_over_all_subsets() {
    let phi = morphism(F_JSON);
    for u in subsets(&E_OBJ) {
        for v in subsets(&F_OBJ) {
            assert_eq!(
                phi.image(&u).is_subset(&v),
                u.is_subset(&phi.pullback_subobject(&v)),
                "∃_f ⊣ f* failed U={u:?} V={v:?}"
            );
            assert_eq!(
                phi.pullback_subobject(&v).is_subset(&u),
                v.is_subset(&phi.universal_image(&u)),
                "f* ⊣ ∀_f failed U={u:?} V={v:?}"
            );
        }
    }

    // φ is non-injective (planner, reviewer ↦ head), so ∃_f ≠ ∀_f.
    let only_planner: BTreeSet<String> = [String::from("planner")].into_iter().collect();
    assert_ne!(phi.image(&only_planner), phi.universal_image(&only_planner));
}

#[test]
fn morphism_section_shows_the_adjoint_triple() {
    let phi = morphism(F_JSON);
    let agentic = agentic_topos("{}");
    let membrane = membrane_topos("{}");

    // Agentic side, U = {planner}: ∃f lands on head, f*∃f pulls back the whole
    // fiber {planner, reviewer}, and ∀f({planner}) is empty.
    let u: BTreeSet<String> = [String::from("planner")].into_iter().collect();
    let Section::Text(model) = phi.section(&agentic, &membrane, &u, MorphismSide::Domain) else {
        panic!("morphism section is text");
    };
    assert!(model.text.contains("∃f(U) = {head}"), "{}", model.text);
    assert!(model.text.contains("∀f(U) = ∅"), "{}", model.text);
    assert!(model.text.contains("f*∃f(U) = {planner, reviewer}"), "{}", model.text);
    assert!(model.text.contains("f merges planner, reviewer → head"), "{}", model.text);

    // U = {planner, reviewer}: ∀f now reaches head and f*∃f no longer grows U.
    let u: BTreeSet<String> = [String::from("planner"), String::from("reviewer")].into_iter().collect();
    let Section::Text(model) = phi.section(&agentic, &membrane, &u, MorphismSide::Domain) else {
        panic!("morphism section is text");
    };
    assert!(model.text.contains("∀f(U) = {head}"), "{}", model.text);
    assert!(!model.text.contains("information lost"), "{}", model.text);

    // Membrane side, U = {head}: f* pulls back to the whole fiber in E.
    let u: BTreeSet<String> = [String::from("head")].into_iter().collect();
    let Section::Text(model) = phi.section(&agentic, &membrane, &u, MorphismSide::Codomain) else {
        panic!("morphism section is text");
    };
    assert!(model.text.contains("f*(U) = {planner, reviewer}"), "{}", model.text);
    assert!(model.text.contains("Planner, Reviewer"), "{}", model.text);
}

#[test]
fn status_table_marks_each_check() {
    let Section::Table(model) = status_table("Check", "Holds", &[("a".to_owned(), true), ("b".to_owned(), false)])
    else {
        panic!("status table is a table");
    };
    assert_eq!(model.rows.len(), 2);
    assert!(matches!(&model.rows[0].cells[1], TableCell::Status { label, .. } if label == "ok"));
    assert!(matches!(&model.rows[1].cells[1], TableCell::Status { label, .. } if label == "error"));
}

#[test]
fn validate_rejects_bad_sort_maps() {
    let agentic = agentic_topos("{}");
    let membrane = membrane_topos("{}");

    let non_total = GeometricMorphism {
        id: "f".to_owned(),
        domain: "topos-agentic".to_owned(),
        codomain: "topos-membrane".to_owned(),
        object_sorts: pairs(&[("planner", "head"), ("coder", "tail"), ("reviewer", "head")]),
        morphism_sorts: pairs(&[
            ("continues", "tail_bond"),
            ("shares", "lateral"),
            ("consolidates", "solvation"),
        ]),
    };
    assert!(matches!(
        non_total.validate(&agentic, &membrane),
        Err(MorphismError::NotTotal { part: "object", sort }) if sort == "memory"
    ));

    let outside = GeometricMorphism {
        id: "f".to_owned(),
        domain: "topos-agentic".to_owned(),
        codomain: "topos-membrane".to_owned(),
        object_sorts: pairs(&[
            ("planner", "head"),
            ("coder", "tail"),
            ("reviewer", "head"),
            ("memory", "ghost"),
        ]),
        morphism_sorts: pairs(&[
            ("continues", "tail_bond"),
            ("shares", "lateral"),
            ("consolidates", "solvation"),
        ]),
    };
    assert!(matches!(
        outside.validate(&agentic, &membrane),
        Err(MorphismError::OutsideCodomain { part: "object", sort, image })
            if sort == "memory" && image == "ghost"
    ));

    let non_commuting = GeometricMorphism {
        id: "f".to_owned(),
        domain: "topos-agentic".to_owned(),
        codomain: "topos-membrane".to_owned(),
        object_sorts: pairs(&[
            ("planner", "head"),
            ("coder", "head"),
            ("reviewer", "head"),
            ("memory", "solvent"),
        ]),
        morphism_sorts: pairs(&[
            ("continues", "tail_bond"),
            ("shares", "lateral"),
            ("consolidates", "solvation"),
        ]),
    };
    assert!(matches!(
        non_commuting.validate(&agentic, &membrane),
        Err(MorphismError::NotCommuting { part: "object", at }) if at == "coder"
    ));

    // The authored morphism validates.
    morphism(F_JSON).validate(&agentic, &membrane).unwrap();
}

fn pairs(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())).collect()
}

#[test]
fn compose_builds_the_composite_sort_map() {
    let f = morphism(F_JSON); // agentic → membrane
    let p_membrane = morphism(PM_JSON); // membrane → physics
    let composed = f.compose(&p_membrane).unwrap(); // agentic → physics

    assert_eq!(composed.domain, "topos-agentic");
    assert_eq!(composed.codomain, "topos-physics");
    // p_membrane ∘ f == p_agentic on sorts (the base-commuting law).
    assert_eq!(composed.object_sorts, morphism(PA_JSON).object_sorts);
    assert_eq!(composed.morphism_sorts, morphism(PA_JSON).morphism_sorts);

    assert!(matches!(f.compose(&f), Err(MorphismError::ComposeMismatch { .. })));
}

#[test]
fn morphism_commutes_over_the_base() {
    let f = morphism(F_JSON);
    let p_agentic = morphism(PA_JSON);
    let p_membrane = morphism(PM_JSON);
    assert!(f.commutes_over(&p_agentic, &p_membrane));
    f.validate_over(&p_agentic, &p_membrane).unwrap();

    let p_membrane_altered = morphism(PM_ALT_JSON);
    assert!(!f.commutes_over(&p_agentic, &p_membrane_altered));
    assert!(matches!(
        f.validate_over(&p_agentic, &p_membrane_altered),
        Err(MorphismError::NotCommuting { .. })
    ));
}

// --- Physics section -------------------------------------------------------

#[test]
fn physics_section_is_regime_invariant() {
    let site = demo_site();
    let agentic = agentic_topos("{}");
    let membrane = membrane_topos("{}");

    let section_a = agentic.physics_section(&site, &physics_a());
    let section_m = membrane.physics_section(&site, &physics_b());
    assert_eq!(section_a, section_m);

    // Spot-check the shared engine parameters.
    assert_eq!(section_a.objects[0], ("p1".to_owned(), "hydrophilic".to_owned(), 9.0, 1.0));
    assert_eq!(section_a.objects[1], ("c1".to_owned(), "hydrophobic".to_owned(), 3.0, 2.0));

    // Altering p_membrane breaks the invariance.
    let physics_altered = Physics::new(&base(), &morphism(PM_ALT_JSON), &membrane).unwrap();
    let section_altered = membrane.physics_section(&site, &physics_altered);
    assert_ne!(section_a, section_altered);
}

// --- Sheaf rendering through a physics base ---------------------------------

#[test]
fn assembly_and_physics_sheaves_render() {
    let sheaves = r##"{"asm":{"kind":"assembly","morphism_sorts":["continues","shares","consolidates"]},"asm_badges":{"kind":"assembly_badges","morphism_sorts":["continues","shares","consolidates"]},"phys":{"kind":"physics","scope":"objects"}}"##;
    let agentic = agentic_topos(sheaves);
    let sections = agentic.global_sections(&demo_site(), &physics_a()).unwrap();

    match sections.get("asm").unwrap() {
        Section::Table(table) => {
            assert_eq!(table.rows.len(), 1);
            let cells = &table.rows[0].cells;
            assert_eq!(cell_text(&cells[0]), "p1"); // representative
            assert_eq!(cell_text(&cells[1]), "4"); // V
            assert_eq!(cell_text(&cells[2]), "3"); // E
            assert_eq!(cell_text(&cells[3]), "0"); // F
            assert_eq!(cell_text(&cells[8]), "handoff chain"); // stage prose
        }
        other => panic!("asm is not a table: {other:?}"),
    }

    match sections.get("asm_badges").unwrap() {
        Section::Badges(badges) => assert_eq!(badges.len(), 1),
        other => panic!("asm_badges is not badges: {other:?}"),
    }

    match sections.get("phys").unwrap() {
        Section::Table(table) => {
            assert_eq!(table.rows.len(), 4);
            let first = &table.rows[0].cells;
            assert_eq!(cell_text(&first[0]), "p1");
            assert_eq!(cell_text(&first[1]), "hydrophilic");
            assert_eq!(cell_text(&first[2]), "9");
            assert_eq!(cell_text(&first[3]), "1");
        }
        other => panic!("phys is not a table: {other:?}"),
    }
}

fn cell_text(cell: &TableCell) -> &str {
    match cell {
        TableCell::Text(text) => text.as_str(),
        other => panic!("cell is not text: {other:?}"),
    }
}

// --- Assembly stage classification -----------------------------------------

fn bond_sorting() -> BTreeMap<String, String> {
    BTreeMap::from([("bond".to_owned(), "bond".to_owned())])
}

fn bond_sorts() -> BTreeSet<String> {
    [String::from("bond")].into_iter().collect()
}

fn site_from(n: usize, edges: &[(usize, usize)]) -> Site {
    let objects = (0..n)
        .map(|i| Object {
            id: format!("v{i}"),
            title: format!("v{i}"),
            kind: "node".to_owned(),
            tags: Vec::new(),
            fields: BTreeMap::new(),
        })
        .collect();
    let morphisms = edges
        .iter()
        .map(|(a, b)| Morphism {
            domain: format!("v{a}"),
            codomain: format!("v{b}"),
            kind: Some("bond".to_owned()),
        })
        .collect();
    Site { objects, morphisms }
}

fn single_stage(site: &Site) -> Stage {
    let components = assembly(site, &bond_sorting(), &bond_sorts());
    assert_eq!(components.len(), 1, "expected one component, got {}", components.len());
    components[0].stage
}

const ICOSAHEDRON: [(usize, usize); 30] = [
    (0, 1), (0, 2), (0, 3), (0, 4), (0, 5),
    (1, 2), (2, 3), (3, 4), (4, 5), (5, 1),
    (11, 6), (11, 7), (11, 8), (11, 9), (11, 10),
    (6, 7), (7, 8), (8, 9), (9, 10), (10, 6),
    (1, 6), (1, 7), (2, 7), (2, 8), (3, 8),
    (3, 9), (4, 9), (4, 10), (5, 10), (5, 6),
];

const SQUARE_TUBE: [(usize, usize); 16] = [
    (0, 1), (1, 2), (2, 3), (3, 0),
    (4, 5), (5, 6), (6, 7), (7, 4),
    (0, 4), (1, 5), (2, 6), (3, 7),
    (0, 5), (1, 6), (2, 7), (3, 4),
];

#[test]
fn assembly_classifies_each_stage() {
    assert_eq!(single_stage(&site_from(4, &[(0, 1), (1, 2), (2, 3)])), Stage::Chain);
    assert_eq!(single_stage(&site_from(4, &[(0, 1), (0, 2), (0, 3)])), Stage::Branched);
    assert_eq!(single_stage(&site_from(4, &[(0, 1), (1, 2), (2, 3), (3, 0)])), Stage::Ring);
    assert_eq!(
        single_stage(&site_from(4, &[(0, 1), (0, 2), (1, 2), (1, 3), (2, 3)])),
        Stage::Sheet
    );
    assert_eq!(single_stage(&site_from(8, &SQUARE_TUBE)), Stage::Tube);
    assert_eq!(single_stage(&site_from(12, &ICOSAHEDRON)), Stage::Membrane);
}

#[test]
fn sheet_tube_membrane_invariants() {
    let sheet = assembly(&site_from(4, &[(0, 1), (0, 2), (1, 2), (1, 3), (2, 3)]), &bond_sorting(), &bond_sorts());
    assert_eq!((sheet[0].v, sheet[0].e, sheet[0].f, sheet[0].chi, sheet[0].boundary), (4, 5, 2, 1, 4));

    let tube = assembly(&site_from(8, &SQUARE_TUBE), &bond_sorting(), &bond_sorts());
    assert_eq!((tube[0].v, tube[0].e, tube[0].f, tube[0].chi, tube[0].boundary), (8, 16, 8, 0, 8));

    let membrane = assembly(&site_from(12, &ICOSAHEDRON), &bond_sorting(), &bond_sorts());
    assert_eq!(
        (membrane[0].v, membrane[0].e, membrane[0].f, membrane[0].chi, membrane[0].boundary),
        (12, 30, 20, 2, 0)
    );
}

#[test]
fn assembly_orders_components_and_counts_stages() {
    // A chain (v0..v3) and a disjoint ring (v4..v7).
    let site = site_from(8, &[(0, 1), (1, 2), (2, 3), (4, 5), (5, 6), (6, 7), (7, 4)]);
    let components = assembly(&site, &bond_sorting(), &bond_sorts());
    assert_eq!(components.len(), 2);
    assert_eq!(components[0].representative, "v0");
    assert_eq!(components[0].stage, Stage::Chain);
    assert_eq!(components[1].representative, "v4");
    assert_eq!(components[1].stage, Stage::Ring);
}

// --- Restriction -----------------------------------------------------------

fn identity(kinds: &[&str]) -> BTreeMap<String, String> {
    kinds.iter().map(|kind| ((*kind).to_owned(), (*kind).to_owned())).collect()
}

const E_MOR: [&str; 3] = ["continues", "shares", "consolidates"];

#[test]
fn object_restriction_composes_by_intersection_and_is_identity_on_all_sorts() {
    let site = site_from_lines();
    let sorting = identity(&E_OBJ);
    for u in subsets(&E_OBJ) {
        for v in subsets(&E_OBJ) {
            let both: BTreeSet<String> = u.intersection(&v).cloned().collect();
            assert_eq!(
                site.restrict_objects(&sorting, &u).restrict_objects(&sorting, &v),
                site.restrict_objects(&sorting, &both),
                "U={u:?} V={v:?}"
            );
        }
    }
    let all: BTreeSet<String> = E_OBJ.iter().map(|sort| (*sort).to_owned()).collect();
    assert_eq!(site.restrict_objects(&sorting, &all), site);
}

#[test]
fn morphism_restriction_composes_by_intersection_and_is_identity_on_all_sorts() {
    let site = site_from_lines();
    let sorting = identity(&E_MOR);
    for u in subsets(&E_MOR) {
        for v in subsets(&E_MOR) {
            let both: BTreeSet<String> = u.intersection(&v).cloned().collect();
            assert_eq!(
                site.restrict_morphisms(&sorting, &u).restrict_morphisms(&sorting, &v),
                site.restrict_morphisms(&sorting, &both),
                "U={u:?} V={v:?}"
            );
        }
    }
    let all: BTreeSet<String> = E_MOR.iter().map(|sort| (*sort).to_owned()).collect();
    assert_eq!(site.restrict_morphisms(&sorting, &all), site);
}

/// The inline workspace with its `load` panel bound as a table.
fn table_workspace() -> String {
    WS_MIN.replace(r#""kind":"gauges""#, r#""kind":"table""#)
}

fn table_topos(load: &str, edits: &str) -> Topos {
    let json = format!(
        r##"{{"spec_version":1,"id":"topos-agentic","regime":"Agentic traces","object_sorting":{AGENTIC_OSORT},"morphism_sorting":{AGENTIC_MSORT},"stalks":{AGENTIC_STALKS},"sheaves":{{"legend":{{"kind":"text","heading":"Legend"}},"load":{load}}},"edits":{edits},"workspace":{ws}}}"##,
        ws = table_workspace(),
    );
    Topos::from_json_str(&json).unwrap()
}

fn table_rows(topos: &Topos, site: &Site) -> Vec<String> {
    match topos.global_sections(site, &physics_a()).unwrap().get("load").unwrap() {
        Section::Table(table) => table.rows.iter().map(|row| cell_text(&row.cells[0]).to_owned()).collect(),
        other => panic!("load is not a table: {other:?}"),
    }
}

#[test]
fn a_restricted_table_has_rows_only_for_its_sorts() {
    let topos = table_topos(
        r##"{"kind":"table","scope":"objects","restrict":["coder","reviewer"],"columns":[{"key":"id","title":"Id","width":{"flex":{"weight":1}},"align":"left","cell":{"cell":"text","expr":"id"}}]}"##,
        "{}",
    );
    topos.validate().unwrap();
    assert_eq!(table_rows(&topos, &demo_site()), ["c1", "r1"]);
}

#[test]
fn an_optional_field_reads_empty_where_a_required_one_fails() {
    let column = |expr: &str| {
        format!(
            r##"{{"kind":"table","scope":"objects","restrict":["coder"],"columns":[{{"key":"f","title":"F","width":{{"flex":{{"weight":1}}}},"align":"left","cell":{{"cell":"text","expr":"{expr}"}}}}]}}"##
        )
    };
    let site = demo_site();
    assert_eq!(table_rows(&table_topos(&column("field?:missing"), "{}"), &site), [""]);
    assert_eq!(table_rows(&table_topos(&column("field?:energy"), "{}"), &site), table_rows(&table_topos(&column("field:energy"), "{}"), &site));
    assert!(matches!(
        table_topos(&column("field:missing"), "{}").global_sections(&site, &physics_a()),
        Err(SheafError::MissingField { .. })
    ));
}

#[test]
fn a_morphism_table_restricts_by_morphism_sort() {
    let topos = table_topos(
        r##"{"kind":"table","scope":"morphisms","restrict":["shares"],"columns":[{"key":"s","title":"S","width":{"flex":{"weight":1}},"align":"left","cell":{"cell":"text","expr":"source"}}]}"##,
        "{}",
    );
    topos.validate().unwrap();
    assert_eq!(table_rows(&topos, &demo_site()), ["c1"]);
}

#[test]
fn validate_rejects_a_restriction_to_an_undeclared_sort() {
    let topos = table_topos(
        r##"{"kind":"table","scope":"objects","restrict":["head"],"columns":[]}"##,
        "{}",
    );
    assert_eq!(
        topos.validate(),
        Err(ToposError::RestrictOutsideSorts { binding: "load".to_owned(), sort: "head".to_owned() })
    );
    let morphisms = table_topos(
        r##"{"kind":"table","scope":"morphisms","restrict":["coder"],"columns":[]}"##,
        "{}",
    );
    assert!(matches!(morphisms.validate(), Err(ToposError::RestrictOutsideSorts { .. })));
}

#[test]
fn a_restricted_flamegraph_drops_roots_and_traversed_objects_outside_its_sorts() {
    // p1 -continues-> c1 -continues-> r1: restricted to {coder, reviewer}, p1 is
    // gone, so c1 roots the only remaining chain.
    let site = Site {
        objects: vec![object("p1", "planner", "1"), object("c1", "coder", "2"), object("r1", "reviewer", "3")],
        morphisms: vec![typed("p1", "c1", "continues"), typed("c1", "r1", "continues")],
    };
    let sheaves = r##"{"flame":{"kind":"flamegraph","morphism_kind":"continues","value":"field:energy","restrict":["coder","reviewer"]}}"##;
    let topos = agentic_topos(sheaves);
    let Section::Flamegraph(spans) = topos.global_sections(&site, &physics_a()).unwrap().get("flame").unwrap().clone() else {
        panic!("not a flamegraph");
    };
    let frames: Vec<(&str, u16)> = spans.iter().map(|span| (span.label.as_str(), span.depth)).collect();
    assert_eq!(frames, [("C1", 0), ("R1", 1)]);
}

#[test]
fn restricted_badges_count_only_their_sorts() {
    let sheaves = r##"{"roles":{"kind":"badges","group":"kinds","restrict":["memory"]}}"##;
    let topos = agentic_topos(sheaves);
    let Section::Badges(badges) = topos.global_sections(&demo_site(), &physics_a()).unwrap().get("roles").unwrap().clone() else {
        panic!("not badges");
    };
    assert_eq!(badges.len(), 1);
}

// --- Gluing ----------------------------------------------------------------

fn anchored(id: &str, kind: &str) -> Object {
    Object { id: id.to_owned(), title: id.to_owned(), kind: kind.to_owned(), tags: Vec::new(), fields: BTreeMap::new() }
}

fn side(objects: Vec<Object>, morphisms: Vec<Morphism>) -> Site {
    Site { objects, morphisms }
}

#[test]
fn glue_is_associative() {
    let a = side(vec![anchored("r1", "repo"), anchored("s1", "session")], vec![typed("s1", "r1", "in_repo")]);
    let b = side(vec![anchored("r1", "repo"), anchored("p1", "policy")], vec![typed("p1", "r1", "governs")]);
    let c = side(vec![anchored("r1", "repo"), anchored("r2", "repo"), anchored("q1", "queued")], vec![typed("q1", "r2", "targets")]);
    let left = a.glue(&b, "repo").unwrap().glue(&c, "repo").unwrap();
    let right = a.glue(&b.glue(&c, "repo").unwrap(), "repo").unwrap();
    assert_eq!(left, right);
    assert_eq!(left.objects.iter().filter(|object| object.id == "r1").count(), 1);
}

#[test]
fn restricting_the_glued_site_to_one_sides_kinds_returns_that_side() {
    let trace = side(vec![anchored("r1", "repo"), anchored("s1", "session")], vec![typed("s1", "r1", "in_repo")]);
    let control = side(vec![anchored("p1", "policy"), anchored("r1", "repo")], vec![typed("p1", "r1", "governs")]);
    let glued = trace.glue(&control, "repo").unwrap();

    let trace_kinds: BTreeSet<String> = ["repo", "session"].map(str::to_owned).into();
    let restricted = glued.restrict_objects(&identity(&["repo", "session", "policy"]), &trace_kinds);
    assert_eq!(restricted, trace);

    let control_kinds: BTreeSet<String> = ["repo", "policy"].map(str::to_owned).into();
    let restricted = glued.restrict_objects(&identity(&["repo", "session", "policy"]), &control_kinds);
    assert_eq!(restricted.morphisms, control.morphisms);
    let mut objects = restricted.objects.clone();
    objects.sort_by(|x, y| x.id.cmp(&y.id));
    let mut expected = control.objects.clone();
    expected.sort_by(|x, y| x.id.cmp(&y.id));
    assert_eq!(objects, expected);
}

#[test]
fn glue_refuses_disagreeing_identified_objects_and_other_collisions() {
    let a = side(vec![anchored("r1", "repo")], Vec::new());
    let mut differing = anchored("r1", "repo");
    differing.title = "elsewhere".to_owned();
    assert_eq!(
        a.glue(&side(vec![differing], Vec::new()), "repo"),
        Err(crate::SiteError::GlueDisagrees { id: "r1".to_owned() })
    );
    assert_eq!(
        a.glue(&side(vec![anchored("r1", "session")], Vec::new()), "repo"),
        Err(crate::SiteError::IdCollision { id: "r1".to_owned() })
    );
}

// --- Emitters --------------------------------------------------------------

const LINES_EMITTER: &str = r##"{"engine":"pest","object":"N|{id}|{title}|{kind}|{tags}|{fields}"}"##;

fn with_emitter(package: &str, emitter: &str) -> Result<CompiledGrammar, GrammarError> {
    let mut package = GrammarPackage::from_json_str(package).unwrap();
    package.emitter = Some(serde_json::from_str(emitter).unwrap());
    package.compile()
}

fn assert_emitter_laws(grammar: &CompiledGrammar, site: &Site) {
    let mut supports: Vec<BTreeSet<String>> = site.objects.iter().map(|object| BTreeSet::from([object.id.clone()])).collect();
    supports.push(site.objects.iter().map(|object| object.id.clone()).collect());
    for support in supports {
        let bytes = grammar.emit(site, &support).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        let parsed = grammar.parse(&text).unwrap();
        let expected: Vec<Object> = site.objects.iter().filter(|object| support.contains(&object.id)).cloned().collect();
        assert_eq!(parsed.objects, expected, "round trip on {support:?}");
        assert_eq!(grammar.emit(&parsed, &support).unwrap(), bytes, "idempotent on {support:?}");
    }
}

#[test]
fn pest_emitter_round_trips_and_is_idempotent() {
    let grammar = with_emitter(GRAMMAR_LINES, LINES_EMITTER).unwrap();
    assert_emitter_laws(&grammar, &tagged(site_from_lines()));
}

/// The fixtures carry no tags; the laws must also hold for objects that do.
fn tagged(mut site: Site) -> Site {
    site.objects[0].tags = vec!["hot".to_owned(), "x2".to_owned()];
    site.objects[1].tags = vec!["cold".to_owned()];
    site
}

#[test]
fn json_emitter_round_trips_and_is_idempotent() {
    let grammar = with_emitter(GRAMMAR_JSON, r##"{"engine":"json"}"##).unwrap();
    let site = tagged(grammar.parse(DATA_JSON).unwrap());
    assert_emitter_laws(&grammar, &site);
}

#[test]
fn a_template_that_loses_data_or_names_an_unknown_hole_is_rejected() {
    let no_fields = r##"{"engine":"pest","object":"N|{id}|{title}|{kind}|{tags}|"}"##;
    assert!(matches!(with_emitter(GRAMMAR_LINES, no_fields), Err(GrammarError::EmitterLosesData(_))));
    let unknown = r##"{"engine":"pest","object":"N|{id}|{name}"}"##;
    assert!(matches!(with_emitter(GRAMMAR_LINES, unknown), Err(GrammarError::InvalidTemplate(_))));
    assert!(matches!(
        with_emitter(GRAMMAR_LINES, r##"{"engine":"json"}"##),
        Err(GrammarError::EmitterEngine { parser: "pest", emitter: "json" })
    ));
}

#[test]
fn the_json_emitter_refuses_a_field_its_map_does_not_carry() {
    let grammar = with_emitter(GRAMMAR_JSON, r##"{"engine":"json"}"##).unwrap();
    let mut site = grammar.parse(DATA_JSON).unwrap();
    site.objects[0].fields.insert("unmapped".to_owned(), "x".to_owned());
    let support = BTreeSet::from([site.objects[0].id.clone()]);
    assert!(matches!(grammar.emit(&site, &support), Err(GrammarError::EmitterLosesData(_))));
    assert_eq!(
        grammar.emit(&site, &BTreeSet::from(["nope".to_owned()])),
        Err(GrammarError::UnknownSupportObject("nope".to_owned()))
    );
}

// --- Section edits ---------------------------------------------------------

const STATUS_EDIT: &str = r##"{"triage":{"label":"Triage","sorts":["coder","reviewer"],"set":{"status":{"type":"enum","values":["ok","error"]},"energy":{"type":"int","min":0,"max":500}},"grammar":"trace-json"}}"##;
const EDIT_TABLE: &str = r##"{"kind":"table","scope":"objects","edits":["triage"],"columns":[{"key":"id","title":"Id","width":{"flex":{"weight":1}},"align":"left","cell":{"cell":"text","expr":"id"}}]}"##;

fn input(object: &str, values: &[(&str, &str)]) -> crate::EditInput {
    crate::EditInput {
        object: object.to_owned(),
        values: values.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(),
    }
}

#[test]
fn a_set_only_revision_changes_one_objects_data_and_nothing_else() {
    let topos = table_topos(EDIT_TABLE, STATUS_EDIT);
    topos.validate().unwrap();
    let site = site_from_lines();
    let (revised, revision) = topos.revise(&site, "triage", &input("c1", &[("status", "ok"), ("energy", " 07 ")])).unwrap();

    assert_eq!(revision.support, BTreeSet::from(["c1".to_owned()]));
    assert_eq!(revised.morphisms, site.morphisms);
    for (before, after) in site.objects.iter().zip(&revised.objects) {
        if before.id == "c1" {
            assert_eq!(after.fields["status"], "ok");
            assert_eq!(after.fields["energy"], "7");
            assert_eq!((&after.title, &after.kind, &after.tags), (&before.title, &before.kind, &before.tags));
        } else {
            assert_eq!(after, before);
        }
    }

    // Setting values already present is the identity revision.
    let (again, unchanged) = topos.revise(&revised, "triage", &input("c1", &[("status", "ok")])).unwrap();
    assert_eq!(again, revised);
    assert!(unchanged.support.is_empty());
}

#[test]
fn revise_refuses_bad_input_and_leaves_the_site_alone() {
    let topos = table_topos(EDIT_TABLE, STATUS_EDIT);
    let site = site_from_lines();
    let refused = [
        (topos.revise(&site, "nope", &input("c1", &[("status", "ok")])), "unknown edit"),
        (topos.revise(&site, "triage", &input("zz", &[("status", "ok")])), "unknown object"),
        (topos.revise(&site, "triage", &input("p1", &[("status", "ok")])), "planner is not a triage sort"),
        (topos.revise(&site, "triage", &input("c1", &[("t", "1")])), "undeclared field"),
        (topos.revise(&site, "triage", &input("c1", &[("status", "fine")])), "outside the enum"),
        (topos.revise(&site, "triage", &input("c1", &[("energy", "900")])), "outside the int range"),
        (topos.revise(&site, "triage", &input("c1", &[])), "no fields"),
    ];
    for (result, case) in refused {
        assert!(result.is_err(), "{case} should be refused");
    }
    assert_eq!(site, site_from_lines());
}

#[test]
fn each_row_offers_only_the_edits_for_its_objects_sort() {
    let topos = table_topos(EDIT_TABLE, STATUS_EDIT);
    let sections = topos.global_sections(&demo_site(), &physics_a()).unwrap();
    let offered: Vec<(&str, Vec<&str>)> = sections
        .row_edits("load")
        .unwrap()
        .iter()
        .map(|row| (row.object.as_str(), row.edits.iter().map(String::as_str).collect()))
        .collect();
    assert_eq!(offered, [("p1", vec![]), ("c1", vec!["triage"]), ("r1", vec!["triage"]), ("m1", vec![])]);
    assert_eq!(sections.row_edits("legend"), None);
}

#[test]
fn each_rows_id_names_the_row_edits_behind_it() {
    let topos = table_topos(EDIT_TABLE, STATUS_EDIT);
    let sections = topos.global_sections(&demo_site(), &physics_a()).unwrap();
    let Some(Section::Table(table)) = sections.get("load") else {
        panic!("load resolves to a table");
    };
    let row_ids: Vec<&str> = table.rows.iter().map(|row| row.id.as_str()).collect();
    let edit_ids: Vec<String> = sections.row_edits("load").unwrap().iter().map(crate::RowEdits::row_id).collect();
    assert_eq!(row_ids, edit_ids);
}

#[test]
fn validate_rejects_malformed_or_misplaced_edits() {
    let bad_sort = r##"{"triage":{"label":"T","sorts":["head"],"set":{"status":{"type":"text","max":5}},"grammar":"g"}}"##;
    assert!(matches!(table_topos(EDIT_TABLE, bad_sort).validate(), Err(ToposError::InvalidEdit { .. })));
    let empty_enum = r##"{"triage":{"label":"T","sorts":["coder"],"set":{"status":{"type":"enum","values":[]}},"grammar":"g"}}"##;
    assert!(matches!(table_topos(EDIT_TABLE, empty_enum).validate(), Err(ToposError::InvalidEdit { .. })));
    let missing = r##"{"kind":"table","scope":"objects","edits":["nope"],"columns":[]}"##;
    assert_eq!(
        table_topos(missing, "{}").validate(),
        Err(ToposError::UnknownEdit { binding: "load".to_owned(), edit: "nope".to_owned() })
    );
    let on_morphisms = r##"{"kind":"table","scope":"morphisms","edits":["triage"],"columns":[]}"##;
    assert!(matches!(table_topos(on_morphisms, STATUS_EDIT).validate(), Err(ToposError::InvalidEdit { .. })));
}

#[test]
fn a_write_intent_parses_back_to_the_revised_object() {
    let topos = table_topos(EDIT_TABLE, STATUS_EDIT);
    let grammar = with_emitter(GRAMMAR_JSON, r##"{"engine":"json"}"##).unwrap();
    let site = grammar.parse(DATA_JSON).unwrap();
    let (revised, revision) = topos.revise(&site, "triage", &input("c1", &[("status", "ok")])).unwrap();
    let intent = topos.write_intent(&grammar, &revised, &revision).unwrap();
    assert_eq!((intent.grammar.as_str(), intent.edit.as_str()), ("trace-json", "triage"));
    let written = grammar.parse(std::str::from_utf8(&intent.bytes).unwrap()).unwrap();
    assert_eq!(written.objects.len(), 1);
    assert_eq!(written.objects[0].fields["status"], "ok");

    let lines = with_emitter(GRAMMAR_LINES, LINES_EMITTER).unwrap();
    assert!(matches!(topos.write_intent(&lines, &revised, &revision), Err(crate::EditError::GrammarMismatch { .. })));
}

const NOTES_EDIT: &str = r##"{"annotate":{"label":"Annotate","sorts":["coder"],"set":{"notes":{"type":"lines","max":4},"status":{"type":"text","max":40}},"grammar":"trace-lines"}}"##;
const NOTES_TABLE: &str = r##"{"kind":"table","scope":"objects","edits":["annotate"],"columns":[{"key":"id","title":"Id","width":{"flex":{"weight":1}},"align":"left","cell":{"cell":"text","expr":"id"}}]}"##;

#[test]
fn a_value_the_line_format_cannot_carry_is_an_emit_error_not_corrupt_bytes() {
    let topos = table_topos(NOTES_TABLE, NOTES_EDIT);
    topos.validate().unwrap();
    let grammar = with_emitter(GRAMMAR_LINES, LINES_EMITTER).unwrap();
    let site = site_from_lines();
    // Newline, `;` and `|` delimit records, properties and fields; `=` inside a value is legal.
    for (field, value) in [("notes", "first\nsecond"), ("status", "a;b"), ("status", "a|b")] {
        let (revised, revision) = topos.revise(&site, "annotate", &input("c1", &[(field, value)])).unwrap();
        assert!(
            matches!(topos.write_intent(&grammar, &revised, &revision), Err(crate::EditError::Emit(GrammarError::EmitterLosesData(_)))),
            "{field} = {value:?}"
        );
    }
    let (revised, revision) = topos.revise(&site, "annotate", &input("c1", &[("status", "k=v")])).unwrap();
    assert!(topos.write_intent(&grammar, &revised, &revision).is_ok());
}

#[test]
fn a_revision_that_changed_nothing_has_nothing_to_write() {
    let topos = table_topos(EDIT_TABLE, STATUS_EDIT);
    let grammar = with_emitter(GRAMMAR_JSON, r##"{"engine":"json"}"##).unwrap();
    let site = grammar.parse(DATA_JSON).unwrap();
    let current = site.objects.iter().find(|o| o.id == "c1").unwrap().fields["status"].clone();
    let (revised, revision) = topos.revise(&site, "triage", &input("c1", &[("status", &current)])).unwrap();
    assert!(revision.support.is_empty());
    assert_eq!(topos.write_intent(&grammar, &revised, &revision), Err(crate::EditError::NothingToWrite("triage".to_owned())));
}

#[test]
fn glue_refuses_a_morphism_both_sides_carry_between_glued_objects() {
    let a = side(vec![anchored("r1", "repo"), anchored("r2", "repo")], vec![typed("r1", "r2", "mirrors")]);
    let b = side(vec![anchored("r1", "repo"), anchored("r2", "repo")], vec![typed("r1", "r2", "mirrors")]);
    assert_eq!(
        a.glue(&b, "repo"),
        Err(crate::SiteError::DuplicateMorphism { domain: "r1".to_owned(), codomain: "r2".to_owned() })
    );
    let c = side(vec![anchored("r1", "repo"), anchored("r2", "repo")], vec![typed("r2", "r1", "mirrors")]);
    assert_eq!(a.glue(&c, "repo").unwrap().morphisms.len(), 2);
}

#[test]
fn the_json_emitter_refuses_tags_when_its_map_has_no_tags_member() {
    let mut package: serde_json::Value = serde_json::from_str(GRAMMAR_JSON).unwrap();
    package["parser"]["node"].as_object_mut().unwrap().remove("tags");
    let grammar = with_emitter(&package.to_string(), r##"{"engine":"json"}"##).unwrap();
    let mut site = grammar.parse(DATA_JSON).unwrap();
    let support = BTreeSet::from([site.objects[0].id.clone()]);
    assert!(grammar.emit(&site, &support).is_ok());
    site.objects[0].tags = vec!["hot".to_owned()];
    assert!(matches!(grammar.emit(&site, &support), Err(GrammarError::EmitterLosesData(_))));
}

#[test]
fn edit_decode_rejects_unknown_keys_and_validation_rejects_max_zero() {
    let unknown_edit_key = r##"{"triage":{"label":"T","sorts":["coder"],"set":{"s":{"type":"text","max":5}},"grammar":"g","extra":1}}"##;
    let unknown_type_key = r##"{"triage":{"label":"T","sorts":["coder"],"set":{"s":{"type":"text","max":5,"min":1}},"grammar":"g"}}"##;
    for edits in [unknown_edit_key, unknown_type_key] {
        let json = format!(
            r##"{{"spec_version":1,"id":"t","regime":"r","object_sorting":{AGENTIC_OSORT},"morphism_sorting":{AGENTIC_MSORT},"stalks":{AGENTIC_STALKS},"sheaves":{{"legend":{{"kind":"text","heading":"L"}},"load":{EDIT_TABLE}}},"edits":{edits},"workspace":{ws}}}"##,
            ws = table_workspace(),
        );
        assert!(Topos::from_json_str(&json).is_err(), "{edits}");
    }
    for ty in ["text", "lines"] {
        let edits = format!(r##"{{"triage":{{"label":"T","sorts":["coder"],"set":{{"s":{{"type":"{ty}","max":0}}}},"grammar":"g"}}}}"##);
        let table = EDIT_TABLE;
        assert!(matches!(table_topos(table, &edits).validate(), Err(ToposError::InvalidEdit { .. })), "{ty}");
    }
}

// --- Table row ids ------------------------------------------------------------

const EDGE_KINDS: [&str; 3] = ["continues", "shares", "consolidates"];

/// One table sheaf per row-id format: object and morphism rows, assembly
/// components, physics objects and physics bonds.
fn row_id_topos() -> Topos {
    let column = |key: &str, expr: &str| {
        format!(r##"{{"key":"{key}","title":"{key}","width":{{"flex":{{"weight":1}}}},"align":"left","cell":{{"cell":"text","expr":"{expr}"}}}}"##)
    };
    let sheaves = format!(
        r##"{{"objs":{{"kind":"table","scope":"objects","edits":["triage"],"columns":[{id}]}},"edges":{{"kind":"table","scope":"morphisms","columns":[{source},{target},{edge}]}},"asm":{{"kind":"assembly","morphism_sorts":["continues","shares","consolidates"]}},"po":{{"kind":"physics","scope":"objects"}},"pm":{{"kind":"physics","scope":"morphisms"}}}}"##,
        id = column("id", "id"),
        source = column("from", "source"),
        target = column("to", "target"),
        edge = column("kind", "edge_kind"),
    );
    let mut topos = agentic_topos(&sheaves);
    topos.edits = serde_json::from_str(STATUS_EDIT).unwrap();
    topos
}

fn row_ids(sections: &crate::GlobalSections, binding: &str) -> Vec<String> {
    match sections.get(binding) {
        Some(Section::Table(table)) => table.rows.iter().map(|row| row.id.clone()).collect(),
        other => panic!("{binding} is not a table: {other:?}"),
    }
}

#[test]
fn parallel_morphisms_get_distinct_row_ids_named_by_endpoints_and_kind() {
    let mut site = demo_site();
    site.morphisms = vec![
        typed("p1", "c1", "continues"),
        typed("p1", "c1", "shares"),
        typed("p1", "c1", "continues"),
        Morphism { domain: "p1".to_owned(), codomain: "c1".to_owned(), kind: None },
    ];
    let sections = row_id_topos().global_sections(&site, &physics_a()).unwrap();
    let expected = [
        "p1->c1:continues",
        "p1->c1:shares",
        "p1->c1:continues#2",
        "p1->c1",
    ];
    assert_eq!(row_ids(&sections, "edges"), expected.map(|id| format!("morphism:{id}")));
    assert_eq!(row_ids(&sections, "pm"), expected.map(|id| format!("bond:{id}")));
}

#[test]
fn assembly_rows_are_identified_by_their_component() {
    // Two disjoint pairs: every component has β0 = 1, so β0 cannot name a row.
    let mut site = demo_site();
    site.morphisms = vec![typed("p1", "c1", "continues"), typed("r1", "m1", "consolidates")];
    let sections = row_id_topos().global_sections(&site, &physics_a()).unwrap();
    assert_eq!(row_ids(&sections, "asm"), ["component:p1", "component:r1"]);
}

#[test]
fn status_table_row_ids_stay_unique_when_labels_repeat_or_look_like_suffixes() {
    let rows = [("a", true), ("a", false), ("a#2", true), ("a", true)].map(|(label, ok)| (label.to_owned(), ok));
    let Section::Table(model) = status_table("Check", "Holds", &rows) else {
        panic!("status table is a table");
    };
    let ids: Vec<&str> = model.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, ["gate:a", "gate:a#2", "gate:a#2#2", "gate:a#3"]);
}

/// Sites with unique object ids and morphisms inside up to three groups of
/// objects, so a site often has several components: parallel, repeated,
/// untyped and self-loop morphisms included.
pub(crate) fn arb_site() -> impl Strategy<Value = Site> {
    (2usize..8, 1usize..4).prop_flat_map(|(objects, groups)| {
        let kinds = prop::collection::vec(0..E_OBJ.len(), objects);
        let edge = (0..objects, 0..objects, prop::option::of(0..EDGE_KINDS.len()));
        (kinds, prop::collection::vec(edge, 0..16)).prop_map(move |(kinds, edges)| Site {
            objects: kinds.iter().enumerate().map(|(at, kind)| object(&format!("o{at}"), E_OBJ[*kind], "5")).collect(),
            morphisms: edges
                .into_iter()
                .filter(|(from, to, _)| from % groups == to % groups)
                .map(|(from, to, kind)| Morphism {
                    domain: format!("o{from}"),
                    codomain: format!("o{to}"),
                    kind: kind.map(|kind| EDGE_KINDS[kind].to_owned()),
                })
                .collect(),
        })
    })
}

#[test]
fn every_table_section_lists_unique_row_ids_that_name_their_rows() {
    let topos = row_id_topos();
    let physics = physics_a();
    let extra = (0usize..64, 0usize..64, prop::option::of(0..EDGE_KINDS.len()));
    proptest!(|(site in arb_site(), extra in extra)| {
        let sections = topos.global_sections(&site, &physics).unwrap();
        for binding in ["objs", "edges", "asm", "po", "pm"] {
            let ids = row_ids(&sections, binding);
            let distinct: BTreeSet<&String> = ids.iter().collect();
            prop_assert_eq!(distinct.len(), ids.len(), "{} repeats a row id: {:?}", binding, ids);
        }

        // One row per object, morphism or bond: no row is dropped to make ids unique.
        prop_assert_eq!(row_ids(&sections, "objs").len(), site.objects.len());
        prop_assert_eq!(row_ids(&sections, "po").len(), site.objects.len());
        prop_assert_eq!(row_ids(&sections, "edges").len(), site.morphisms.len());
        prop_assert_eq!(row_ids(&sections, "pm").len(), site.morphisms.len());

        // An object row's id is the id of the row edits behind it.
        let edits: Vec<String> = sections.row_edits("objs").unwrap().iter().map(crate::RowEdits::row_id).collect();
        prop_assert_eq!(row_ids(&sections, "objs"), edits);

        // A morphism row's id names its endpoints and kind: it follows the
        // morphism, not its position among parallel ones.
        for (id, morphism) in row_ids(&sections, "edges").iter().zip(&site.morphisms) {
            let named = match &morphism.kind {
                Some(kind) => format!("morphism:{}->{}:{kind}", morphism.domain, morphism.codomain),
                None => format!("morphism:{}->{}", morphism.domain, morphism.codomain),
            };
            prop_assert!(*id == named || id.starts_with(&format!("{named}#")), "{} does not name {:?}", id, morphism);
        }

        // Appending a morphism renames no existing row.
        let mut grown = site.clone();
        grown.morphisms.push(Morphism {
            domain: format!("o{}", extra.0 % site.objects.len()),
            codomain: format!("o{}", extra.1 % site.objects.len()),
            kind: extra.2.map(|kind| EDGE_KINDS[kind].to_owned()),
        });
        let after = topos.global_sections(&grown, &physics).unwrap();
        for binding in ["edges", "pm"] {
            let before = row_ids(&sections, binding);
            prop_assert_eq!(&row_ids(&after, binding)[..before.len()], &before[..], "{} renamed a row", binding);
        }
    });
}

// --- Single-binding sections --------------------------------------------------

/// Every sheaf kind that restricts, edits or reads morphisms, beside the
/// unrestricted ones of `row_id_topos`.
fn restricted_topos() -> Topos {
    let column = |expr: &str| format!(r##"{{"key":"c","title":"C","width":{{"flex":{{"weight":1}}}},"align":"left","cell":{{"cell":"text","expr":"{expr}"}}}}"##);
    let sheaves = format!(
        r##"{{"legend":{{"kind":"text","heading":"Legend"}},"roles":{{"kind":"badges","group":"kinds","restrict":["coder","reviewer"]}},"flame":{{"kind":"flamegraph","morphism_kind":"continues","value":"field:energy","restrict":["planner","coder","reviewer"]}},"load":{{"kind":"table","scope":"objects","restrict":["coder","reviewer"],"edits":["triage"],"columns":[{objects}]}},"shared":{{"kind":"table","scope":"morphisms","restrict":["shares"],"columns":[{morphisms}]}}}}"##,
        objects = column("id"),
        morphisms = column("source"),
    );
    let mut topos = agentic_topos(&sheaves);
    topos.edits = serde_json::from_str(STATUS_EDIT).unwrap();
    topos
}

#[test]
fn one_binding_resolves_to_what_global_sections_holds_for_it() {
    let physics = physics_a();
    let topoi = [row_id_topos(), restricted_topos()];
    proptest!(|(site in arb_site())| {
        for topos in &topoi {
            let all = topos.global_sections(&site, &physics).unwrap();
            for (binding, expected) in all.iter() {
                let one = topos.section(&site, &physics, binding).unwrap();
                prop_assert_eq!(one.as_ref(), Some(expected), "{}", binding);
            }
            prop_assert_eq!(topos.section(&site, &physics, "unbound").unwrap(), None);
        }
    });
}

#[test]
fn one_binding_matches_global_sections_on_the_fixture_site() {
    let physics = physics_a();
    for site in [demo_site(), site_from_lines()] {
        let topos = restricted_topos();
        let all = topos.global_sections(&site, &physics).unwrap();
        assert_eq!(all.len(), 5);
        for (binding, expected) in all.iter() {
            assert_eq!(topos.section(&site, &physics, binding).unwrap().as_ref(), Some(expected), "{binding}");
        }
    }
}

#[test]
fn one_binding_is_not_evaluated_with_the_rest() {
    let sheaves = r##"{"roles":{"kind":"badges","group":"kinds"},"flame":{"kind":"flamegraph","root":"absent","morphism_kind":"continues","value":"field:energy"}}"##;
    let topos = agentic_topos(sheaves);
    let (site, physics) = (demo_site(), physics_a());
    assert!(topos.global_sections(&site, &physics).is_err(), "the flamegraph root is not in the site");
    assert!(matches!(topos.section(&site, &physics, "roles"), Ok(Some(Section::Badges(_)))));
    assert!(topos.section(&site, &physics, "flame").is_err());
}

#[test]
fn one_binding_refuses_a_site_with_an_unsorted_kind_like_global_sections() {
    let topos = restricted_topos();
    let mut site = demo_site();
    site.objects.push(object("x1", "alien", "1"));
    assert!(matches!(topos.section(&site, &physics_a(), "legend"), Err(SheafError::UnsortedObjectKind(_))));
    assert!(matches!(topos.global_sections(&site, &physics_a()), Err(SheafError::UnsortedObjectKind(_))));
}

// --- Typed table columns ------------------------------------------------------

fn column_json(key: &str, kind: Option<&str>, cell: &str) -> String {
    column_json_aligned(key, kind, Some("left"), cell)
}

fn column_json_aligned(key: &str, kind: Option<&str>, align: Option<&str>, cell: &str) -> String {
    let kind = kind.map_or(String::new(), |kind| format!(r##""kind":"{kind}","##));
    let align = align.map_or(String::new(), |align| format!(r##""align":"{align}","##));
    format!(r##"{{"key":"{key}","title":"{key}","width":{{"flex":{{"weight":1}}}},{align}{kind}"cell":{cell}}}"##)
}

fn text_cell(expr: &str) -> String {
    format!(r##"{{"cell":"text","expr":"{expr}"}}"##)
}

fn meter_cell(value: &str, scale: f64) -> String {
    format!(r##"{{"cell":"meter","value":"{value}","scale":{scale:?},"text":"{value}"}}"##)
}

fn table_sheaf(scope: &str, columns: &[String]) -> String {
    format!(r##"{{"kind":"table","scope":"{scope}","columns":[{}]}}"##, columns.join(","))
}

fn model_of<'a>(sections: &'a crate::GlobalSections, binding: &str) -> &'a TableModel {
    match sections.get(binding) {
        Some(Section::Table(model)) => model,
        other => panic!("{binding} is not a table: {other:?}"),
    }
}

#[test]
fn table_columns_take_their_kind_from_the_spec_or_else_from_their_cell() {
    use ColumnKind::{Enum, Number, Text};
    let objects = [
        ("id", None, text_cell("id"), Text),
        ("title", None, text_cell("title"), Text),
        ("kind", None, text_cell("kind"), Enum),
        ("degree", None, text_cell("degree"), Number),
        ("in", None, text_cell("in_degree"), Number),
        ("out", None, text_cell("out_degree"), Number),
        ("repulsion", None, text_cell("repulsion"), Number),
        ("mass", None, text_cell("mass"), Number),
        ("weighted", None, text_cell("weighted:energy"), Number),
        ("load", None, meter_cell("field:energy", 100.0), Number),
        ("status", None, r##"{"cell":"status","from":"kind"}"##.to_owned(), Enum),
        ("field", None, text_cell("field:energy"), Text),
        ("tag", None, text_cell("tag:0"), Text),
        ("energy", Some("number"), text_cell("field:energy"), Number),
        ("tag_enum", Some("enum"), text_cell("tag:0"), Enum),
        ("degree_text", Some("text"), text_cell("degree"), Text),
        ("kind_number", Some("number"), text_cell("kind"), Number),
    ];
    let edges = [
        ("source", None, text_cell("source"), Text),
        ("target", None, text_cell("target"), Text),
        ("edge", None, text_cell("edge_kind"), Enum),
        ("weight", None, text_cell("weight"), Number),
        ("rest", None, text_cell("rest_length"), Number),
    ];
    let json = |rows: &[(&str, Option<&str>, String, ColumnKind)]| -> Vec<String> {
        rows.iter().map(|(key, kind, cell, _)| column_json(key, *kind, cell)).collect()
    };
    let sheaves = format!(
        r##"{{"objs":{},"edges":{}}}"##,
        table_sheaf("objects", &json(&objects)),
        table_sheaf("morphisms", &json(&edges)),
    );
    let sections = agentic_topos(&sheaves).global_sections(&demo_site(), &physics_a()).unwrap();

    for (binding, expected) in [("objs", &objects[..].iter().map(|c| (c.0, c.3)).collect::<Vec<_>>()), ("edges", &edges[..].iter().map(|c| (c.0, c.3)).collect())] {
        let kinds: Vec<(&str, ColumnKind)> =
            model_of(&sections, binding).columns.iter().map(|column| (column.key.as_str(), column.kind)).collect();
        assert_eq!(&kinds, expected, "{binding}");
    }
}

#[test]
fn a_column_without_a_kind_encodes_without_one_and_a_named_kind_round_trips() {
    let bare: crate::ColumnSpec = serde_json::from_str(&column_json("k", None, &text_cell("degree"))).unwrap();
    assert_eq!(bare.kind, None);
    assert!(!serde_json::to_string(&bare).unwrap().contains("kind\":"));
    let named: crate::ColumnSpec = serde_json::from_str(&column_json("k", Some("number"), &text_cell("id"))).unwrap();
    assert_eq!(named.kind, Some(ColumnKind::Number));
    assert_eq!(serde_json::from_str::<crate::ColumnSpec>(&serde_json::to_string(&named).unwrap()).unwrap(), named);
}

#[test]
fn an_undeclared_alignment_follows_the_kind_and_a_declared_one_wins() {
    use crate::CellAlign::{Center, Left, Right};
    let cases = [
        // (key, kind, declared alignment, cell, alignment painted)
        ("degree", None, None, text_cell("degree"), Right),
        ("load", None, None, meter_cell("field:energy", 10.0), Right),
        ("energy", Some("number"), None, text_cell("field:energy"), Right),
        ("id", None, None, text_cell("id"), Left),
        ("kind", None, None, text_cell("kind"), Left),
        ("degree_left", None, Some("left"), text_cell("degree"), Left),
        ("degree_centre", None, Some("center"), text_cell("degree"), Center),
        ("load_left", None, Some("left"), meter_cell("field:energy", 10.0), Left),
        ("id_right", None, Some("right"), text_cell("id"), Right),
    ];
    let columns: Vec<String> =
        cases.iter().map(|(key, kind, align, cell, _)| column_json_aligned(key, *kind, *align, cell)).collect();
    let sections = agentic_topos(&format!(r##"{{"objs":{}}}"##, table_sheaf("objects", &columns)))
        .global_sections(&demo_site(), &physics_a())
        .unwrap();
    let painted: Vec<(&str, TextAlign)> =
        model_of(&sections, "objs").columns.iter().map(|column| (column.key.as_str(), column.align)).collect();
    let expected: Vec<(&str, TextAlign)> = cases
        .iter()
        .map(|(key, .., painted)| (*key, match painted { Left => TextAlign::Left, Center => TextAlign::Center, Right => TextAlign::Right }))
        .collect();
    assert_eq!(painted, expected);

    // An undeclared alignment stays undeclared when the spec is written back.
    let bare: crate::ColumnSpec = serde_json::from_str(&column_json_aligned("k", None, None, &text_cell("degree"))).unwrap();
    assert_eq!(bare.align, None);
    assert!(!serde_json::to_string(&bare).unwrap().contains("align\":"));
}

/// Row ids in the order the interactive table shows `model` sorted on `column`.
fn sorted_on(model: &TableModel, column: &str, dir: SortDir) -> Vec<String> {
    let columns = model.data_columns();
    let data = model.data();
    let defaults = TableDefaults { sort: vec![SortSpec::new(column, dir)], ..TableDefaults::default() };
    let state = TableState::new(&columns, defaults);
    View::build(&columns, &data, &state, BuildOptions::default()).ids(&data).map(str::to_owned).collect()
}

/// Sites whose objects carry a decimal `energy`, negatives and repeats included,
/// and a few morphisms so degrees differ.
fn arb_energy_site() -> impl Strategy<Value = Site> {
    (prop::collection::vec((0..E_OBJ.len(), -5000i32..5000), 1..12), prop::collection::vec((0usize..64, 0usize..64), 0..16))
        .prop_map(|(objects, edges)| {
            let count = objects.len();
            Site {
                objects: objects
                    .iter()
                    .enumerate()
                    .map(|(at, (kind, hundredths))| {
                        object(&format!("o{at}"), E_OBJ[*kind], &format!("{}", f64::from(*hundredths) / 100.0))
                    })
                    .collect(),
                morphisms: edges
                    .into_iter()
                    .map(|(from, to)| typed(&format!("o{}", from % count), &format!("o{}", to % count), "continues"))
                    .collect(),
            }
        })
}

#[test]
fn a_grammar_tables_number_columns_sort_by_value_and_its_enum_columns_offer_their_values() {
    let columns = [
        column_json("id", None, &text_cell("id")),
        column_json("energy", Some("number"), &text_cell("field:energy")),
        column_json("load", None, &meter_cell("field:energy", 10.0)),
        column_json("degree", None, &text_cell("degree")),
        column_json("sort", None, &text_cell("kind")),
    ];
    let topos = agentic_topos(&format!(r##"{{"objs":{}}}"##, table_sheaf("objects", &columns)));
    let physics = physics_a();
    proptest!(|(site in arb_energy_site())| {
        let sections = topos.global_sections(&site, &physics).unwrap();
        let model = model_of(&sections, "objs");
        let energy = |object: &Object| object.number("energy").unwrap();
        let number = |object: &Object, column: &str| match column {
            "energy" => energy(object),
            "load" => energy(object) / 10.0,
            _ => site.degree(&object.id) as f64,
        };
        for column in ["energy", "load", "degree"] {
            for dir in [SortDir::Asc, SortDir::Desc] {
                let shown: Vec<f64> = sorted_on(model, column, dir)
                    .iter()
                    .map(|id| number(site.objects.iter().find(|object| crate::edit::object_row_id(&object.id) == *id).unwrap(), column))
                    .collect();
                let mut expected: Vec<f64> = site.objects.iter().map(|object| number(object, column)).collect();
                expected.sort_by(f64::total_cmp);
                if dir == SortDir::Desc {
                    expected.reverse();
                }
                prop_assert_eq!(shown, expected, "{} {:?}", column, dir);
            }
        }

        // The enum column offers each regime label present once, in natural order.
        let mut labels: Vec<String> = site
            .objects
            .iter()
            .map(|object| topos.stalks.objects[&topos.object_sorting[&object.kind]].label.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        labels.sort_by(|a, b| natural_cmp(a, b));
        let columns = model.data_columns();
        let view = View::build(&columns, &model.data(), &TableState::new(&columns, TableDefaults::default()), BuildOptions::default());
        prop_assert_eq!(view.options.get("sort").cloned().unwrap_or_default(), labels);
    });
}
// --- Incremental append ----------------------------------------------------

use proptest::prelude::*;

use crate::Limits;

/// One generated line of the trace-lines format; node ids are `n0, n1, ...` in order of appearance.
#[derive(Clone, Debug)]
enum Op {
    Node { title: String, kind: usize, tags: Vec<String>, props: BTreeMap<String, String> },
    /// An edge between two nodes written earlier.
    Edge { from: usize, to: usize, kind: String },
    /// A node reusing an earlier id.
    Reuse { of: usize },
    /// An edge into a node written up to two lines later, if any is.
    Ahead { from: usize, by: usize, kind: String },
    /// A line the grammar rejects, or a record it refuses.
    Junk(usize),
}

const KINDS: [&str; 4] = ["planner", "coder", "reviewer", "memory"];
const JUNK: [&str; 8] = ["N|broken", "E|n0", "X|y|z", "", "N|n0|T|k||k=v;k=w", "N||T|k||", "E||n0|k", "N|n1|T||"];

fn op() -> impl Strategy<Value = Op> {
    let text = "[a-zA-Z0-9 ._é-]{1,8}";
    prop_oneof![
        8 => (text, 0..KINDS.len(), prop::collection::vec("[a-z0-9]{1,5}", 0..3), prop::collection::btree_map("[a-z]{1,4}", "[a-zA-Z0-9 ._é-]{1,6}", 0..3))
            .prop_map(|(title, kind, tags, props)| Op::Node { title, kind, tags, props }),
        4 => (any::<usize>(), any::<usize>(), "[a-z]{1,8}").prop_map(|(from, to, kind)| Op::Edge { from, to, kind }),
        1 => any::<usize>().prop_map(|of| Op::Reuse { of }),
        1 => (any::<usize>(), 1..3usize, "[a-z]{1,8}").prop_map(|(from, by, kind)| Op::Ahead { from, by, kind }),
        1 => (0..JUNK.len()).prop_map(Op::Junk),
    ]
}

fn node_line(id: usize, title: &str, kind: &str, tags: &[String], props: &BTreeMap<String, String>) -> String {
    let props: Vec<String> = props.iter().map(|(key, value)| format!("{key}={value}")).collect();
    format!("N|n{id}|{title}|{kind}|{}|{}", tags.join(","), props.join(";"))
}

/// The line of each op. Ops before `valid_until` are made valid: one the grammar would reject, or
/// that clashes with an earlier line, is written as a fresh node instead.
fn lines_of(ops: &[Op], valid_until: usize) -> Vec<String> {
    let mut nodes = 0;
    let mut lines = Vec::new();
    for (at, op) in ops.iter().enumerate() {
        let strict = at < valid_until;
        let line = match op {
            Op::Edge { from, to, kind } if nodes > 0 => format!("E|n{}|n{}|{kind}", from % nodes, to % nodes),
            Op::Reuse { of } if nodes > 0 && !strict => node_line(of % nodes, "R", KINDS[0], &[], &BTreeMap::new()),
            Op::Ahead { from, by, kind } if nodes > 0 && !strict => format!("E|n{}|n{}|{kind}", from % nodes, nodes + by - 1),
            Op::Junk(which) if !strict => JUNK[*which].to_owned(),
            Op::Node { title, kind, tags, props } => {
                nodes += 1;
                node_line(nodes - 1, title, KINDS[*kind], tags, props)
            }
            _ => {
                nodes += 1;
                node_line(nodes - 1, "T", KINDS[1], &[], &BTreeMap::new())
            }
        };
        lines.push(line);
    }
    lines
}

fn text_of(lines: &[String], newline: &str, terminated: bool) -> String {
    let mut text = lines.join(newline);
    if terminated && !lines.is_empty() {
        text.push_str(newline);
    }
    text
}

/// `suffix` on a new line after `prefix`: the text `parse_append` stands for.
fn appended(prefix: &str, suffix: &str) -> String {
    if suffix.is_empty() {
        return prefix.to_owned();
    }
    let separator = if prefix.is_empty() || prefix.ends_with('\n') { "" } else { "\n" };
    format!("{prefix}{separator}{suffix}")
}

/// The records `extension` adds, behind the objects and morphisms of `site`.
fn extended(site: &Site, added: &crate::pest_engine::Mapped) -> Site {
    Site {
        objects: site.objects.iter().chain(&added.nodes).cloned().collect(),
        morphisms: site.morphisms.iter().chain(&added.edges).cloned().collect(),
    }
}

#[test]
fn appending_lines_is_parsing_the_whole_text() {
    let grammar = lines_grammar();
    proptest!(ProptestConfig::with_cases(768), |(
        ops in prop::collection::vec(op(), 0..14),
        cut in any::<prop::sample::Index>(),
        prefix_terminated in any::<bool>(),
        suffix_terminated in any::<bool>(),
        crlf in any::<bool>(),
    )| {
        let split = cut.index(ops.len() + 1);
        let lines = lines_of(&ops, split);
        let newline = if crlf { "\r\n" } else { "\n" };
        let prefix = text_of(&lines[..split], newline, prefix_terminated);
        let suffix = text_of(&lines[split..], newline, suffix_terminated);
        let before = grammar.parse(&prefix).expect("the prefix is valid by construction");
        let whole = grammar.parse(&appended(&prefix, &suffix));

        let mut site = before.clone();
        let outcome = grammar.parse_append(&mut site, &prefix, &suffix);
        match &whole {
            Ok(whole) => {
                prop_assert_eq!(outcome, Ok(()));
                prop_assert_eq!(&site, whole);
            }
            Err(error) => {
                prop_assert_eq!(outcome, Err(error.clone()));
                prop_assert_eq!(&site, &before);
            }
        }

        let apart = grammar.extension(&before, &suffix);
        match &whole {
            Ok(whole) if !suffix.is_empty() => {
                let added = apart.ok_or_else(|| TestCaseError::fail("a valid suffix was not parsed apart"))?;
                prop_assert_eq!(&extended(&before, &added), whole);
            }
            Err(_) => prop_assert!(apart.is_none(), "an invalid suffix was parsed apart"),
            Ok(_) => {}
        }
    });
}

#[test]
fn appending_lines_one_at_a_time_is_parsing_at_once() {
    let grammar = lines_grammar();
    proptest!(|(ops in prop::collection::vec(op(), 0..24))| {
        let mut source = String::new();
        let mut site = grammar.parse("").unwrap();
        for line in lines_of(&ops, ops.len()) {
            prop_assert!(grammar.extension(&site, &line).is_some(), "'{line}' was not parsed apart");
            prop_assert_eq!(grammar.parse_append(&mut site, &source, &line), Ok(()));
            if !source.is_empty() && !source.ends_with('\n') {
                source.push('\n');
            }
            source.push_str(&line);
            prop_assert_eq!(&site, &grammar.parse(&source).unwrap());
        }
    });
}

#[test]
fn an_appended_edge_may_join_objects_of_the_prefix_and_of_its_own_lines() {
    let grammar = lines_grammar();
    let prefix = "N|a|A|coder||\nN|b|B|coder||\n";
    let suffix = "E|a|c|shares\nN|c|C|coder||\nE|c|b|shares";
    let mut site = grammar.parse(prefix).unwrap();
    assert!(grammar.extension(&site, suffix).is_some(), "the edges are parsed apart from the prefix");
    grammar.parse_append(&mut site, prefix, suffix).unwrap();
    assert_eq!(site, grammar.parse(&format!("{prefix}{suffix}")).unwrap());
    assert_eq!((site.objects.len(), site.morphisms.len()), (3, 2));
}

#[test]
fn a_suffix_the_whole_text_would_refuse_is_refused_and_leaves_the_site() {
    let grammar = lines_grammar();
    let prefix = "N|a|A|coder||\n";
    let refused = [
        ("N|a|Again|coder||", GrammarError::DuplicateNodeId("a".to_owned())),
        ("E|a|ghost|shares", GrammarError::DanglingEdge { source: "a".to_owned(), target: "ghost".to_owned(), missing: "target" }),
        ("N|b|B|coder||\nE|b|nowhere|shares", GrammarError::DanglingEdge { source: "b".to_owned(), target: "nowhere".to_owned(), missing: "target" }),
    ];
    for (suffix, error) in refused {
        let before = grammar.parse(prefix).unwrap();
        let mut site = before.clone();
        assert_eq!(grammar.parse(&format!("{prefix}{suffix}")), Err(error.clone()), "{suffix}");
        assert_eq!(grammar.parse_append(&mut site, prefix, suffix), Err(error), "{suffix}");
        assert_eq!(site, before, "{suffix}");
        assert!(grammar.extension(&before, suffix).is_none(), "{suffix}");
    }
}

#[test]
fn appending_counts_the_records_already_held_against_the_limits() {
    let mut package = GrammarPackage::from_json_str(GRAMMAR_LINES).unwrap();
    package.limits.nodes = 3;
    package.limits.edges = 1;
    let grammar = package.compile().unwrap();
    let prefix = "N|a|A|k||\nN|b|B|k||\nE|a|b|k\n";
    let site = grammar.parse(prefix).unwrap();
    assert!(grammar.extension(&site, "N|c|C|k||").is_some(), "a third node is at the limit");
    for (suffix, kind, actual, max) in [("N|c|C|k||\nN|d|D|k||", "nodes", 4, 3), ("E|b|a|k", "edges", 2, 1)] {
        let error = GrammarError::RecordLimit { kind, actual, max };
        let mut grown = site.clone();
        assert_eq!(grammar.parse(&format!("{prefix}{suffix}")), Err(error.clone()), "{suffix}");
        assert_eq!(grammar.parse_append(&mut grown, prefix, suffix), Err(error), "{suffix}");
        assert_eq!(grown, site, "{suffix}");
    }
}

#[test]
fn appending_counts_the_whole_input_against_the_input_limit() {
    // 19 bytes whole: the separator counts when the prefix has no final newline.
    for prefix in ["N|a|A|k||\n", "N|a|A|k||"] {
        let suffix = "N|b|B|k||";
        let whole = appended(prefix, suffix);
        let mut package = GrammarPackage::from_json_str(GRAMMAR_LINES).unwrap();
        package.limits.input_bytes = whole.len();
        let grammar = package.compile().unwrap();
        let site = grammar.parse(prefix).unwrap();
        let mut grown = site.clone();
        assert_eq!(grammar.parse_append(&mut grown, prefix, suffix), Ok(()), "{prefix:?} fits exactly");
        assert_eq!(grown, grammar.parse(&whole).unwrap());

        package.limits.input_bytes = whole.len() - 1;
        let grammar = package.compile().unwrap();
        let mut grown = site.clone();
        let error = GrammarError::InputTooLarge { actual: whole.len(), max: whole.len() - 1 };
        assert_eq!(grammar.parse(&whole), Err(error.clone()));
        assert_eq!(grammar.parse_append(&mut grown, prefix, suffix), Err(error), "{prefix:?} is one byte over");
        assert_eq!(grown, site);
    }
}

#[test]
fn a_grammar_that_does_not_declare_line_records_is_parsed_whole() {
    let mut package = GrammarPackage::from_json_str(GRAMMAR_LINES).unwrap();
    let ParserSpec::Pest(spec) = &mut package.parser else { unreachable!("trace-lines is a pest package") };
    spec.line_records = false;
    let undeclared = package.compile().unwrap();
    assert!(lines_grammar().appends_incrementally());
    assert!(!undeclared.appends_incrementally());
    let (prefix, suffix) = ("N|a|A|coder||\n", "N|b|B|coder||");
    let mut site = undeclared.parse(prefix).unwrap();
    assert!(undeclared.extension(&site, suffix).is_none());
    undeclared.parse_append(&mut site, prefix, suffix).unwrap();
    assert_eq!(site, undeclared.parse(&format!("{prefix}{suffix}")).unwrap());

    let json = json_grammar();
    let mut document = json.parse(DATA_JSON).unwrap();
    assert!(!json.appends_incrementally());
    assert!(json.extension(&document, "{}").is_none());
    let error = json.parse(&format!("{DATA_JSON}!")).unwrap_err();
    assert_eq!(json.parse_append(&mut document, DATA_JSON, "!"), Err(error));
}

#[test]
fn a_declaration_the_grammar_contradicts_fails_to_compile() {
    let lines_with = |file: &str, emitter: bool| {
        let mut package = GrammarPackage::from_json_str(GRAMMAR_LINES).unwrap();
        let ParserSpec::Pest(spec) = &mut package.parser else { unreachable!("trace-lines is a pest package") };
        spec.grammar = spec.grammar.replacen("file = { SOI ~ (record ~ NEWLINE)* ~ record? ~ EOI }", file, 1);
        if emitter {
            package.emitter = Some(serde_json::from_str(LINES_EMITTER).unwrap());
        }
        package.compile()
    };
    assert!(lines_with("file = { SOI ~ (record ~ NEWLINE)* ~ record? ~ EOI }", true).is_ok());
    let header = "file = { SOI ~ \"#trace\" ~ NEWLINE ~ (record ~ NEWLINE)* ~ record? ~ EOI }";
    assert!(matches!(lines_with(header, false), Err(GrammarError::NotLineRecords(_))), "a header makes empty input invalid");
    let one_record = "file = { SOI ~ record? ~ NEWLINE? ~ EOI }";
    assert!(matches!(lines_with(one_record, true), Err(GrammarError::NotLineRecords(_))), "two emitted lines do not parse");
}

/// Timing probe, not an assertion: the parse cost per appended line as the input grows, appending
/// with `parse_append` and, for `FULL_LINES` lines, reparsing the whole text as a host once did.
/// `cargo test --release -p panel-kit-grammar -- --ignored --nocapture parse_cost`; `APPEND_LINES`
/// defaults to 10 000, `FULL_LINES` to 1 000.
#[test]
#[ignore = "timing probe"]
fn parse_cost_by_input_size() {
    let count = |name: &str, default: usize| std::env::var(name).ok().and_then(|text| text.parse().ok()).unwrap_or(default);
    let (appended_lines, reparsed_lines) = (count("APPEND_LINES", 10_000), count("FULL_LINES", 1_000));
    let mut package = GrammarPackage::from_json_str(GRAMMAR_LINES).unwrap();
    package.limits = Limits { grammar_bytes: 8192, input_bytes: 64 << 20, nodes: 100_000, edges: 100_000 };
    let grammar = package.compile().unwrap();
    let line = |n: usize| format!("N|appended_{n}|Appended {n}|memory|demo|latency_ms={};energy={};t={};status=ok", 3 + n, 5 + n, 64 + n);

    let started = std::time::Instant::now();
    let (mut source, mut site) = (DATA_LINES.to_owned(), site_from_lines());
    for n in 0..appended_lines {
        let line = line(n);
        grammar.parse_append(&mut site, &source, &line).unwrap();
        source.push_str(&line);
        source.push('\n');
    }
    println!("parse_append: {appended_lines} lines in {:.3?}, {:.3?} per line", started.elapsed(), started.elapsed() / appended_lines as u32);
    assert_eq!(site, grammar.parse(&source).unwrap());

    let started = std::time::Instant::now();
    let mut source = DATA_LINES.to_owned();
    for n in 0..reparsed_lines {
        source.push_str(&line(n));
        source.push('\n');
        grammar.parse(&source).unwrap();
    }
    println!("parse of the whole text: {reparsed_lines} lines in {:.3?}, {:.3?} per line", started.elapsed(), started.elapsed() / reparsed_lines as u32);
}
