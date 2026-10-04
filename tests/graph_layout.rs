//! Layout solver contract: every algorithm positions every node, twice the
//! same, and grouping by sort moves the arrangement.

#[path = "../examples/support/graph_layout.rs"]
mod graph_layout;

use std::collections::BTreeMap;

use graph_layout::{Tween, fibonacci_sphere, project_2d, solve};

use panel_kit_core::widgets::graph::{
    GraphEdge, GraphLayout, GraphModel, GraphNode, LayoutAlgorithm,
};

const ALGORITHMS: [LayoutAlgorithm; 8] = [
    LayoutAlgorithm::Force,
    LayoutAlgorithm::Circle,
    LayoutAlgorithm::Grid,
    LayoutAlgorithm::Concentric,
    LayoutAlgorithm::Spectral,
    LayoutAlgorithm::Dagre,
    LayoutAlgorithm::Fcose,
    LayoutAlgorithm::Sphere,
];

fn node(id: &str, sort: &str) -> GraphNode {
    GraphNode {
        id: id.into(),
        label: id.to_uppercase(),
        sort: sort.into(),
        color: (120, 160, 200),
        size: 2.0,
        mass: 1.0,
        repulsion: 1000.0,
    }
}

fn edge(source: u32, target: u32, kind: &str) -> GraphEdge {
    GraphEdge {
        source,
        target,
        kind: kind.into(),
        weight: 1.0,
        rest_length: 60.0,
    }
}

/// Two three-node groups joined by one morphism: enough topology for every
/// solver to do something other than return zeros.
fn model() -> GraphModel {
    GraphModel {
        nodes: vec![
            node("a", "metric"),
            node("b", "metric"),
            node("c", "dataset"),
            node("d", "dataset"),
            node("e", "policy"),
        ],
        edges: vec![
            edge(0, 2, "derived_from"),
            edge(1, 2, "derived_from"),
            edge(2, 4, "executed_by"),
            edge(0, 4, "references"),
        ],
    }
}

fn layout(algorithm: LayoutAlgorithm, group_by: Option<&str>) -> GraphLayout {
    let mut params = BTreeMap::new();
    params.insert("rank_separation".to_string(), 120.0);
    params.insert("level_spacing".to_string(), 90.0);
    GraphLayout {
        algorithm,
        group_by: group_by.map(str::to_string),
        params,
    }
}

#[test]
fn every_algorithm_positions_every_node_finitely() {
    let model = model();
    for algorithm in ALGORITHMS {
        let positions = solve(&model, &layout(algorithm, None), None);
        assert_eq!(
            positions.len(),
            model.nodes.len(),
            "{algorithm:?} must return one position per node"
        );
        for (index, point) in positions.iter().enumerate() {
            assert!(
                point.iter().all(|value| value.is_finite()),
                "{algorithm:?} node {index} is not finite: {point:?}"
            );
        }
    }
}

#[test]
fn solving_is_deterministic() {
    let model = model();
    for algorithm in ALGORITHMS {
        let request = layout(algorithm, None);
        assert_eq!(
            solve(&model, &request, None),
            solve(&model, &request, None),
            "{algorithm:?} must not depend on unseeded randomness"
        );
        let seeded = solve(&model, &request, Some(&fibonacci_sphere(model.nodes.len(), 50.0)));
        assert_eq!(seeded, solve(&model, &request, Some(&fibonacci_sphere(5, 50.0))));
    }
}

#[test]
fn grouping_by_sort_moves_the_arrangement() {
    let model = model();
    for algorithm in ALGORITHMS {
        let whole = solve(&model, &layout(algorithm, None), None);
        let grouped = solve(&model, &layout(algorithm, Some("sort")), None);
        assert_eq!(grouped.len(), whole.len(), "{algorithm:?}");
        assert_ne!(
            whole, grouped,
            "{algorithm:?} must compose sorts differently when grouped"
        );
    }
}

#[test]
fn an_unknown_group_field_leaves_the_graph_whole() {
    let model = model();
    for algorithm in ALGORITHMS {
        let whole = solve(&model, &layout(algorithm, None), None);
        let unknown = solve(&model, &layout(algorithm, Some("mass")), None);
        assert_eq!(whole, unknown, "{algorithm:?}");
    }
}

#[test]
fn force_seeds_the_simulator_from_the_live_positions() {
    let model = model();
    let live: Vec<[f32; 3]> = (0..model.nodes.len())
        .map(|index| [index as f32, 0.0, 0.0])
        .collect();
    let positions = solve(&model, &layout(LayoutAlgorithm::Force, None), Some(&live));
    assert_eq!(positions, live);
}

#[test]
fn a_wrong_length_seed_falls_back_to_the_sphere() {
    let model = model();
    let short: Vec<[f32; 3]> = vec![[0.0; 3]];
    let positions = solve(&model, &layout(LayoutAlgorithm::Force, None), Some(&short));
    assert_eq!(positions, fibonacci_sphere(model.nodes.len(), 200.0));
}

#[test]
fn duplicate_node_ids_still_solve_every_node() {
    let mut model = model();
    model.nodes[1].id = model.nodes[0].id.clone();
    for algorithm in ALGORITHMS {
        let positions = solve(&model, &layout(algorithm, None), None);
        assert_eq!(
            positions.len(),
            model.nodes.len(),
            "{algorithm:?} must not collapse repeated ids"
        );
        assert!(positions.iter().all(|p| p.iter().all(|v| v.is_finite())));
    }
}

#[test]
fn an_empty_model_solves_to_nothing() {
    let model = GraphModel::default();
    assert!(solve(&model, &layout(LayoutAlgorithm::Dagre, None), None).is_empty());
    assert!(project_2d(&[]).is_empty());
}

#[test]
fn edges_outside_the_model_are_dropped() {
    let mut model = model();
    model.edges.push(edge(0, 99, "references"));
    for algorithm in ALGORITHMS {
        let positions = solve(&model, &layout(algorithm, None), None);
        assert_eq!(positions.len(), model.nodes.len(), "{algorithm:?}");
        assert!(positions.iter().all(|p| p.iter().all(|v| v.is_finite())));
    }
}

#[test]
fn projection_keeps_planar_layouts_and_normalises_to_the_unit_square() {
    let planar = vec![[0.0, 10.0, 0.0], [20.0, 0.0, 0.0], [10.0, 5.0, 0.0]];
    let projected = project_2d(&planar);
    assert_eq!(projected.len(), 3);
    assert!(projected.iter().all(|p| p[0] >= 0.0 && p[0] <= 1.0));
    assert!(projected.iter().all(|p| p[1] >= 0.0 && p[1] <= 1.0));
    // The flat axis is dropped, so the arrangement survives the projection.
    assert_eq!(projected[0], [0.0, 1.0]);
    assert_eq!(projected[1], [1.0, 0.0]);
    assert_eq!(projected[2], [0.5, 0.5]);
    assert_eq!(project_2d(&[[1.0, 2.0, 3.0]]), vec![[0.5, 0.5]]);
}

#[test]
fn projection_drops_the_narrowest_axis_of_a_sphere() {
    let sphere = fibonacci_sphere(24, 100.0);
    let projected = project_2d(&sphere);
    assert_eq!(projected.len(), sphere.len());
    assert!(projected.iter().all(|p| p[0].is_finite() && p[1].is_finite()));
    let span = projected
        .iter()
        .map(|p| p[0])
        .fold(f32::MIN, f32::max)
        - projected.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
    assert!(span > 0.9, "a sphere seen from any axis spans the unit square");
}

#[test]
fn tween_hits_its_endpoints_and_eases_in_out() {
    let from = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]];
    let to = vec![[100.0, 0.0, 0.0], [10.0, 0.0, 0.0]];
    let tween = Tween::new(from.clone(), to.clone(), 800, false);

    assert_eq!(tween.at(0), from);
    assert_eq!(tween.at(800), to);
    assert_eq!(tween.at(4000), to);
    assert!(!tween.is_complete(400));
    assert!(tween.is_complete(800));

    let quarter = tween.at(200)[0][0];
    let half = tween.at(400)[0][0];
    let three_quarters = tween.at(600)[0][0];
    assert!(quarter < half && half < three_quarters, "easing must be monotone");
    assert!(half > 40.0 && half < 60.0, "ease-in-out is slow at the midpoint");
}

#[test]
fn reduced_motion_makes_the_tween_instant() {
    let from = vec![[0.0; 3]];
    let to = vec![[9.0, 8.0, 7.0]];
    let tween = Tween::new(from, to.clone(), 800, true);
    assert!(tween.is_complete(0));
    assert_eq!(tween.at(0), to);
    assert_eq!(tween.at(1), to);
}

#[test]
fn a_tween_with_nothing_to_interpolate_lands_on_the_target() {
    let tween = Tween::new(vec![[0.0; 3]], vec![[1.0; 3], [2.0; 3]], 800, false);
    assert_eq!(tween.at(0), vec![[1.0; 3], [2.0; 3]]);
}