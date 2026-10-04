//! Behavior-level tests: both engines produce the same site; limits and
//! validation reject bad input; a records-only JSON grammar yields no
//! morphisms; the geometric morphism satisfies the triangle identities and
//! both Galois connections; validation rejects non-total/outside/non-commuting
//! sort maps; the physics section is regime-invariant; and assembly classifies
//! each topological stage.

use std::collections::{BTreeMap, BTreeSet};

use panel_kit_core::widgets::table::TableCell;

use crate::error::SheafError;
use crate::package::CompiledGrammar;
use crate::{
    assembly, BaseTopos, GeometricMorphism, GrammarError, GrammarPackage, Morphism, MorphismError,
    Object, ParserSpec, Physics, Section, Site, Stage, Topos, ToposError,
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
