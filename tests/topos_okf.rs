//! Host-contract test for the OKF graph-topoi façade, over the real Nix JSON
//! (the `topos-okf-test` check exports `okfEnv`): the pinned knowledge-catalog
//! snapshot parses to 57 objects and 160 morphisms; every workspace binding
//! resolves under both regimes; the graph is one graph read twice — same node
//! ids and order, regime sorts and colours; `f` validates and commutes over
//! the physics base; switching transports the selection's subobject by `∃f`
//! and back by `f*`; φ's fibres cover every node.
//!
//! Paths come from the environment at run time, so this target compiles under
//! every check.

#[allow(dead_code)]
#[path = "../examples/support/okf_host.rs"]
mod okf_host;

use std::collections::BTreeMap;

use panel_kit_core::spec::{ComponentContract, ComponentFallback, WorkspaceSpec};
use panel_kit_core::widgets::graph::LayoutAlgorithm;
use panel_kit_core::widgets::ContentSpec;
use panel_kit_grammar::{BaseTopos, GeometricMorphism, GrammarPackage, Physics, Section, Topos};

use okf_host::{OkfHost, OkfSpecs, CATALOG, GRAPH, LAYOUT, LINEAGE, MORPHISM, REPOSITORY, SELECTED};

fn read(var: &str) -> String {
    let path = std::env::var(var).unwrap_or_else(|_| panic!("{var} is set by the topos-okf-test check"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{var}={path}: {error}"))
}

/// The eight Nix-authored specs `okfEnv` names; the fixture owns them, so the
/// borrowed [`OkfSpecs`] never outlives it.
struct Fixture {
    grammar: String,
    data: String,
    catalog: String,
    lineage: String,
    base: String,
    morphism: String,
    catalog_physics: String,
    lineage_physics: String,
}

impl Fixture {
    fn load() -> Self {
        Self {
            grammar: read("PANEL_KIT_OKF_GRAMMAR"),
            data: read("PANEL_KIT_OKF_DATA"),
            catalog: read("PANEL_KIT_OKF_CATALOG"),
            lineage: read("PANEL_KIT_OKF_LINEAGE"),
            base: read("PANEL_KIT_OKF_BASE"),
            morphism: read("PANEL_KIT_OKF_MORPHISM"),
            catalog_physics: read("PANEL_KIT_OKF_CATALOG_PHYSICS"),
            lineage_physics: read("PANEL_KIT_OKF_LINEAGE_PHYSICS"),
        }
    }

    fn specs(&self) -> OkfSpecs<'_> {
        OkfSpecs {
            grammar: &self.grammar,
            data: &self.data,
            catalog: &self.catalog,
            lineage: &self.lineage,
            base: &self.base,
            morphism: &self.morphism,
            catalog_physics: &self.catalog_physics,
            lineage_physics: &self.lineage_physics,
        }
    }

    fn host(&self) -> OkfHost {
        OkfHost::load(&self.specs()).expect("the okf host loads the pinned snapshot")
    }

    /// A host with `id` active.
    fn host_from(&self, id: &str) -> OkfHost {
        let mut host = self.host();
        assert!(host.set_topos(id), "{id} is a regime");
        host
    }

    fn topos(&self, var: &str) -> Topos {
        let topos = Topos::from_json_str(&read(var)).expect("topos decodes");
        topos.validate().expect("topos validates");
        topos
    }

    fn morphism(&self, var: &str) -> GeometricMorphism {
        GeometricMorphism::from_json_str(&read(var)).expect("morphism decodes")
    }
}

fn text(host: &OkfHost, binding: &str) -> String {
    match host.resolve(binding) {
        Some(Section::Text(model)) => model.text.clone(),
        Some(other) => panic!("{binding} is not text: {}", other.kind()),
        None => panic!("{binding} is unresolved"),
    }
}

/// Every non-external panel binding of a workspace, with its content kind.
fn bindings(spec: &WorkspaceSpec) -> Vec<(String, &'static str)> {
    spec.panels
        .iter()
        .filter_map(|panel| match &panel.content {
            ContentSpec::External { .. } => None,
            content => content.binding_id().map(|id| (id.to_owned(), content.kind())),
        })
        .collect()
}

/// The solver's spec name, as `LayoutAlgorithm` serialises it.
fn algorithm_name(algorithm: &LayoutAlgorithm) -> &'static str {
    match algorithm {
        LayoutAlgorithm::Force => "force",
        LayoutAlgorithm::Circle => "circle",
        LayoutAlgorithm::Grid => "grid",
        LayoutAlgorithm::Concentric => "concentric",
        LayoutAlgorithm::Spectral => "spectral",
        LayoutAlgorithm::Dagre => "dagre",
        LayoutAlgorithm::Fcose => "fcose",
        LayoutAlgorithm::Sphere => "sphere",
    }
}

#[test]
fn the_pinned_snapshot_is_the_documented_graph() {
    let fixture = Fixture::load();
    let host = fixture.host();
    let graph = host.graph();

    assert_eq!(graph.nodes.len(), 57, "4 bundles + 53 concepts");
    assert_eq!(graph.edges.len(), 160, "53 contains + 8 derived_from + 2 executed_by + 97 references");
    assert_eq!(host.unresolved_links(), 0);
    assert!(
        graph.edges.iter().all(|edge| {
            (edge.source as usize) < graph.nodes.len() && (edge.target as usize) < graph.nodes.len()
        }),
        "every edge endpoint indexes a node"
    );
}

#[test]
fn each_regimes_panels_read_the_corpus() {
    let fixture = Fixture::load();
    let host = fixture.host();

    let Section::Table(concepts) = host.resolve("okf.concepts").expect("the concepts table") else {
        panic!("okf.concepts is not a table");
    };
    assert_eq!(concepts.rows.len(), 57, "every object has a row, bundles included");
    assert_eq!(
        concepts.columns.iter().map(|column| column.title.as_str()).collect::<Vec<_>>(),
        ["Concept", "Kind", "Status", "Trust", "Tags", "Stale after"]
    );

    let Section::Badges(tags) = host.resolve("okf.tags").expect("the tag strip") else {
        panic!("okf.tags is not badges");
    };
    assert_eq!(tags.len(), 53, "one badge per concept: bundles carry no tags");

    let Section::Gauges(kinds) = host.resolve("okf.kinds").expect("the kind gauges") else {
        panic!("okf.kinds is not gauges");
    };
    assert_eq!(kinds.len(), 8, "one gauge per OKF type the corpus exercises");

    let Section::Meter(stable) = host.resolve("okf.stable").expect("the stable-share meter") else {
        panic!("okf.stable is not a meter");
    };
    assert_eq!(stable.text, "8", "eight concepts declare status: stable");
    assert!(stable.ratio > 0.0 && stable.ratio < 1.0);

    let lineage = fixture.host_from(LINEAGE);
    let Section::Table(derivation) = lineage.resolve("okf.derivation").expect("the derivation table") else {
        panic!("okf.derivation is not a table");
    };
    assert_eq!(derivation.rows.len(), 10, "8 derived_from + 2 executed_by, no computes or attested_by");

    let Section::Badges(trust) = lineage.resolve("okf.trust").expect("the trust strip") else {
        panic!("okf.trust is not badges");
    };
    assert_eq!(trust.len(), 5, "one badge per derived artifact: 3 metrics + 2 computations");
    assert!(trust.iter().all(|badge| !badge.value.is_empty()), "derived artifacts all carry a tier");

    let Section::Gauges(roles) = lineage.resolve("okf.roles").expect("the role gauges") else {
        panic!("okf.roles is not gauges");
    };
    assert_eq!(roles.len(), 4, "source, derived, operator, context");
}

#[test]
fn every_workspace_binding_resolves_under_both_topoi() {
    let fixture = Fixture::load();
    let mut host = fixture.host();
    for id in host.topos_ids() {
        assert!(host.set_topos(id), "{id} is a regime");
        let spec = host.workspace_spec();
        assert!(spec.id.ends_with("-ws"), "{id} carries its own workspace");
        let bound = bindings(spec);
        assert!(!bound.is_empty(), "{id} has no binding-backed panels");
        for (binding, kind) in bound {
            let section =
                host.resolve(&binding).unwrap_or_else(|| panic!("{binding} unresolved under {id}"));
            assert_eq!(section.kind(), kind, "{binding} kind mismatch under {id}");
        }
    }
}

#[test]
fn both_workspaces_declare_the_graph_component_and_external_panel() {
    let fixture = Fixture::load();
    for (id, algorithm) in [(CATALOG, "concentric"), (LINEAGE, "dagre")] {
        let host = fixture.host_from(id);
        let components = host.components();
        assert_eq!(components.len(), 1, "{id} declares one external component");
        let graph = &components[0];
        assert_eq!(graph.id, "graph");
        assert_eq!(graph.contract, ComponentContract::Graph);
        assert_eq!(graph.fallback, ComponentFallback::Cells);
        assert!(graph.keep_mounted, "an imperative canvas stays mounted");
        assert_eq!(graph.requires.len(), 1);
        assert!(graph.min_size[0] > 0.0 && graph.min_size[1] > 0.0);

        let layout = host.graph_layout();
        assert_eq!(algorithm_name(&layout.algorithm), algorithm, "{id} solves {algorithm}");
        assert!(layout.groups_by("sort"), "{id} groups the graph by sort");
        assert!(!layout.params.is_empty(), "{id} tunes its solver");
        assert_eq!(host.topos_id(), id);
    }
}

#[test]
fn the_two_regimes_share_the_graph_and_recolour_it() {
    let fixture = Fixture::load();
    let catalog = fixture.host_from(CATALOG).graph();
    let lineage = fixture.host_from(LINEAGE).graph();

    assert_eq!(
        catalog.nodes.iter().map(|node| node.id.as_str()).collect::<Vec<_>>(),
        lineage.nodes.iter().map(|node| node.id.as_str()).collect::<Vec<_>>(),
        "site order is the site's, not the regime's"
    );
    assert_eq!(
        catalog.edges.iter().map(|edge| (edge.source, edge.target)).collect::<Vec<_>>(),
        lineage.edges.iter().map(|edge| (edge.source, edge.target)).collect::<Vec<_>>()
    );
    assert!(
        catalog.nodes.iter().zip(&lineage.nodes).all(|(left, right)| left.sort != right.sort),
        "phi renames every node's sort"
    );
    assert!(
        catalog.nodes.iter().zip(&lineage.nodes).any(|(left, right)| left.color != right.color),
        "the two regimes paint the same node differently"
    );
    assert!(lineage.edges.iter().all(|edge| matches!(
        edge.kind.as_str(),
        "derivation" | "execution" | "citation" | "containment"
    )));
    assert!(catalog.edges.iter().all(|edge| edge.weight > 0.0 && edge.rest_length > 0.0));
    // Node physics is the regime's pullback, not an authored number: the four
    // lineage roles share three engine sorts.
    assert!(lineage.nodes.iter().all(|node| node.mass > 0.0 && node.repulsion > 0.0));
}

#[test]
fn f_validates_and_commutes_over_the_physics_base() {
    let fixture = Fixture::load();
    let catalog = fixture.topos("PANEL_KIT_OKF_CATALOG");
    let lineage = fixture.topos("PANEL_KIT_OKF_LINEAGE");
    let f = fixture.morphism("PANEL_KIT_OKF_MORPHISM");
    let p_catalog = fixture.morphism("PANEL_KIT_OKF_CATALOG_PHYSICS");
    let p_lineage = fixture.morphism("PANEL_KIT_OKF_LINEAGE_PHYSICS");
    let base = BaseTopos::from_json_str(&read("PANEL_KIT_OKF_BASE")).expect("the base decodes");

    f.validate(&catalog, &lineage).expect("phi is total and lands in the lineage sorts");
    f.validate_over(&p_catalog, &p_lineage).expect("p_lineage . f = p_catalog");

    // Commutation is not only declared: both regimes induce one section.
    let site = fixture.host().site().clone();
    assert_eq!(
        catalog.physics_section(&site, &Physics::new(&base, &p_catalog, &catalog).unwrap()),
        lineage.physics_section(&site, &Physics::new(&base, &p_lineage, &lineage).unwrap()),
    );
}

#[test]
fn switching_transports_the_selection_by_image_then_pullback() {
    let fixture = Fixture::load();
    let mut host = fixture.host();
    host.select(Some("acme_retail/computations/gross-margin-period"));
    assert_eq!(host.selected(), Some("acme_retail/computations/gross-margin-period"));

    let forward = host.switch_topos();
    assert_eq!((forward.from.as_str(), forward.to.as_str()), (CATALOG, LINEAGE));
    assert_eq!(
        host.active_sorts().into_iter().collect::<Vec<_>>(),
        vec!["derived".to_owned()],
        "exists_f sends the catalog's metric sort to the lineage role"
    );

    let back = host.switch_topos();
    assert_eq!((back.from.as_str(), back.to.as_str()), (LINEAGE, CATALOG));
    assert_eq!(
        host.active_sorts().into_iter().collect::<Vec<_>>(),
        vec!["computation".to_owned(), "metric".to_owned()],
        "f* gives back the whole fibre: which type it was, is lost"
    );
    assert_eq!(host.selected(), Some("acme_retail/computations/gross-margin-period"));
}

#[test]
fn selecting_an_unknown_node_clears_rather_than_lies() {
    let fixture = Fixture::load();
    let mut host = fixture.host();
    host.select(Some("no/such/concept"));
    assert_eq!(host.selected(), None);

    host.cycle_selection(1);
    let first = host.selected().expect("cycling selects").to_owned();
    assert_eq!(first, host.site().objects[0].id, "forward from nothing starts at the first node");

    host.cycle_selection(-1);
    let last = host.selected().expect("cycling selects").to_owned();
    assert_eq!(last, host.site().objects[56].id, "back from nothing wraps to the last node");

    host.cycle_selection(1);
    assert_eq!(host.selected(), Some(first.as_str()), "one step forward returns to the first");
}

#[test]
fn phi_fibres_cover_every_node() {
    let fixture = Fixture::load();
    let host = fixture.host();
    let fibres: BTreeMap<String, (Vec<String>, usize)> = host
        .fibres()
        .into_iter()
        .map(|fibre| (fibre.codomain_sort, (fibre.domain_sorts, fibre.nodes)))
        .collect();

    assert_eq!(
        fibres.keys().map(String::as_str).collect::<Vec<_>>(),
        ["context", "derived", "operator", "source"]
    );
    assert_eq!(
        fibres["source"],
        (vec!["dataset".to_owned(), "policy".to_owned(), "table".to_owned()], 27)
    );
    assert_eq!(fibres["derived"], (vec!["computation".to_owned(), "metric".to_owned()], 5));
    assert_eq!(fibres["operator"], (vec!["skill".to_owned()], 1));
    assert_eq!(
        fibres["context"],
        (
            vec!["bundle".to_owned(), "concept".to_owned(), "log".to_owned(), "reference".to_owned()],
            24
        )
    );
    assert_eq!(fibres.values().map(|(_, nodes)| nodes).sum::<usize>(), 57, "phi covers every node");
}

#[test]
fn the_layout_overlay_names_both_solvers() {
    let fixture = Fixture::load();
    let mut host = fixture.host();
    let forward = text(&host, LAYOUT);
    assert!(forward.contains("concentric -> dagre"), "{forward}");
    assert!(forward.contains("pulled back into topos-okf-lineage"), "{forward}");

    host.switch_topos();
    let back = text(&host, LAYOUT);
    assert!(back.contains("concentric -> dagre"), "{back}");
    assert!(back.contains("context"), "the overlay states phi: {back}");
}

#[test]
fn the_selected_overlay_names_the_selected_concepts_neighbours() {
    let fixture = Fixture::load();
    let mut host = fixture.host();
    assert!(text(&host, SELECTED).contains("nothing selected"));

    host.select(Some("acme_retail/computations/gross-margin-period"));
    let overlay = text(&host, SELECTED);
    assert!(overlay.contains("acme_retail/computations/gross-margin-period"), "{overlay}");
    assert!(overlay.contains("trust: human-reviewed"), "{overlay}");
    assert!(overlay.contains("tags: attested, finance, margin"), "{overlay}");

    let graph = host.graph();
    let index = graph.nodes.iter().position(|node| node.id == "acme_retail/computations/gross-margin-period").unwrap();
    let neighbours: Vec<&str> = graph
        .edges
        .iter()
        .filter(|edge| edge.source as usize == index || edge.target as usize == index)
        .map(|edge| {
            let other = if edge.source as usize == index { edge.target } else { edge.source } as usize;
            graph.nodes[other].id.as_str()
        })
        .collect();
    assert!(!neighbours.is_empty());
    for neighbour in neighbours {
        assert!(overlay.contains(neighbour), "{overlay} is missing neighbour {neighbour}");
    }
    assert!(overlay.contains("neighbours ("), "{overlay}");
}

#[test]
fn the_morphism_overlay_shows_the_adjoint_triple() {
    let fixture = Fixture::load();
    let mut host = fixture.host();
    host.select(Some("ga4/references/metrics/purchasers"));
    let domain = text(&host, MORPHISM);
    assert!(domain.contains("f: topos-okf-catalog -> topos-okf-lineage"), "{domain}");
    assert!(domain.contains("∃f(U)"), "{domain}");
    assert!(domain.contains("∀f(U)"), "{domain}");

    host.switch_topos();
    let codomain = text(&host, MORPHISM);
    assert!(codomain.contains("f*(U)"), "{codomain}");
}

#[test]
fn the_repository_overlay_names_the_snapshot_and_the_grammar_rules() {
    let fixture = Fixture::load();
    let overlay = text(&fixture.host(), REPOSITORY);
    assert!(overlay.contains("GoogleCloudPlatform/knowledge-catalog"), "{overlay}");
    assert!(overlay.contains("rev: 58e16bdb"), "{overlay}");
    assert!(overlay.contains("acme_retail"), "{overlay}");
    assert!(overlay.contains("53 concepts, 160 morphisms, 0 unresolved links"), "{overlay}");
    assert!(overlay.contains("BigQuery Table -> table"), "{overlay}");
    assert!(overlay.contains("sources[*].resource -> derived_from"), "{overlay}");
    assert_eq!(GrammarPackage::from_json_str(&read("PANEL_KIT_OKF_GRAMMAR")).unwrap().metadata.id, "okf");
}

#[test]
fn the_graph_panel_is_mounted_under_the_graph_binding() {
    let fixture = Fixture::load();
    for id in fixture.host().topos_ids() {
        let host = fixture.host_from(id);
        let panel = host
            .workspace_spec()
            .panels
            .iter()
            .find(|panel| matches!(panel.content, ContentSpec::External { .. }))
            .expect("the workspace mounts the graph");
        match &panel.content {
            ContentSpec::External { component, source, .. } => {
                assert_eq!(component, "graph");
                assert_eq!(source.binding_id(), Some(GRAPH));
            }
            other => panic!("{id}: {other:?}"),
        }
        assert_eq!(host.topos_id(), id);
    }
}
