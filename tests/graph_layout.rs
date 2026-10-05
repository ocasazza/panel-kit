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

/// `rest_length` is in units of the seed's mean radius, matching the
/// dimensionless rest lengths of the pulled-back physics base.
fn edge(source: u32, target: u32, kind: &str) -> GraphEdge {
    GraphEdge {
        source,
        target,
        kind: kind.into(),
        weight: 1.0,
        rest_length: 1.0,
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
    for algorithm in ALGORITHMS.into_iter().filter(|a| *a != LayoutAlgorithm::Force) {
        let whole = solve(&model, &layout(algorithm, None), None);
        let grouped = solve(&model, &layout(algorithm, Some("sort")), None);
        assert_eq!(grouped.len(), whole.len(), "{algorithm:?}");
        assert_ne!(
            whole, grouped,
            "{algorithm:?} must compose sorts differently when grouped"
        );
    }
}

/// Force assembles connected components, never sorts: a closed surface whose
/// nodes carry three sorts stays one shell, and a separate chain lands beside
/// it instead of inside it.
#[test]
fn force_assembles_each_component_whole_and_apart() {
    let mut model = icosahedron();
    let sorts = ["planner", "coder", "reviewer"];
    for (index, node) in model.nodes.iter_mut().enumerate() {
        node.sort = sorts[index % sorts.len()].into();
    }
    let offset = model.nodes.len() as u32;
    let tail = chain(8, 1.0, 1.0);
    model.nodes.extend(tail.nodes);
    model.edges.extend(tail.edges.into_iter().map(|edge| GraphEdge {
        source: edge.source + offset,
        target: edge.target + offset,
        ..edge
    }));

    let mut request = force_layout();
    request.group_by = Some("sort".into());
    let positions = solve(&model, &request, None);
    assert_eq!(positions, solve(&model, &force_layout(), None), "group_by must not split a force solve");

    let (shell, rod) = positions.split_at(offset as usize);
    let shell_centre = centre(shell);
    let radii: Vec<f64> = shell.iter().map(|p| distance(*p, shell_centre)).collect();
    let shell_radius = mean(radii.iter().copied());
    for (index, radius) in radii.iter().enumerate() {
        assert!(
            (radius - shell_radius).abs() <= 0.25 * shell_radius,
            "node {index} sits at radius {radius}, mean {shell_radius}: {radii:?}"
        );
    }
    let nearest = rod
        .iter()
        .map(|p| distance(*p, shell_centre))
        .fold(f64::INFINITY, f64::min);
    assert!(
        nearest > 1.5 * shell_radius,
        "the chain reaches {nearest} from the shell's centre, inside radius {shell_radius}"
    );
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
fn force_relaxes_from_the_live_positions() {
    let mut model = model();
    model.edges.push(edge(3, 2, "derived_from"));
    let request = force_layout();
    let live = fibonacci_sphere(model.nodes.len(), 400.0);
    let relaxed = solve(&model, &request, Some(&live));
    assert_ne!(relaxed, live, "the solve must relax, not echo the seed");
    // The seed is the starting arrangement, not a hint: the solved layout keeps
    // the seed's own scale and centre, so a topos switch tween begins where the
    // GPU sim already was instead of jumping to a fresh sphere.
    let centre = |points: &[[f32; 3]]| {
        [
            mean(points.iter().map(|p| f64::from(p[0]))),
            mean(points.iter().map(|p| f64::from(p[1]))),
            mean(points.iter().map(|p| f64::from(p[2]))),
        ]
    };
    let radius = |points: &[[f32; 3]]| {
        let c = centre(points);
        mean(points.iter().map(|p| {
            (0..3)
                .map(|axis| f64::from(p[axis]) - c[axis])
                .fold(0.0_f64, |sum, v| sum.hypot(v))
        }))
    };
    assert!(
        (radius(&relaxed) - radius(&live)).abs() < 0.5 * radius(&live),
        "the solve kept radius {} against the seed's {}",
        radius(&relaxed),
        radius(&live)
    );
    for axis in 0..3 {
        assert!(
            (centre(&relaxed)[axis] - centre(&live)[axis]).abs() < 0.5 * radius(&live),
            "the solve drifted off the seed's centre"
        );
    }
}

#[test]
fn a_wrong_length_seed_falls_back_to_the_sphere() {
    let model = model();
    let short: Vec<[f32; 3]> = vec![[0.0; 3]];
    let request = force_layout();
    let positions = solve(&model, &request, Some(&short));
    assert_eq!(positions.len(), model.nodes.len());
    assert!(positions.iter().all(|p| p.iter().all(|v| v.is_finite())));
    assert_eq!(
        positions,
        solve(&model, &request, None),
        "a wrong-length seed must solve like no seed at all"
    );
}

/// An icosahedron: 12 nodes, 30 edges, uniform physics. A closed triangulated
/// surface is the shell case — repulsion alone spreads every node, the springs
/// hold neighbours at one rest length, and the equilibrium is a shell.
fn icosahedron() -> GraphModel {
    let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let corners = [
        (-1.0, phi, 0.0),
        (1.0, phi, 0.0),
        (-1.0, -phi, 0.0),
        (1.0, -phi, 0.0),
        (0.0, -1.0, phi),
        (0.0, 1.0, phi),
        (0.0, -1.0, -phi),
        (0.0, 1.0, -phi),
        (phi, 0.0, -1.0),
        (phi, 0.0, 1.0),
        (-phi, 0.0, -1.0),
        (-phi, 0.0, 1.0),
    ];
    // The 20 triangular faces of the icosahedron; their edge set is the graph.
    let faces = [
        (0, 11, 5),
        (0, 5, 1),
        (0, 1, 7),
        (0, 7, 10),
        (0, 10, 11),
        (1, 5, 9),
        (5, 11, 4),
        (11, 10, 2),
        (10, 7, 6),
        (7, 1, 8),
        (3, 9, 4),
        (3, 4, 2),
        (3, 2, 6),
        (3, 6, 8),
        (3, 8, 9),
        (4, 9, 5),
        (2, 4, 11),
        (6, 2, 10),
        (8, 6, 7),
        (9, 8, 1),
    ];
    let mut seen = std::collections::BTreeSet::new();
    let mut edges = Vec::new();
    for (a, b, c) in faces {
        for (source, target) in [(a, b), (b, c), (c, a)] {
            let key = if source < target {
                (source, target)
            } else {
                (target, source)
            };
            if seen.insert(key) {
                edges.push(edge(key.0 as u32, key.1 as u32, "tail_bond"));
            }
        }
    }
    assert_eq!(edges.len(), 30, "the icosahedron has 30 edges");
    GraphModel {
        nodes: corners
            .iter()
            .enumerate()
            .map(|(index, _)| node(&format!("v{index:02}"), "solvent"))
            .collect(),
        edges,
    }
}

/// A chain of `count` nodes: no cycle, so nothing but the springs can hold the
/// ends apart.
fn chain(count: usize, rest_length: f64, weight: f64) -> GraphModel {
    GraphModel {
        nodes: (0..count)
            .map(|index| node(&format!("n{index:03}"), "tail"))
            .collect(),
        edges: (1..count as u32)
            .map(|index| GraphEdge {
                source: index - 1,
                target: index,
                kind: "tail_bond".into(),
                weight,
                rest_length,
            })
            .collect(),
    }
}

fn force_layout() -> GraphLayout {
    GraphLayout {
        algorithm: LayoutAlgorithm::Force,
        group_by: None,
        params: BTreeMap::new(),
    }
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let values: Vec<f64> = values.collect();
    assert!(!values.is_empty());
    values.iter().sum::<f64>() / values.len() as f64
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f64 {
    (0..3)
        .map(|axis| f64::from(a[axis]) - f64::from(b[axis]))
        .fold(0.0_f64, |sum, value| sum.hypot(value))
}

fn centre(points: &[[f32; 3]]) -> [f32; 3] {
    let axis = |index: usize| mean(points.iter().map(|p| f64::from(p[index]))) as f32;
    [axis(0), axis(1), axis(2)]
}

#[test]
fn force_lays_a_closed_surface_out_as_a_shell() {
    let model = icosahedron();
    // A flat seed: the previous force behaviour returned it verbatim, so every
    // node sat in one plane and the shell assertion below could not hold.
    let seed: Vec<[f32; 3]> = (0..model.nodes.len())
        .map(|index| [index as f32 * 20.0, 0.0, 0.0])
        .collect();

    let positions = solve(&model, &force_layout(), Some(&seed));
    assert_eq!(positions.len(), model.nodes.len());
    let centre = (0..3)
        .map(|axis| mean(positions.iter().map(|p| f64::from(p[axis]))))
        .collect::<Vec<_>>();
    let radii: Vec<f64> = positions
        .iter()
        .map(|p| {
            (0..3)
                .map(|axis| f64::from(p[axis]) - centre[axis])
                .fold(0.0_f64, |sum, value| sum.hypot(value))
        })
        .collect();
    let mean_radius = mean(radii.iter().copied());
    assert!(mean_radius > 1.0, "the shell must have a real radius: {radii:?}");
    for (index, radius) in radii.iter().enumerate() {
        assert!(
            (radius - mean_radius).abs() <= 0.25 * mean_radius,
            "node {index} sits at radius {radius}, mean {mean_radius}: {radii:?}"
        );
    }
}

#[test]
fn force_keeps_a_chain_stretched_out() {
    let model = chain(14, 1.0, 1.0);
    let positions = solve(&model, &force_layout(), None);
    assert_eq!(positions.len(), model.nodes.len());
    let mean_edge = mean((0..model.nodes.len() - 1).map(|i| distance(positions[i], positions[i + 1])));
    let end_to_end = distance(positions[0], positions[model.nodes.len() - 1]);
    assert!(
        end_to_end > 0.6 * (model.nodes.len() - 1) as f64 * mean_edge,
        "chain ends {end_to_end} apart over a mean edge of {mean_edge}"
    );
}

/// A camera looks down z, so a ring must lie across x and y, widest along x,
/// rather than stand edge-on as a line.
#[test]
fn force_turns_a_ring_to_face_the_viewer() {
    let mut model = chain(12, 1.0, 1.0);
    model.edges.push(edge(11, 0, "tail_bond"));
    let positions = solve(&model, &force_layout(), None);
    let extent = |axis: usize| {
        let values = positions.iter().map(|p| p[axis]);
        values.clone().fold(f32::MIN, f32::max) - values.fold(f32::MAX, f32::min)
    };
    let (x, y, z) = (extent(0), extent(1), extent(2));
    assert!(x >= y && y >= z, "extents x {x}, y {y}, z {z} must narrow towards z");
    assert!(z < 0.5 * y, "the ring stands {z} deep across a {y} face");
}

#[test]
fn force_is_deterministic_across_runs() {
    let model = icosahedron();
    let seed = fibonacci_sphere(model.nodes.len(), 120.0);
    let request = force_layout();
    assert_eq!(
        solve(&model, &request, Some(&seed)),
        solve(&model, &request, Some(&seed)),
        "the CPU force solve must not depend on unseeded randomness"
    );
    assert_eq!(solve(&model, &request, None), solve(&model, &request, None));
}

#[test]
fn stiffer_shorter_edges_end_shorter_than_weak_long_ones() {
    let stiff = solve(&chain(14, 0.4, 5.0), &force_layout(), None);
    let weak = solve(&chain(14, 2.0, 0.5), &force_layout(), None);
    let edge_length = |positions: &[[f32; 3]]| {
        mean((0..positions.len() - 1).map(|i| distance(positions[i], positions[i + 1])))
    };
    assert!(
        edge_length(&stiff) < edge_length(&weak),
        "stiff short edges drew up to {} against weak long ones at {}",
        edge_length(&stiff),
        edge_length(&weak)
    );
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