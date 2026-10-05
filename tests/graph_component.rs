//! The `graph` contract's host mounting: the layout text it describes, and
//! that the module compiles as an included support file.

// The test exercises only the pure helpers; the component itself is used by the okf host.
#[allow(dead_code)]
#[path = "../examples/support/graph_component.rs"]
mod graph_component;

use graph_component::describe;
use panel_kit_core::widgets::graph::{GraphLayout, LayoutAlgorithm};
use std::collections::BTreeMap;

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
