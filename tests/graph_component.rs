//! The `graph` contract's host mounting: the physics it hands the GPU force
//! engine, and that the module compiles as an included support file.

// The test exercises only the pure helpers; the component itself is used by the okf host.
#[allow(dead_code)]
#[path = "../examples/support/graph_component.rs"]
mod graph_component;

use graph_component::describe;
use panel_kit_core::widgets::graph::{GraphEdge, GraphLayout, GraphModel, GraphNode, LayoutAlgorithm};
use std::collections::BTreeMap;

fn node(id: &str) -> GraphNode {
    GraphNode {
        id: id.into(),
        label: id.into(),
        sort: "s".into(),
        color: (1, 2, 3),
        size: 1.0,
        mass: 2.0,
        repulsion: 1000.0,
    }
}

fn model() -> GraphModel {
    GraphModel {
        nodes: vec![node("a"), node("b")],
        edges: vec![GraphEdge {
            source: 0,
            target: 1,
            kind: "k".into(),
            weight: 3.0,
            rest_length: 250.0,
        }],
    }
}

#[test]
fn physics_aggregates_into_the_force_engine_options() {
    let options = graph_component::force_options(&model());
    assert_eq!(options.repulsion, 2000.0, "mean repulsion x mean mass");
    assert_eq!(options.spring_k, 3.0, "mean edge weight");
    assert_eq!(options.spring_len, 250.0, "mean edge rest length");
    assert_eq!(options.repulsion_radius, 1000.0, "4 x spring length");
}

#[test]
fn an_empty_model_keeps_the_engine_defaults() {
    let defaults = graph_component::force_options(&GraphModel::default());
    assert_eq!(defaults.repulsion, 4000.0, "engine default");
    assert_eq!(defaults.spring_len, 400.0, "engine default");
    assert_eq!(defaults.repulsion_radius, 1600.0, "engine default");
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
