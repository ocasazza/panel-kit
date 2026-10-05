//! Host-contract test for the trio façade, over the real Nix JSON the
//! `topos-trio-test` check exports through `trioEnv`. Two sites and three
//! regimes, related by `f: agentic → membrane`, `ψ: okf-catalog → membrane`,
//! and `χ: okf-catalog → agentic` with `f ∘ χ = ψ`.
//!
//! Every workspace binding of every available (site, regime) pair resolves; the
//! trace site is refused in okf-catalog by name rather than faked; the graph's
//! mass, repulsion, weight, and rest length are identical across the regimes
//! that read one site while sorts and colours move; the trace site's assembly
//! reaches every stage; a selection on the OKF site survives the trip from
//! okf-catalog to membrane along `ψ`; and a ψ that no longer commutes with the
//! triangle fails `load` with the triangle named.
//!
//! Paths come from the environment at compile time through `option_env!`, so
//! this target compiles under every check and skips where the env is absent.

#[allow(dead_code)]
#[path = "../examples/support/trio_host.rs"]
mod trio_host;

use std::collections::BTreeSet;

use panel_kit_core::badge::Rgb;
use panel_kit_core::spec::WorkspaceSpec;
use panel_kit_core::theme::ThemeTokens;
use panel_kit_grammar::{Section, Stage};

use trio_host::{
    MorphismId, RegimeId, Route, SiteId, TrioHost, TrioSpecs, GRAMMAR_JSON, GRAMMAR_LINES,
    MORPHISM, NO_MORPHISM_TO_CATALOG, PHYSICS, SELECTED, SELECTED_OKF,
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

/// The sixteen Nix-authored documents `trioEnv` names; the fixture owns them,
/// so the borrowed [`TrioSpecs`] never outlives it.
struct Fixture {
    agentic: String,
    membrane: String,
    okf_catalog: String,
    base: String,
    f: String,
    psi: String,
    chi: String,
    agentic_physics: String,
    membrane_physics: String,
    catalog_physics: String,
    trace_lines_grammar: String,
    trace_lines_data: String,
    trace_json_grammar: String,
    trace_json_data: String,
    okf_grammar: String,
    okf_data: String,
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
            okf_grammar: read("PANEL_KIT_TRIO_OKF_GRAMMAR", exported!("PANEL_KIT_TRIO_OKF_GRAMMAR")),
            okf_data: read("PANEL_KIT_TRIO_OKF_DATA", exported!("PANEL_KIT_TRIO_OKF_DATA")),
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
            agentic_physics: &self.agentic_physics,
            membrane_physics: &self.membrane_physics,
            catalog_physics: &self.catalog_physics,
            trace_lines_grammar: &self.trace_lines_grammar,
            trace_lines_data: &self.trace_lines_data,
            trace_json_grammar: &self.trace_json_grammar,
            trace_json_data: &self.trace_json_data,
            okf_grammar: &self.okf_grammar,
            okf_data: &self.okf_data,
        }
    }

    fn host(&self) -> TrioHost {
        TrioHost::load(&self.specs()).expect("the trio host loads the authored specs")
    }

    /// A host reading `site` in `regime`.
    fn host_at(&self, site: SiteId, regime: RegimeId) -> TrioHost {
        let mut host = self.host();
        host.set_site(site);
        host.set_regime(regime)
            .unwrap_or_else(|error| panic!("{site}/{regime}: {error}"));
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

/// The (site, regime) pairs the host offers, in site then regime order.
fn available_pairs() -> Vec<(SiteId, RegimeId)> {
    let mut pairs = Vec::new();
    for site in SiteId::ALL {
        for regime in RegimeId::ALL {
            if TrioHost::available(site, regime).is_ok() {
                pairs.push((site, regime));
            }
        }
    }
    pairs
}

fn text(host: &TrioHost, binding: &str) -> String {
    match host.resolve(binding) {
        Some(Section::Text(model)) => model.text.clone(),
        Some(other) => panic!("{binding} is not text: {}", other.kind()),
        None => panic!("{binding} is unresolved"),
    }
}

#[test]
fn every_workspace_binding_resolves_under_every_available_pair() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host();
    let pairs = available_pairs();
    assert_eq!(pairs.len(), 5, "two regimes over traces, three over OKF");

    for (site, regime) in pairs {
        host.set_site(site);
        host.set_regime(regime).unwrap_or_else(|error| panic!("{site}/{regime}: {error}"));
        let spec = host.workspace_spec();
        assert_eq!(spec.id, format!("{}-ws", regime.topos_id()));
        let bound = bindings(spec);
        assert!(!bound.is_empty(), "{site}/{regime} has no binding-backed panels");
        for (binding, kind) in bound {
            let section = host
                .resolve(&binding)
                .unwrap_or_else(|| panic!("{binding} unresolved under {site}/{regime}"));
            if host.unreadable().contains(&binding.as_str()) {
                // A regime's sheaves are authored over one site; the panel that
                // asks for a field the other site lacks says so instead of
                // inventing a value.
                assert_eq!(section.kind(), "text", "{binding} degrades to a diagnostic");
                assert!(
                    text(&host, &binding).contains("does not read"),
                    "{binding} names why it is empty"
                );
            } else if kind == "external" {
                assert_eq!(section.kind(), "text", "{binding} answers with its placeholder");
            } else {
                assert_eq!(section.kind(), kind, "{binding} kind mismatch under {site}/{regime}");
            }
        }
    }
}

#[test]
fn the_okf_regimes_read_the_catalog_and_the_trace_regimes_read_the_trace() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host();
    for (site, regime) in available_pairs() {
        if !TrioHost::available(site, regime).expect("available").is_native() {
            continue;
        }
        host.set_site(site);
        host.set_regime(regime).expect("an available pair");
        assert_eq!(host.unreadable(), Vec::<&str>::new(), "{site}/{regime} reads its own site");
    }
    // The reverse: the agentic and membrane sheaves read the trace fields
    // `latency_ms` and `energy`, which the OKF corpus does not carry, so those
    // panels report it while the regime's own text, badges, assembly, and
    // physics panels still read the corpus.
    let missing: BTreeSet<&str> = ["latency_ms", "energy"].into_iter().collect();
    for regime in [RegimeId::Agentic, RegimeId::Membrane] {
        let host = fixture.host_at(SiteId::Okf, regime);
        let unreadable = host.unreadable();
        assert!(!unreadable.is_empty(), "{regime} names the panels it cannot read on OKF");
        for binding in unreadable {
            let text = text(&host, binding);
            assert!(text.contains("does not read"), "{binding} explains itself: {text}");
            assert!(
                missing.iter().any(|field| text.contains(field)),
                "{binding} names the field the corpus lacks: {text}"
            );
        }
        let assembly = binding_of(&host, regime, "assembly");
        let Some(Section::Table(table)) = host.resolve(&assembly) else {
            panic!("{assembly} is not a table");
        };
        assert_eq!(
            table.rows.len(),
            host.assembly().len(),
            "{regime} still counts the corpus's components"
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
fn the_trace_site_is_not_offered_in_the_okf_catalog_regime() {
    let Some(fixture) = Fixture::load() else { return };
    assert_eq!(
        TrioHost::available(SiteId::Traces, RegimeId::OkfCatalog).unwrap_err(),
        NO_MORPHISM_TO_CATALOG
    );

    let mut host = fixture.host();
    host.set_site(SiteId::Traces);
    assert_eq!(
        host.set_regime(RegimeId::OkfCatalog),
        Err(NO_MORPHISM_TO_CATALOG.to_owned()),
        "the refused switch is refused with the stated reason"
    );
    assert_eq!(host.regime(), RegimeId::Agentic, "and it changes nothing");

    // The OKF site is native in that regime, so the pair exists there.
    assert!(TrioHost::available(SiteId::Okf, RegimeId::OkfCatalog).expect("native").is_native());
}

#[test]
fn every_available_pair_routes_from_the_sites_native_regime() {
    let routes: Vec<(SiteId, RegimeId, Route)> = [
        (SiteId::Traces, RegimeId::Agentic, Route {
            native: RegimeId::Agentic,
            regime: RegimeId::Agentic,
            path: Vec::new(),
            composite: None,
        }),
        (SiteId::Traces, RegimeId::Membrane, Route {
            native: RegimeId::Agentic,
            regime: RegimeId::Membrane,
            path: vec![MorphismId::F],
            composite: None,
        }),
        (SiteId::Okf, RegimeId::OkfCatalog, Route {
            native: RegimeId::OkfCatalog,
            regime: RegimeId::OkfCatalog,
            path: Vec::new(),
            composite: None,
        }),
        (SiteId::Okf, RegimeId::Agentic, Route {
            native: RegimeId::OkfCatalog,
            regime: RegimeId::Agentic,
            path: vec![MorphismId::Chi],
            composite: None,
        }),
        (SiteId::Okf, RegimeId::Membrane, Route {
            native: RegimeId::OkfCatalog,
            regime: RegimeId::Membrane,
            path: vec![MorphismId::Psi],
            composite: Some(vec![MorphismId::Chi, MorphismId::F]),
        }),
    ]
    .into_iter()
    .map(|(site, regime, route)| {
        let offered = TrioHost::available(site, regime).unwrap_or_else(|error| panic!("{site}/{regime}: {error}"));
        assert_eq!(offered, route, "{site}/{regime} route");
        (site, regime, offered)
    })
    .collect();
    assert_eq!(routes.len(), 5);

    let okf_membrane = TrioHost::available(SiteId::Okf, RegimeId::Membrane).expect("available");
    assert_eq!(okf_membrane.composite, Some(vec![MorphismId::Chi, MorphismId::F]));
    assert!(
        okf_membrane.text().contains("ψ"),
        "the route names the morphism it takes: {}",
        okf_membrane.text()
    );
}

#[test]
fn the_graph_physics_is_identical_across_the_regimes_of_one_site() {
    let Some(fixture) = Fixture::load() else { return };
    /// One site's tuning as read in one regime: `(id, size, mass, repulsion)`
    /// per node and `(source, target, weight, rest length)` per edge.
    type Tuning = (Vec<(String, f32, f64, f64)>, Vec<(u32, u32, f64, f64)>);

    let mut host = fixture.host();
    for site in SiteId::ALL {
        let mut read: Option<Tuning> = None;
        for (at, regime) in RegimeId::ALL.into_iter().enumerate() {
            if TrioHost::available(site, regime).is_err() {
                continue;
            }
            host.set_site(site);
            host.set_regime(regime).expect("an available pair");
            let graph = host.graph();
            assert_eq!(graph.nodes.len(), host.site().objects.len(), "{site}/{regime} node count");
            assert_eq!(graph.edges.len(), host.site().morphisms.len(), "{site}/{regime} edge count");
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
                "{site}/{regime} reads real pulled-back tuning"
            );
            match &read {
                None => read = Some((nodes, edges)),
                Some((base_nodes, base_edges)) => {
                    assert_eq!(base_nodes, &nodes, "{site}/{regime} node tuning differs from regime {at}");
                    assert_eq!(base_edges, &edges, "{site}/{regime} edge tuning differs");
                }
            }
        }
        assert!(read.is_some(), "{site} has at least one available regime");
    }
}

#[test]
fn the_graph_sorts_and_colours_move_while_the_physics_does_not() {
    let Some(fixture) = Fixture::load() else { return };
    let palette = ThemeTokens::dark().colors;
    let fallback = (palette.fg.r, palette.fg.g, palette.fg.b);
    let mut host = fixture.host();

    for site in SiteId::ALL {
        let mut per_regime: Vec<(RegimeId, Vec<String>, Vec<Rgb>)> = Vec::new();
        for regime in RegimeId::ALL {
            if TrioHost::available(site, regime).is_err() {
                continue;
            }
            host.set_site(site);
            host.set_regime(regime).expect("an available pair");
            let graph = host.graph();
            assert!(
                graph.nodes.iter().all(|node| (node.color.0, node.color.1, node.color.2) != fallback),
                "{site}/{regime} paints every sort from the palette, not the fallback"
            );

            per_regime.push((
                regime,
                graph.nodes.iter().map(|node| node.sort.clone()).collect(),
                graph.nodes.iter().map(|node| node.color).collect(),
            ));
        }
        assert!(per_regime.len() >= 2, "{site} is read by at least two regimes");
        for (regime, sorts, colors) in &per_regime {
            assert!(
                sorts.iter().any(|sort| *sort != per_regime[0].1[0]),
                "{site}/{regime} sorts objects by its own sorts"
            );
            assert!(colors.iter().any(|color| *color != per_regime[0].2[0]), "{site}/{regime} recolours");
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
}

#[test]
fn the_trace_site_assembles_into_every_stage() {
    let Some(fixture) = Fixture::load() else { return };
    let mut summary: Vec<(SiteId, RegimeId, usize)> = Vec::new();
    for (site, regime) in [
        (SiteId::Traces, RegimeId::Agentic),
        (SiteId::Traces, RegimeId::Membrane),
        (SiteId::Okf, RegimeId::OkfCatalog),
        (SiteId::Okf, RegimeId::Agentic),
        (SiteId::Okf, RegimeId::Membrane),
    ] {
        let host = fixture.host_at(site, regime);
        let components = host.assembly();
        let stages: Vec<Stage> = components.iter().map(|component| component.stage).collect();
        if site == SiteId::Traces {
            for wanted in [Stage::Membrane, Stage::Tube, Stage::Sheet, Stage::Ring, Stage::Chain] {
                assert!(
                    stages.contains(&wanted),
                    "{regime}: the trace data has no {wanted:?} component, only {stages:?}"
                );
            }
        }
        let covered: usize = components.iter().map(|component| component.v).sum();
        assert!(
            covered <= host.site().objects.len() && !components.is_empty(),
            "{site}/{regime}: {covered} objects sit in components, out of {}",
            host.site().objects.len()
        );
        summary.push((site, regime, components.len()));
        println!(
            "{site}/{regime}: {} components: {:?}",
            components.len(),
            components
                .iter()
                .map(|component| (host.stage_label(component.stage), component.v, component.e, component.f))
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(summary.len(), 5, "every available pair assembles the active site");
}

#[test]
fn a_selection_survives_transport_from_okf_catalog_to_membrane() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host_at(SiteId::Okf, RegimeId::OkfCatalog);
    let node = host.site().objects[7].id.clone();
    host.select(Some(&node));
    assert_eq!(host.selected(), Some(node.as_str()));
    let before = host.active_sorts().clone();
    assert_eq!(before.len(), 1, "the catalog sort of the selected concept");

    host.set_regime(RegimeId::Membrane).expect("OKF reaches membrane along ψ");
    assert_eq!(host.route().expect("available").path, vec![MorphismId::Psi]);
    assert_eq!(host.selected(), Some(node.as_str()), "one site, so the node survives");
    let after = host.active_sorts().clone();
    assert!(!after.is_empty(), "the subobject travels with the selection");
    assert_ne!(before, after, "the sorts are read in the new regime");

    // And back: the pullback reaches every sort that maps into the fibre.
    host.set_regime(RegimeId::OkfCatalog).expect("available");
    assert_eq!(host.selected(), Some(node.as_str()));
    let back = host.active_sorts().clone();
    assert!(back.is_superset(&before), "f*∃f grows the subobject back: {back:?} ⊇ {before:?}");

    // The trace site carries a selection across f the same way.
    let mut traces = fixture.host_at(SiteId::Traces, RegimeId::Agentic);
    let node = traces.site().objects[3].id.clone();
    traces.select(Some(&node));
    assert_eq!(traces.active_sorts().len(), 1);
    traces.set_regime(RegimeId::Membrane).expect("traces reach membrane along f");
    assert_eq!(traces.selected(), Some(node.as_str()));
}

#[test]
fn cycling_the_selection_walks_the_site_and_clears_on_an_unknown_node() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host_at(SiteId::Traces, RegimeId::Agentic);
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
fn the_morphism_overlay_states_the_route_fibres_and_the_triangle() {
    let Some(fixture) = Fixture::load() else { return };
    let host = fixture.host_at(SiteId::Okf, RegimeId::Membrane);
    let text = text(&host, MORPHISM);
    assert!(text.contains("ψ: topos-okf-catalog -> topos-membrane"), "{text}");
    assert!(text.contains("χ then f"), "the composite is reported: {text}");
    assert!(text.contains("triangle: f ∘ χ = ψ"), "{text}");
    assert!(text.contains("φ fibres on the OKF site"), "{text}");
    // φ merges the three catalog source sorts into one head, so the head fibre
    // covers every table, dataset, and policy of the corpus.
    let head = text
        .lines()
        .find(|line| line.contains(" -> Head : "))
        .unwrap_or_else(|| panic!("no head fibre in {text}"));
    assert!(head.starts_with("  Dataset, Policy, Table -> Head"), "{head}");
    let count: usize = head
        .rsplit(": ")
        .next()
        .and_then(|tail| tail.split_whitespace().next())
        .unwrap_or_default()
        .parse()
        .expect("a node count");
    assert!(count >= 3, "table, dataset, and policy all fold into head: {head}");
    // The fibres partition the site: every object lands in exactly one.
    let total: usize = text
        .lines()
        .filter(|line| line.starts_with("  ") && line.contains(" -> "))
        .map(|line| {
            line.rsplit(": ")
                .next()
                .and_then(|tail| tail.split_whitespace().next())
                .unwrap_or_default()
                .parse::<usize>()
                .expect("a node count")
        })
        .sum();
    assert_eq!(total, host.site().objects.len(), "the fibres cover the corpus once: {text}");
}

#[test]
fn the_physics_overlay_states_the_pulled_back_tuning() {
    let Some(fixture) = Fixture::load() else { return };
    let host = fixture.host_at(SiteId::Traces, RegimeId::Membrane);
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
    assert!(text[3].contains("same engine"), "the overlay states the invariance: {text:?}");
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

#[test]
fn the_selected_overlay_names_the_object_and_its_neighbours() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host_at(SiteId::Okf, RegimeId::OkfCatalog);
    assert!(text(&host, SELECTED).contains("nothing selected"));
    let node = host.site().objects[9].id.clone();
    host.select(Some(&node));
    let text = text(&host, SELECTED);
    assert!(text.contains(&node), "{text}");
    assert!(text.contains("neighbours ("), "{text}");
    // The catalog workspace's own Selected panel mounts the same section.
    assert_eq!(host.resolve(SELECTED_OKF), host.resolve(SELECTED));
}

#[test]
fn both_trace_grammars_read_the_same_site() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host();
    assert_eq!(host.grammar_id(), GRAMMAR_LINES);
    let lines = host.site().objects.iter().map(|object| object.id.clone()).collect::<Vec<_>>();
    assert!(host.set_grammar(GRAMMAR_JSON));
    assert_eq!(host.grammar_id(), GRAMMAR_JSON);
    assert_eq!(
        host.site().objects.iter().map(|object| object.id.clone()).collect::<Vec<_>>(),
        lines,
        "the two grammars read one site"
    );
    assert!(!host.set_grammar("nope"));
    host.cycle_grammar();
    assert_eq!(host.grammar_id(), GRAMMAR_LINES);
    host.set_site(SiteId::Okf);
    assert_eq!(host.grammar_ids(), &["okf"]);
    host.cycle_grammar();
    assert_eq!(host.grammar_id(), "okf");
}

#[test]
fn the_graph_panel_declares_its_component_and_layout() {
    let Some(fixture) = Fixture::load() else { return };
    let mut host = fixture.host();
    for (site, regime) in available_pairs() {
        host.set_site(site);
        host.set_regime(regime).expect("an available pair");
        let component = host
            .components()
            .iter()
            .find(|component| component.id == "graph")
            .unwrap_or_else(|| panic!("{site}/{regime} declares the graph component"));
        assert!(component.keep_mounted, "an imperative canvas stays mounted");
        assert_eq!(component.events.len(), 2, "select and hover");
        let layout = host.graph_layout();
        assert_eq!(layout.group_by.as_deref(), Some("sort"));
        assert!(
            text(&host, regime.graph_binding()).contains("TrioHost::graph()"),
            "the external panel's body is named"
        );
        assert!(!host.workspace_json().is_empty());
    }
}

#[test]
fn a_broken_psi_fails_load_with_the_triangle_named() {
    let Some(fixture) = Fixture::load() else { return };
    TrioHost::load(&fixture.specs()).expect("the authored trio loads");

    // One object sort remapped. ψ stays total into the membrane sorts and still
    // reaches the catalog, so the sortings hold; only the triangle does not.
    let tampered = fixture.psi.replace("\"concept\":\"solvent\"", "\"concept\":\"tail\"");
    assert_ne!(tampered, fixture.psi, "the tamper reached the document");

    let mut specs = fixture.specs();
    specs.psi = &tampered;
    let error = TrioHost::load(&specs).err().expect("a ψ that is not χ then f must not load");
    assert!(error.contains("triangle f ∘ χ = ψ"), "the law is named: {error}");
    assert!(error.contains("concept"), "the sort that breaks it is named: {error}");

    // A different broken sort on the other leg is named the same way.
    let other = fixture.chi.replace("\"skill\":\"coder\"", "\"skill\":\"memory\"");
    assert_ne!(other, fixture.chi);
    let mut specs = fixture.specs();
    specs.chi = &other;
    let error = TrioHost::load(&specs).err().expect("a χ that does not compose to ψ must not load");
    assert!(error.contains("triangle f ∘ χ = ψ"), "the law is named: {error}");
    assert!(error.contains("skill"), "the sort that breaks it is named: {error}");

    // A document that is not a morphism at all is named by its own id.
    let mut specs = fixture.specs();
    specs.psi = "{ not json";
    let error = TrioHost::load(&specs).err().expect("an undecodable ψ must not load");
    assert!(error.contains("ψ: topos-okf-catalog -> topos-membrane does not decode"), "{error}");
}
