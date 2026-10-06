//! Host-contract test for the trio façade, over the real Nix JSON the
//! `topos-trio-test` check exports through `trioEnv`: **one site** carrying the
//! agent sessions and the OKF objects they produce, verify, and read, read in
//! **three always-available regimes**, related by `f: agentic → membrane`,
//! `ψ: okf-catalog → membrane`, `χ: okf-catalog → agentic`, and
//! `ω: agentic → okf-catalog` with `f ∘ χ = ψ` and `ψ ∘ ω = f`.
//!
//! Every workspace binding of every regime resolves with no panel degraded —
//! the morphisms transport the fields, so a foreign kind is read through its
//! attribute map — while the graph's mass, repulsion, weight, and rest length
//! are identical across the regimes and only the sorts and colours move; the
//! sessions still assemble into every stage; a selection survives transport
//! between regimes; and an ω that maps `reviewer` to `skill`, which breaks
//! `ψ ∘ ω = f`, fails `load` with the square named.
//!
//! Paths come from the environment at compile time through `option_env!`, so
//! this target compiles under every check and skips where the env is absent.

#[allow(dead_code)]
#[path = "../examples/support/trio_host.rs"]
mod trio_host;

#[allow(dead_code)]
#[path = "../examples/support/graph_layout.rs"]
mod graph_layout;

use std::collections::{BTreeMap, BTreeSet};

use panel_kit_core::badge::Rgb;
use panel_kit_core::spec::WorkspaceSpec;
use panel_kit_core::theme::ThemeTokens;
use panel_kit_core::widgets::graph::LayoutAlgorithm;
use panel_kit_grammar::{Section, Stage};

use trio_host::{
    MorphismId, RegimeId, Route, TrioHost, TrioSpecs, GRAMMAR_JSON, GRAMMAR_LINES, MORPHISM,
    PHYSICS, SELECTED, SELECTED_OKF,
};

/// Read a document named by the check's environment, or `None` when this build
/// carries no such variable.
macro_rules! exported {
    ($name:literal) => {
        match option_env!($name) {
            Some(path) => Some(
                std::fs::read_to_string(path)
                    .unwrap_or_else(|error| panic!("{}={path}: {error}", $name)),
            ),
            None => None,
        }
    };
}

/// The Nix-authored documents `trioEnv` names; the fixture owns them, so the
/// borrowed [`TrioSpecs`] never outlives it.
struct Fixture {
    agentic: String,
    membrane: String,
    okf_catalog: String,
    base: String,
    f: String,
    psi: String,
    chi: String,
    omega: String,
    agentic_physics: String,
    membrane_physics: String,
    catalog_physics: String,
    trace_lines_grammar: String,
    trace_lines_data: String,
    trace_json_grammar: String,
    trace_json_data: String,
}

impl Fixture {
    /// Read every document, or report which are missing and skip.
    fn load() -> Option<Self> {
        let mut missing: Vec<String> = Vec::new();
        let mut read = |var: &str, text: Option<String>| -> String {
            match text {
                Some(text) => text,
                None => {
                    missing.push(var.to_owned());
                    String::new()
                }
            }
        };
        let fixture = Fixture {
            agentic: read("PANEL_KIT_TRIO_AGENTIC", exported!("PANEL_KIT_TRIO_AGENTIC")),
            membrane: read("PANEL_KIT_TRIO_MEMBRANE", exported!("PANEL_KIT_TRIO_MEMBRANE")),
            okf_catalog: read("PANEL_KIT_TRIO_OKF_CATALOG", exported!("PANEL_KIT_TRIO_OKF_CATALOG")),
            base: read("PANEL_KIT_TRIO_BASE", exported!("PANEL_KIT_TRIO_BASE")),
            f: read("PANEL_KIT_TRIO_F", exported!("PANEL_KIT_TRIO_F")),
            psi: read("PANEL_KIT_TRIO_PSI", exported!("PANEL_KIT_TRIO_PSI")),
            chi: read("PANEL_KIT_TRIO_CHI", exported!("PANEL_KIT_TRIO_CHI")),
            omega: read("PANEL_KIT_TRIO_OMEGA", exported!("PANEL_KIT_TRIO_OMEGA")),
            agentic_physics: read(
                "PANEL_KIT_TRIO_AGENTIC_PHYSICS",
                exported!("PANEL_KIT_TRIO_AGENTIC_PHYSICS"),
            ),
            membrane_physics: read(
                "PANEL_KIT_TRIO_MEMBRANE_PHYSICS",
                exported!("PANEL_KIT_TRIO_MEMBRANE_PHYSICS"),
            ),
            catalog_physics: read(
                "PANEL_KIT_TRIO_OKF_CATALOG_PHYSICS",
                exported!("PANEL_KIT_TRIO_OKF_CATALOG_PHYSICS"),
            ),
            trace_lines_grammar: read(
                "PANEL_KIT_TRIO_GRAMMAR_LINES",
                exported!("PANEL_KIT_TRIO_GRAMMAR_LINES"),
            ),
            trace_lines_data: read(
                "PANEL_KIT_TRIO_DATA_LINES",
                exported!("PANEL_KIT_TRIO_DATA_LINES"),
            ),
            trace_json_grammar: read(
                "PANEL_KIT_TRIO_GRAMMAR_JSON",
                exported!("PANEL_KIT_TRIO_GRAMMAR_JSON"),
            ),
            trace_json_data: read(
                "PANEL_KIT_TRIO_DATA_JSON",
                exported!("PANEL_KIT_TRIO_DATA_JSON"),
            ),
        };
        if !missing.is_empty() {
            println!(
                "skipped: this build exports none of {}; build .#checks.aarch64-darwin.topos-trio-test",
                missing.join(", ")
            );
            return None;
        }
        Some(fixture)
    }

    fn specs(&self) -> TrioSpecs<'_> {
        TrioSpecs {
            agentic: &self.agentic,
            membrane: &self.membrane,
            okf_catalog: &self.okf_catalog,
            base: &self.base,
            f: &self.f,
            psi: &self.psi,
            chi: &self.chi,
            omega: &self.omega,
            agentic_physics: &self.agentic_physics,
            membrane_physics: &self.membrane_physics,
            catalog_physics: &self.catalog_physics,
            trace_lines_grammar: &self.trace_lines_grammar,
            trace_lines_data: &self.trace_lines_data,
            trace_json_grammar: &self.trace_json_grammar,
            trace_json_data: &self.trace_json_data,
        }
    }

    fn host(&self) -> TrioHost {
        TrioHost::load(&self.specs()).expect("the trio host loads the authored specs")
    }

    /// A host reading the site in `regime`.
    fn host_at(&self, regime: RegimeId) -> TrioHost {
        let mut host = self.host();
        host.set_regime(regime);
        assert_eq!(host.regime(), regime);
        host
    }
}

/// Binding id and declared content kind of every panel in a workspace, the
/// external graph panel included: its body is the host's, so the host answers
/// its binding with a placeholder.
fn bindings(spec: &WorkspaceSpec) -> Vec<(String, &'static str)> {
    spec.panels
        .iter()
        .filter_map(|panel| {
            panel.content.binding_id().map(|id| (id.to_owned(), panel.content.kind()))
        })
        .collect()
}

fn text(host: &TrioHost, binding: &str) -> String {
    match host.resolve(binding) {
        Some(Section::Text(model)) => model.text.clone(),
        Some(other) => panic!("{binding} is not text: {}", other.kind()),
        None => panic!("{binding} is unresolved"),
    }
}

/// Rows of a table binding.
fn table_rows(host: &TrioHost, binding: &str) -> usize {
    match host.resolve(binding) {
        Some(Section::Table(model)) => model.rows.len(),
        Some(other) => panic!("{binding} is a {} section", other.kind()),
        None => panic!("{binding} is unresolved"),
    }
}

#[test]
fn every_workspace_binding_resolves_under_every_regime() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host();
    for regime in RegimeId::ALL {
        host.set_regime(regime);
        let spec = host.workspace_spec();
        assert_eq!(spec.id, format!("{}-ws", regime.topos_id()));
        let bound = bindings(spec);
        assert!(!bound.is_empty(), "{regime} has no binding-backed panels");
        assert_eq!(host.unreadable(), Vec::<&str>::new(), "{regime} reads the whole site");
        for (binding, kind) in bound {
            let section = host
                .resolve(&binding)
                .unwrap_or_else(|| panic!("{binding} unresolved in {regime}"));
            if kind == "external" || kind == "editor" {
                assert_eq!(section.kind(), "text", "{binding} answers with a host-owned document");
            } else {
                assert_eq!(section.kind(), kind, "{binding} kind mismatch in {regime}");
            }
            if section.kind() == "text" {
                assert!(!text(&host, &binding).is_empty(), "{binding} is empty in {regime}");
            }
        }
    }
}

#[test]
fn the_foreign_panels_read_their_fields_through_the_attribute_maps() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host();

    // An OKF object read in agentic: the table's latency meter and status cell
    // read `stale_after` and `status` off the knowledge object, not off a
    // session's `latency_ms`.
    host.set_regime(RegimeId::Agentic);
    let binding = binding_of(&host, RegimeId::Agentic, "status");
    let Section::Table(model) = host.resolve(&binding).expect("a table") else {
        panic!("{binding} is not a table");
    };
    let row = model
        .rows
        .iter()
        .find(|row| matches!(&row.cells[0], panel_kit_core::widgets::table::TableCell::Text(text) if text == "Gross margin"))
        .expect("the gross-margin metric is a row");
    let latency = match &row.cells[2] {
        panel_kit_core::widgets::table::TableCell::Meter { text, .. } => text.clone(),
        other => panic!("the latency cell is a meter: {other:?}"),
    };
    assert_eq!(latency, "7", "the agentic latency reads the object's stale_after: {row:?}");
    let status = match &row.cells[3] {
        panel_kit_core::widgets::table::TableCell::Status { label, .. } => label.clone(),
        other => panic!("the status cell is a status: {other:?}"),
    };
    assert_eq!(status, "stable", "status crosses unchanged");

    // A session read in the catalog: the Concepts table's trust and stale_after
    // columns read the session's energy and latency.
    host.set_regime(RegimeId::OkfCatalog);
    let binding = binding_of(&host, RegimeId::OkfCatalog, "concepts");
    let Section::Table(model) = host.resolve(&binding).expect("a table") else {
        panic!("{binding} is not a table");
    };
    let row = model
        .rows
        .iter()
        .find(|row| matches!(&row.cells[0], panel_kit_core::widgets::table::TableCell::Text(text) if text == "Session c0"))
        .expect("a session is a row of the catalog's concepts");
    let cells: Vec<String> = row
        .cells
        .iter()
        .map(|cell| format!("{cell:?}"))
        .collect();
    assert!(cells[3].contains("240"), "trust reads the session's energy: {cells:?}");
    assert!(cells[5].contains("30"), "stale_after reads the session's latency_ms: {cells:?}");
}

#[test]
fn one_site_carries_the_sessions_the_knowledge_objects_and_the_cross_morphisms() {
    let Some(fixture) = Fixture::load() else { return };
    let host = fixture.host();
    let site = host.site();

    let kinds: BTreeSet<&str> = site.objects.iter().map(|object| object.kind.as_str()).collect();
    for kind in [
        "planner",
        "coder",
        "reviewer",
        "memory",
        "bundle",
        "table",
        "dataset",
        "metric",
        "computation",
        "policy",
        "skill",
        "reference",
        "concept",
        "log",
    ] {
        assert!(kinds.contains(kind), "the merged site is missing the kind {kind}");
    }
    assert_eq!(kinds.len(), 14, "fourteen kinds on one site");

    let morphism_kinds: BTreeSet<&str> =
        site.morphisms.iter().filter_map(|morphism| morphism.kind.as_deref()).collect();
    for kind in [
        "continues",
        "shares",
        "consolidates",
        "derived_from",
        "computes",
        "executed_by",
        "attested_by",
        "references",
        "contains",
        "produces",
        "verifies",
    ] {
        assert!(morphism_kinds.contains(kind), "the merged site is missing {kind}");
    }
    assert_eq!(morphism_kinds.len(), 11, "eleven morphism kinds on one site");

    let cross = site
        .morphisms
        .iter()
        .filter(|morphism| matches!(morphism.kind.as_deref(), Some("produces") | Some("verifies")))
        .count();
    assert!((10..=20).contains(&cross), "the trace data carries {cross} cross morphisms");

    // Every cross morphism joins a session to a knowledge object.
    let session: BTreeSet<&str> = site
        .objects
        .iter()
        .filter(|object| matches!(object.kind.as_str(), "planner" | "coder" | "reviewer" | "memory"))
        .map(|object| object.id.as_str())
        .collect();
    for morphism in site
        .morphisms
        .iter()
        .filter(|morphism| matches!(morphism.kind.as_deref(), Some("produces") | Some("verifies")))
    {
        assert!(
            session.contains(morphism.domain.as_str()) ^ session.contains(morphism.codomain.as_str()),
            "a cross morphism joins a session to a knowledge object: {morphism:?}"
        );
    }
}

/// The binding a regime's workspace mounts for one of its panels, named by the
/// cell the panel occupies.
fn binding_of(host: &TrioHost, regime: RegimeId, cell: &str) -> String {
    host.workspace_spec()
        .panels
        .iter()
        .find(|panel| panel.id == cell)
        .and_then(|panel| panel.content.binding_id())
        .unwrap_or_else(|| panic!("{} mounts no {cell} panel", regime.topos_id()))
        .to_owned()
}

#[test]
fn every_regime_reaches_every_kind_along_a_morphism() {
    let Some(fixture) = Fixture::load() else { return };
    let host = fixture.host();
    let site = host.site();
    let kinds: BTreeSet<String> =
        site.objects.iter().map(|object| object.kind.clone()).collect();

    let natives = host.native_regimes();
    assert_eq!(
        natives,
        vec![(RegimeId::Agentic, 4, 43), (RegimeId::OkfCatalog, 10, 27)],
        "sessions are native in agentic, knowledge objects in the catalog"
    );

    for kind in &kinds {
        let native = host.native_regime(kind);
        assert!(host.route_to(kind, native).is_native(), "{kind} is native in {native}");
        for regime in RegimeId::ALL {
            let route = host.route_to(kind, regime);
            assert!(!route.path.is_empty() || regime == native, "{kind} must be sorted in {regime}");
            assert!(!route.text().is_empty(), "{kind} in {regime}: {}", route.text());
        }
    }

    assert_eq!(
        host.route_to("planner", RegimeId::Membrane),
        Route {
            native: RegimeId::Agentic,
            regime: RegimeId::Membrane,
            path: vec![MorphismId::F],
            composite: Some(vec![MorphismId::Omega, MorphismId::Psi]),
        },
        "a session reaches the membrane along f, and the square makes ω then ψ the same map"
    );
    assert_eq!(
        host.route_to("planner", RegimeId::OkfCatalog).path,
        vec![MorphismId::Omega],
        "a session reaches the catalog along omega"
    );
    assert_eq!(
        host.route_to("metric", RegimeId::Agentic).path,
        vec![MorphismId::Chi],
        "a knowledge object reaches the agentic regime along chi"
    );
    assert_eq!(
        host.route_to("metric", RegimeId::Membrane),
        Route {
            native: RegimeId::OkfCatalog,
            regime: RegimeId::Membrane,
            path: vec![MorphismId::Psi],
            composite: Some(vec![MorphismId::Chi, MorphismId::F]),
        }
    );
}

#[test]
fn the_graph_physics_is_identical_across_the_regimes_of_one_site() {
    let Some(fixture) = Fixture::load() else { return };
    /// One regime's tuning as read off the graph: `(id, size, mass, repulsion)`
    /// per node and `(source, target, weight, rest length)` per edge.
    type Tuning = (Vec<(String, f32, f64, f64)>, Vec<(u32, u32, f64, f64)>);

    let mut host = fixture.host();
    let mut read: Option<Tuning> = None;
    for (at, regime) in RegimeId::ALL.into_iter().enumerate() {
        host.set_regime(regime);
        let graph = host.graph();
        assert_eq!(graph.nodes.len(), host.site().objects.len(), "{regime} node count");
        assert_eq!(graph.edges.len(), host.site().morphisms.len(), "{regime} edge count");
        let nodes: Vec<(String, f32, f64, f64)> = graph
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node.size, node.mass, node.repulsion))
            .collect();
        let edges: Vec<(u32, u32, f64, f64)> = graph
            .edges
            .iter()
            .map(|edge| (edge.source, edge.target, edge.weight, edge.rest_length))
            .collect();
        assert!(
            nodes.iter().any(|node| node.3 > 0.0) && edges.iter().any(|edge| edge.2 > 0.0),
            "{regime} reads real pulled-back tuning"
        );
        match &read {
            None => read = Some((nodes, edges)),
            Some((base_nodes, base_edges)) => {
                assert_eq!(base_nodes, &nodes, "{regime} node tuning differs from regime {at}");
                assert_eq!(base_edges, &edges, "{regime} edge tuning differs from regime {at}");
            }
        }
    }
    assert!(read.is_some(), "the one site is read by all three regimes");
}

#[test]
fn the_graph_sorts_and_colours_move_while_the_physics_does_not() {
    let Some(fixture) = Fixture::load() else { return };
    let palette = ThemeTokens::dark().colors;
    let fallback = (palette.fg.r, palette.fg.g, palette.fg.b);
    let mut host = fixture.host();

    let mut per_regime: Vec<(RegimeId, Vec<String>, Vec<Rgb>)> = Vec::new();
    for regime in RegimeId::ALL {
        host.set_regime(regime);
        let graph = host.graph();
        assert!(
            graph.nodes.iter().all(|node| (node.color.0, node.color.1, node.color.2) != fallback),
            "{regime} paints every sort from the palette, not the fallback"
        );
        per_regime.push((
            regime,
            graph.nodes.iter().map(|node| node.sort.clone()).collect(),
            graph.nodes.iter().map(|node| node.color).collect(),
        ));
    }
    assert_eq!(per_regime.len(), 3, "every regime reads the one site");
    for (regime, sorts, colors) in &per_regime {
        assert!(
            sorts.iter().any(|sort| *sort != per_regime[0].1[0]),
            "{regime} sorts objects by its own sorts"
        );
        assert!(colors.iter().any(|color| *color != per_regime[0].2[0]), "{regime} recolours");
    }
    for left in 0..per_regime.len() {
        for right in 0..per_regime.len() {
            if left == right {
                continue;
            }
            assert_ne!(
                per_regime[left].1, per_regime[right].1,
                "{} and {} must not share a sort reading",
                per_regime[left].0.label(),
                per_regime[right].0.label()
            );
            assert_ne!(
                per_regime[left].2, per_regime[right].2,
                "{} and {} must not share a colouring",
                per_regime[left].0.label(),
                per_regime[right].0.label()
            );
        }
    }
}

#[test]
fn the_sessions_still_assemble_into_every_stage() {
    let Some(fixture) = Fixture::load() else { return };
    for regime in [RegimeId::Agentic, RegimeId::Membrane] {
        let host = fixture.host_at(regime);
        let components = host.assembly();
        let stages: Vec<Stage> = components.iter().map(|component| component.stage).collect();
        for wanted in [Stage::Membrane, Stage::Tube, Stage::Sheet, Stage::Ring, Stage::Chain] {
            assert!(
                stages.contains(&wanted),
                "{regime}: the sessions have no {wanted:?} component, only {stages:?}"
            );
        }
        let covered: usize = components.iter().map(|component| component.v).sum();
        assert!(
            covered <= host.site().objects.len() && !components.is_empty(),
            "{regime}: {covered} objects sit in components, out of {}",
            host.site().objects.len()
        );
        let assembly = binding_of(&host, regime, "assembly");
        assert_eq!(
            table_rows(&host, &assembly),
            components.len(),
            "{regime}: the panel and the host count the same components"
        );
    }

    // The catalog assembles the knowledge objects over their own bonds.
    let host = fixture.host_at(RegimeId::OkfCatalog);
    assert!(!host.assembly().is_empty(), "the catalog assembles the knowledge objects");
}

/// Every assembly component of the agentic read, as node indices into the
/// graph, with its stage.
fn session_components(host: &TrioHost) -> Vec<(Stage, Vec<usize>)> {
    let graph = host.graph();
    // The agentic assembly bonds are the session handoff kinds, not every bond
    // that sorts into continues+shares: the cross morphisms join the sessions
    // to the knowledge objects, which the assembly deliberately leaves out.
    let bonds = ["continues", "shares"];
    let site = host.site();
    let index: BTreeMap<&str, u32> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(at, node)| (node.id.as_str(), at as u32))
        .collect();
    let adjacency: Vec<(u32, u32)> = site
        .morphisms
        .iter()
        .filter(|morphism| {
            morphism.kind.as_deref().is_some_and(|kind| bonds.contains(&kind))
        })
        .filter_map(|morphism| {
            Some((
                *index.get(morphism.domain.as_str())?,
                *index.get(morphism.codomain.as_str())?,
            ))
        })
        .collect();
    host.assembly()
        .iter()
        .map(|component| {
            let start = graph
                .nodes
                .iter()
                .position(|node| node.id == component.representative)
                .expect("the representative is a graph node");
            let mut members = vec![start];
            let mut at = 0;
            while at < members.len() {
                let node = members[at] as u32;
                for (source, target) in &adjacency {
                    let next = if *source == node {
                        Some(*target)
                    } else if *target == node {
                        Some(*source)
                    } else {
                        None
                    };
                    if let Some(next) = next.filter(|next| !members.contains(&(*next as usize))) {
                        members.push(next as usize);
                    }
                }
                at += 1;
            }
            (component.stage, members)
        })
        .collect()
}

/// We form membranes: under the force layout the graph panel asks for, the
/// sessions' closed surface lays out as a hollow vesicle in every regime that
/// reads it — the harder-repelling sorts may bulge it, never collapse or tear it
/// — and no other component's nodes sit inside.
#[test]
fn the_session_membrane_forms_a_shell() {
    let Some(fixture) = Fixture::load() else { return };
    let components = session_components(&fixture.host_at(RegimeId::Agentic));
    let (_, shell) = components
        .iter()
        .find(|(stage, _)| *stage == Stage::Membrane)
        .expect("the sessions assemble a membrane");
    assert!(shell.len() >= 12, "the membrane is a surface, not a pair: {} nodes", shell.len());

    for regime in [RegimeId::Agentic, RegimeId::Membrane] {
        let host = fixture.host_at(regime);
        let layout = host.graph_layout();
        let positions = graph_layout::solve(&host.graph(), &layout, None);
        let point = |index: usize| positions[index].map(f64::from);
        let centre = (0..3)
            .map(|axis| shell.iter().map(|&index| point(index)[axis]).sum::<f64>() / shell.len() as f64)
            .collect::<Vec<_>>();
        let from_centre =
            |index: usize| (0..3).map(|axis| point(index)[axis] - centre[axis]).fold(0.0_f64, f64::hypot);
        let radii: Vec<f64> = shell.iter().map(|&index| from_centre(index)).collect();
        let mean = radii.iter().sum::<f64>() / radii.len() as f64;
        println!("{regime}: {:?} shell radii {radii:.1?}", layout.algorithm);
        for (&index, radius) in shell.iter().zip(&radii) {
            assert!(
                (0.5 * mean..=1.5 * mean).contains(&radius),
                "{regime}: {} sits at radius {radius:.1}, the shell's mean is {mean:.1}: {radii:.1?}",
                host.graph().nodes[index].id
            );
        }
        for index in (0..positions.len()).filter(|index| !shell.contains(index)) {
            assert!(
                from_centre(index) > 0.75 * mean,
                "{regime}: {} sits inside the membrane, {:.1} from its centre, radius {mean:.1}",
                host.graph().nodes[index].id,
                from_centre(index)
            );
        }
    }
}

#[test]
fn a_selection_survives_transport_between_the_regimes() {
    let Some(fixture) = Fixture::load() else { return };
    let metric = "metric";
    let metric_id = "acme_retail/gross-margin";
    let session = "planner";

    // A knowledge object selected in its native regime and carried to the other
    // two: the subobject moves with it and reads in the new regime's sorts.
    let mut host = fixture.host_at(RegimeId::OkfCatalog);
    host.select(Some(metric_id));
    assert_eq!(host.selected(), Some(metric_id));
    let before = host.active_sorts().clone();
    assert_eq!(before.len(), 1, "the catalog sort of the selected object");

    host.set_regime(RegimeId::Membrane);
    assert_eq!(host.route(metric).path, vec![MorphismId::Psi]);
    assert_eq!(host.selected(), Some(metric_id), "one site, so the node survives");
    let after = host.active_sorts().clone();
    assert!(!after.is_empty(), "the subobject travels with the selection");
    assert_ne!(before, after, "the sorts are read in the new regime");

    host.set_regime(RegimeId::Agentic);
    assert_eq!(host.selected(), Some(metric_id));
    assert!(!host.active_sorts().is_empty());

    // A session selected in its native regime reaches the catalog along ω.
    let mut host = fixture.host_at(RegimeId::Agentic);
    host.select(Some("c3"));
    assert_eq!(host.active_sorts().len(), 1);
    host.set_regime(RegimeId::OkfCatalog);
    assert_eq!(host.route(session).path, vec![MorphismId::Omega]);
    assert_eq!(host.selected(), Some("c3"));
    assert_eq!(
        host.active_sorts(),
        &BTreeSet::from(["policy".to_owned()]),
        "a reviewer is the catalog's policy"
    );
}

#[test]
fn cycling_the_selection_walks_the_site_and_clears_on_an_unknown_node() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host_at(RegimeId::Agentic);
    assert_eq!(host.selected(), None);
    let first = host.site().objects[0].id.clone();
    host.cycle_selection(1);
    assert_eq!(host.selected(), Some(first.as_str()));
    let second = host.site().objects[1].id.clone();
    host.cycle_selection(1);
    assert_eq!(host.selected(), Some(second.as_str()));
    host.cycle_selection(-1);
    assert_eq!(host.selected(), Some(first.as_str()));
    host.select(Some("not-a-node"));
    assert_eq!(host.selected(), None, "an unknown node clears rather than lies");
}

#[test]
fn the_morphism_overlay_states_the_routes_the_laws_and_the_attribute_maps() {
    let Some(fixture) = Fixture::load() else { return };
    for regime in RegimeId::ALL {
        let host = fixture.host_at(regime);
        let text = text(&host, MORPHISM);
        assert!(text.contains("one site"), "{regime}: {text}");
        assert!(text.contains("triangle: f ∘ χ = ψ"), "{regime}: {text}");
        assert!(text.contains("square: ψ ∘ ω = f"), "{regime}: {text}");
        assert!(text.contains("ω: topos-agentic -> topos-okf-catalog"), "{regime}: {text}");
        // The attribute maps every regime's sheaves read through.
        assert!(text.contains("trust <- energy"), "{regime}: {text}");
        assert!(text.contains("latency_ms <- stale_after"), "{regime}: {text}");
        // Every site kind is sorted in the active regime.
        let kinds: BTreeSet<&str> =
            host.site().objects.iter().map(|object| object.kind.as_str()).collect();
        for kind in kinds {
            assert!(
                text.lines().any(|line| line.starts_with(&format!("  {kind} ["))),
                "{regime}: the overlay does not sort {kind}: {text}"
            );
        }
    }

    let host = fixture.host_at(RegimeId::Membrane);
    let text = text(&host, MORPHISM);
    assert!(text.contains("χ then f"), "the triangle's composite is reported: {text}");
    assert!(text.contains("ω then ψ"), "the square's composite is reported: {text}");
    // The rows cover the site once: every object lands in exactly one row.
    let total: usize = text
        .lines()
        .filter(|line| line.starts_with("  ") && line.contains(" objects"))
        .filter_map(|line| {
            line.split(" : ")
                .nth(1)
                .and_then(|tail| tail.split_whitespace().next())
                .and_then(|count| count.parse::<usize>().ok())
        })
        .sum();
    assert_eq!(total, host.site().objects.len(), "the rows cover the site once: {text}");
}

#[test]
fn the_physics_overlay_states_the_pulled_back_tuning() {
    let Some(fixture) = Fixture::load() else { return };
    for regime in RegimeId::ALL {
        let host = fixture.host_at(regime);
        let Some(Section::Table(table)) = host.resolve(PHYSICS) else {
            panic!("{PHYSICS} is not a table");
        };
        assert_eq!(
            table.columns.iter().map(|column| column.title.as_str()).collect::<Vec<_>>(),
            ["Part", "Sort", "Engine", "Tuning"]
        );
        assert!(table.rows.len() > 5, "one row per sort plus the law");
        let law = table.rows.last().expect("the law row");
        let text: Vec<String> = law
            .cells
            .iter()
            .map(|cell| match cell {
                panel_kit_core::widgets::table::TableCell::Text(text) => text.clone(),
                other => format!("{other:?}"),
            })
            .collect();
        assert!(text[3].contains("agrees"), "the overlay states the invariance: {text:?}");
        assert!(
            table.rows.iter().any(|row| {
                matches!(&row.cells[2], panel_kit_core::widgets::table::TableCell::Text(engine) if engine == "hydrophilic")
            }),
            "the regime sorts land on the base engine sorts"
        );
        assert!(
            table.rows.iter().any(|row| {
                matches!(&row.cells[3], panel_kit_core::widgets::table::TableCell::Text(tuning) if tuning.contains("repulsion 9, mass 1"))
            }),
            "the base's own numbers are pulled back: {table:?}"
        );
    }
}

#[test]
fn the_selected_overlay_names_the_object_its_route_and_its_neighbours() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host_at(RegimeId::OkfCatalog);
    assert!(text(&host, SELECTED).contains("nothing selected"));
    host.select(Some("acme_retail/orders"));
    let selected = text(&host, SELECTED);
    assert!(selected.contains("acme_retail/orders"), "{selected}");
    assert!(selected.contains("neighbours ("), "{selected}");
    // A session is the foreign kind in the catalog, so its fields arrive
    // through omega.
    host.select(Some("c0"));
    let session = text(&host, SELECTED);
    assert!(
        session.contains("read in okf-catalog through agentic --ω--> okf-catalog"),
        "{session}"
    );
    assert!(session.contains("trust <- energy"), "{session}");
    // The catalog workspace's own Selected panel mounts the same section.
    assert_eq!(host.resolve(SELECTED_OKF), host.resolve(SELECTED));

    let mut host = fixture.host_at(RegimeId::Agentic);
    host.select(Some("acme_retail/orders"));
    let selected = text(&host, SELECTED);
    assert!(
        selected.contains("read in agentic through okf-catalog --χ--> agentic"),
        "{selected}"
    );
    assert!(selected.contains("energy <- stale_after"), "{selected}");
    assert!(selected.contains("latency_ms <- stale_after"), "{selected}");
}

/// The Grammar editor shows the active grammar's own source, in that
/// grammar's language, and follows `g`.
#[test]
fn the_grammar_panel_shows_the_active_grammars_source() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host();
    for regime in RegimeId::ALL {
        host.set_regime(regime);
        assert_eq!(host.grammar_language(), "pest", "{regime}");
        let lines = text(&host, trio_host::GRAMMAR);
        assert!(lines.contains('~'), "{regime}: a pest grammar: {lines}");
        assert!(host.set_grammar(GRAMMAR_JSON));
        assert_eq!(host.grammar_language(), "json");
        let json = text(&host, trio_host::GRAMMAR);
        assert_ne!(json, lines, "{regime}: `g` changes the document");
        assert!(json.trim_start().starts_with('{'), "{regime}: a JSON grammar: {json}");
        assert!(host.set_grammar(GRAMMAR_LINES));
    }
}

/// Badges select what they name: a sort badge walks that sort, a tag badge
/// walks that tag, an object badge names its object; the count suffix is
/// ignored and a stranger changes nothing.
#[test]
fn badge_clicks_select_the_object_they_name() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host_at(RegimeId::Agentic);
    let kind_of = |host: &TrioHost, id: &str| {
        host.site().objects.iter().find(|o| o.id == id).map(|o| o.kind.clone()).unwrap()
    };
    host.select_matching("kind", "Reviewer 9");
    let first = host.selected().expect("a reviewer is selected").to_owned();
    assert_eq!(kind_of(&host, &first), "reviewer");
    host.select_matching("kind", "Reviewer 9");
    let second = host.selected().unwrap().to_owned();
    assert_ne!(first, second, "a repeated click walks to the next reviewer");
    assert_eq!(kind_of(&host, &second), "reviewer");

    host.set_regime(RegimeId::OkfCatalog);
    host.select_matching("value", "okf 27");
    let tagged = host.selected().unwrap().to_owned();
    let tag = host.site().objects.iter().find(|o| o.id == tagged).and_then(|o| o.tags.first().cloned());
    assert_eq!(tag.as_deref(), Some("okf"));

    host.select_matching("object", "Session c3");
    assert_eq!(host.selected(), Some("c3"));
    host.select_matching("kind", "Nobody 0");
    assert_eq!(host.selected(), Some("c3"), "an unmatched badge leaves the selection");
}

#[test]
fn both_grammars_read_the_one_site() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host();
    assert_eq!(host.grammar_id(), GRAMMAR_LINES);
    assert_eq!(host.grammar_ids(), &[GRAMMAR_LINES, GRAMMAR_JSON]);
    let lines = host.site().objects.iter().map(|object| object.id.clone()).collect::<Vec<_>>();
    let lines_edges = host.site().morphisms.clone();
    assert!(host.set_grammar(GRAMMAR_JSON));
    assert_eq!(host.grammar_id(), GRAMMAR_JSON);
    assert_eq!(
        host.site().objects.iter().map(|object| object.id.clone()).collect::<Vec<_>>(),
        lines,
        "the two grammars read one site"
    );
    assert_eq!(host.site().morphisms, lines_edges, "bond for bond");
    assert!(!host.set_grammar("nope"));
    host.cycle_grammar();
    assert_eq!(host.grammar_id(), GRAMMAR_LINES);
}

#[test]
fn the_graph_panel_declares_its_component_and_layout() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host();
    for regime in RegimeId::ALL {
        host.set_regime(regime);
        let component = host
            .components()
            .iter()
            .find(|component| component.id == "graph")
            .unwrap_or_else(|| panic!("{regime} declares the graph component"));
        assert!(component.keep_mounted, "an imperative canvas stays mounted");
        assert_eq!(component.events.len(), 2, "select and hover");
        let layout = host.graph_layout();
        assert_eq!(layout.algorithm, LayoutAlgorithm::Force, "{regime} runs the one pulled-back physics");
        assert!(
            text(&host, regime.graph_binding()).contains("TrioHost::graph()"),
            "the external panel's body is named"
        );
        assert!(!host.workspace_json().is_empty());
    }
    assert!(
        !text(&host, RegimeId::OkfCatalog.graph_binding()).is_empty(),
        "the catalog's graph panel has a body too"
    );
}

#[test]
fn an_omega_that_maps_the_reviewer_to_a_skill_fails_load_with_the_square_named() {
    let Some(fixture) = Fixture::load() else { return };
    TrioHost::load(&fixture.specs()).expect("the authored trio loads");

    // The reviewer is a policy in ω, and ψ sends a policy to the head where f
    // sends a reviewer. Map it to a skill instead and the square breaks.
    let tampered = fixture.omega.replace("\"reviewer\":\"policy\"", "\"reviewer\":\"skill\"");
    assert_ne!(tampered, fixture.omega, "the tamper reached the document");

    let mut specs = fixture.specs();
    specs.omega = &tampered;
    let error = TrioHost::load(&specs).err().expect("an ω that is not ψ⁻¹∘f must not load");
    assert!(error.contains("ψ ∘ ω = f"), "the law is named: {error}");
    assert!(error.contains("reviewer"), "the sort that breaks it is named: {error}");
    assert!(error.contains("head"), "and where f sends it: {error}");

    // A broken ψ is named against the triangle.
    let other = fixture.psi.replace("\"concept\":\"solvent\"", "\"concept\":\"tail\"");
    assert_ne!(other, fixture.psi);
    let mut specs = fixture.specs();
    specs.psi = &other;
    let error = TrioHost::load(&specs).err().expect("a ψ that is not χ then f must not load");
    assert!(error.contains("f ∘ χ = ψ"), "the law is named: {error}");
    assert!(error.contains("concept"), "the sort that breaks it is named: {error}");

    // A document that is not a morphism at all is named by its own id.
    let mut specs = fixture.specs();
    specs.omega = "{ not json";
    let error = TrioHost::load(&specs).err().expect("an undecodable ω must not load");
    assert!(error.contains("ω: topos-agentic -> topos-okf-catalog does not decode"), "{error}");
}