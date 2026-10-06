//! The `graph` contract's host mounting: the per-node and per-edge physics it
//! hands the GPU engine, the engine options that reproduce the CPU solve's
//! laws, and the layout text it describes.

// The test exercises only the pure helpers; the component itself is used by the trio host.
#[allow(dead_code)]
#[path = "../examples/support/graph_component.rs"]
mod graph_component;

use graph_component::graph_layout::{FORCE_LENGTH, FORCE_MIN_MASS};
use graph_component::{describe, edge_physics, force_options, node_physics};
use panel_kit_core::widgets::graph::{GraphEdge, GraphLayout, GraphModel, GraphNode, LayoutAlgorithm};
use std::collections::BTreeMap;

fn node(id: &str, mass: f64, repulsion: f64) -> GraphNode {
    GraphNode {
        id: id.into(),
        label: id.into(),
        sort: "s".into(),
        color: (1, 2, 3),
        size: 1.0,
        mass,
        repulsion,
    }
}

fn edge(source: u32, target: u32, weight: f64, rest_length: f64) -> GraphEdge {
    GraphEdge { source, target, kind: "k".into(), weight, rest_length }
}

#[test]
fn node_charges_keep_the_models_ratios_at_unit_mean() {
    // head 9, tail 3, solvent 0.5: mean 4.1666…
    let model = GraphModel {
        nodes: vec![node("head", 1.0, 9.0), node("tail", 1.5, 3.0), node("solvent", 0.25, 0.5)],
        edges: vec![],
    };
    let physics = node_physics(&model);
    let mean = physics.iter().map(|p| p.repulsion).sum::<f32>() / 3.0;
    assert!((mean - 1.0).abs() < 1e-5, "charges are normalised to unit mean: {mean}");
    assert!((physics[0].repulsion / physics[1].repulsion - 3.0).abs() < 1e-5, "head is 3x tail");
    assert!((physics[1].repulsion / physics[2].repulsion - 6.0).abs() < 1e-5, "tail is 6x solvent");
    assert_eq!([physics[0].mass, physics[1].mass, physics[2].mass], [1.0, 1.5, 0.25]);
}

#[test]
fn unusable_physics_falls_back_without_poisoning_the_rest() {
    let model = GraphModel {
        nodes: vec![node("a", f64::NAN, -2.0), node("b", 0.0, 2.0)],
        edges: vec![edge(0, 1, f64::INFINITY, -1.0)],
    };
    let nodes = node_physics(&model);
    assert_eq!(nodes[0].mass, 1.0, "a non-finite mass is unit mass");
    assert_eq!(nodes[0].repulsion, 1.0, "a non-positive charge is unit charge");
    assert_eq!(nodes[1].mass, FORCE_MIN_MASS as f32, "a weightless node keeps the floor mass");
    assert_eq!(nodes[1].repulsion, 2.0, "the mean counts every node, as the CPU solve does");
    let edges = edge_physics(&model);
    assert_eq!((edges[0].weight, edges[0].rest_length), (1.0, 0.0));
}

/// The engine's default repulsion-to-spring ratio at the rest length is the
/// regime it is known to be stable in; the options keep that ratio at the
/// canonical length, so a stiffer or longer rest length rescales repulsion.
#[test]
fn engine_options_keep_the_engines_stable_ratio_at_the_canonical_length() {
    let defaults = graph_layouts::GpuForceOptions::default();
    let options = force_options();
    let ratio = |o: &graph_layouts::GpuForceOptions| {
        f64::from(o.repulsion) / (f64::from(o.spring_k) * f64::from(o.spring_len).powi(3))
    };
    assert_eq!(options.spring_len as f64, FORCE_LENGTH);
    assert!(
        (ratio(&options) - ratio(&defaults)).abs() < 1e-12,
        "ratio {} vs the engine's {}",
        ratio(&options),
        ratio(&defaults)
    );
    // Rest length 1 is 200 world units at the defaults' 400, and the heaviest
    // base bond (weight 5) lands on the defaults' unit stiffness.
    assert!((options.spring_k * 5.0 - defaults.spring_k).abs() < 1e-6);
    assert_eq!(options.repulsion_radius as f64, 4.0 * FORCE_LENGTH);
    assert!(options.gravity > 0.0 && options.gravity < defaults.gravity, "weaker centring than the defaults");
}

#[test]
fn the_layout_description_names_the_solver_and_its_parameters() {
    let mut params = BTreeMap::new();
    params.insert("rank_separation".to_string(), 120.0);
    let layout = GraphLayout {
        algorithm: LayoutAlgorithm::Dagre,
        group_by: Some("sort".into()),
        params,
    };
    let text = describe(&layout);
    assert!(text.contains("Dagre"), "{text}");
    assert!(text.contains("rank_separation=120"), "{text}");
}
