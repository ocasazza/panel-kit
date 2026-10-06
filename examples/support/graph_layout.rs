//! Host-side layout solving for the `graph` external component contract.
//!
//! One seam for both backends: a [`GraphLayout`] request plus a [`GraphModel`]
//! body in, one position per node out, in node declaration order. Every static
//! [`LayoutAlgorithm`] maps 1:1 onto a vendored `graph-layouts` solver;
//! [`LayoutAlgorithm::Force`] has no static solver, so it runs the
//! deterministic CPU force solve in [`force_solve`] once per connected
//! component; every surface draws that one arrangement.
//!
//! Parameters arrive as the spec's `params` map of `f64`. Scalar knobs map by
//! name (`rank_separation`, `level_spacing`, …); the vendored enums map by
//! their declaration order as an integer (`rank_direction`: 0 TB, 1 BT, 2 LR,
//! 3 RL). A parameter that is absent or not finite keeps the solver default.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use graph_layouts::{
    CircleAxis, CircleLayout, CircleSettings, ConcentricLayout, ConcentricMetric, ConcentricSettings,
    DagreLayout, DagreRanker, DagreSettings, Edge, FcoseLayout, FcoseQuality, FcoseSettings, Graph,
    GridLayout, GridSettings, Node, RankDirection, SpectralLayout, SpectralSettings, SphereLayout,
    SphereSettings, StaticLayout,
};
use panel_kit_core::widgets::graph::{GraphLayout, GraphModel, GraphNode, LayoutAlgorithm};

/// Radius of the deterministic seed sphere [`LayoutAlgorithm::Force`] starts
/// from when the caller supplies no live positions.
pub const FORCE_SEED_RADIUS: f32 = 200.0;

/// Iterations the CPU force solve runs before it returns.
const FORCE_ITERATIONS: f64 = 600.0;
/// First per-iteration displacement cap, in canonical layout lengths.
const FORCE_STEP: f64 = 0.12;
/// Fraction of the displacement cap left after each iteration (cooling).
const FORCE_COOLING: f64 = 0.995;
/// Fraction of a node's velocity carried into the next iteration.
const FORCE_DAMPING: f64 = 0.6;
/// Spring stiffness per unit of `weight`, per canonical layout length.
const FORCE_STIFFNESS: f64 = 1.0;
/// Repulsion per unit of the mean-normalised `repulsion` product.
const FORCE_REPULSION: f64 = 5.0;
/// Pull toward the centroid, as a fraction of the distance from it.
const FORCE_CENTER: f64 = 0.05;
/// Fixed-seed jitter that lifts a seed off its own plane (a line, a plane or a
/// point cannot relax into 3-D on its own), as a fraction of the seed's spread.
const FORCE_JITTER: f64 = 0.05;
/// Seed of that jitter: the solve stays a pure function of the model.
const FORCE_JITTER_SEED: u64 = 0x5EED_0F1E_D000_0001;
/// Smallest seed spread that still fixes a length scale.
const FORCE_MIN_SPREAD: f64 = 1.0e-3;
/// Closest approach two nodes may take, so the `1/d` terms stay finite.
const FORCE_MIN_DISTANCE: f64 = 1.0e-3;
/// Largest rest length, in canonical layout lengths, one edge may ask for.
const FORCE_MAX_REST: f64 = 4.0;
/// Canonical layout length: one `rest_length` of the pulled-back physics.
pub const FORCE_LENGTH: f64 = 200.0;
/// Lightest a node may be treated as, so inertia never divides by zero.
pub const FORCE_MIN_MASS: f64 = 0.05;
/// Side of the square cell a solved component is packed into, in multiples of
/// its own diameter: room enough that neighbouring shapes never touch.
const FORCE_PACK_MARGIN: f32 = 1.5;
/// Width over height the packed components aim for: a graph panel is wider
/// than it is tall.
const FORCE_PACK_ASPECT: f32 = 2.0;

// Solver graphs are cached per topology, so one topology always solves through
// one graph: the vendored `Graph` is `HashMap`-backed, `HashMap` hands each new
// map a fresh hasher, and the spectral solver sums a node's neighbours in that
// map's iteration order.
thread_local! {
    static GRAPHS: RefCell<HashMap<String, Arc<Graph>>> = RefCell::new(HashMap::new());
}

/// Topologies kept before the cache is dropped, so a long-lived host that keeps
/// re-solving a handful of graphs does not grow it without bound.
const GRAPH_CACHE_LIMIT: usize = 64;

/// Solve `model` under `layout`, one `[x, y, z]` per node in declaration order.
///
/// `seed` seeds the solvers that start from a known arrangement (the positions
/// on screen when switching topos along a morphism); a seed of the wrong
/// length falls back to [`fibonacci_sphere`]. Grouping partitions the graph
/// into per-sort sub-solves that are composed back together, see [`compose`];
/// `Force` partitions by connected component instead, see [`force_components`].
/// A solver that errors or returns a non-finite arrangement degrades to the
/// seed rather than painting a broken graph.
pub fn solve(
    model: &GraphModel,
    layout: &GraphLayout,
    seed: Option<&[[f32; 3]]>,
) -> Vec<[f32; 3]> {
    let count = model.nodes.len();
    if count == 0 {
        return Vec::new();
    }
    let seed_positions = match seed {
        Some(seed) if seed.len() == count => seed.to_vec(),
        _ => fibonacci_sphere(count, FORCE_SEED_RADIUS),
    };

    if layout.algorithm == LayoutAlgorithm::Force {
        return force_components(model, layout, &seed_positions);
    }

    let groups = groups(model, layout);
    if groups.len() <= 1 {
        let all: Vec<usize> = (0..count).collect();
        return solve_members(model, layout, &all, &seed_positions);
    }

    let mut out = vec![[0.0; 3]; count];
    for (index, members) in groups.iter().enumerate() {
        let solved = solve_members(model, layout, members, &seed_positions);
        compose(layout, index, &solved, &mut out, members);
    }
    out
}

/// Force layouts assemble per connected component: each component relaxes on
/// its own (so a closed surface becomes a shell, a tube a cylinder), is turned
/// by [`face_the_viewer`] so its flattest axis runs along z, and the solved
/// components are shelf-packed, widest first, into square cells sized
/// to each one by [`FORCE_PACK_MARGIN`], rows wrapping so the whole is about
/// [`FORCE_PACK_ASPECT`] times as wide as tall, centred on the seed. `group_by`
/// does not apply: splitting a component by sort would tear the structure the
/// physics is meant to reveal.
fn force_components(model: &GraphModel, layout: &GraphLayout, seed: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let mut pieces: Vec<(Vec<usize>, Vec<[f32; 3]>, f32)> = connected_components(model)
        .into_iter()
        .map(|members| {
            let points = face_the_viewer(&solve_members(model, layout, &members, seed));
            let radius = reach(&points);
            (members, points, radius)
        })
        .collect();
    pieces.sort_by(|a, b| b.2.total_cmp(&a.2));
    let widest = pieces.first().map_or(0.0, |piece| piece.2);
    let unit = if widest > 0.0 { widest } else { reach(seed).max(1.0) };
    let side = |radius: f32| 2.0 * FORCE_PACK_MARGIN * radius.max(0.25 * unit);
    let area: f32 = pieces.iter().map(|piece| side(piece.2).powi(2)).sum();
    let row_width = (area * FORCE_PACK_ASPECT).sqrt();

    let mut out = vec![[0.0; 3]; model.nodes.len()];
    let (mut x, mut top, mut row_height) = (0.0_f32, 0.0_f32, 0.0_f32);
    for (members, points, radius) in &pieces {
        let cell = side(*radius);
        if x > 0.0 && x + cell > row_width {
            (x, top, row_height) = (0.0, top - row_height, 0.0);
        }
        let origin = [x + 0.5 * cell, top - 0.5 * cell];
        let [cx, cy, cz] = centroid(points);
        for (slot, point) in members.iter().zip(points) {
            out[*slot] = [point[0] - cx + origin[0], point[1] - cy + origin[1], point[2] - cz];
        }
        x += cell;
        row_height = row_height.max(cell);
    }
    let packed = centroid(&out);
    let wanted = centroid(seed);
    for point in &mut out {
        for axis in 0..3 {
            point[axis] += wanted[axis] - packed[axis];
        }
    }
    out
}

/// Largest distance of any point from the points' centroid.
fn reach(points: &[[f32; 3]]) -> f32 {
    let [cx, cy, cz] = centroid(points);
    points
        .iter()
        .map(|p| ((p[0] - cx).powi(2) + (p[1] - cy).powi(2) + (p[2] - cz).powi(2)).sqrt())
        .fold(0.0_f32, f32::max)
}

/// Sweeps of the Jacobi eigen-solve in [`face_the_viewer`]; a 3×3 symmetric
/// matrix converges to machine precision well within this.
const JACOBI_SWEEPS: usize = 8;

/// `points` about their centroid, turned so their widest spread runs along x
/// and their flattest along z: a ring, sheet or chain then faces a camera
/// looking down z instead of standing edge-on to it. A proper rotation, never
/// a mirror, so a shape keeps its handedness.
fn face_the_viewer(points: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let centre = centroid(points).map(f64::from);
    let centred: Vec<[f64; 3]> = points
        .iter()
        .map(|point| [0, 1, 2].map(|axis| f64::from(point[axis]) - centre[axis]))
        .collect();
    let mut spread = [[0.0_f64; 3]; 3];
    for point in &centred {
        for row in 0..3 {
            for column in 0..3 {
                spread[row][column] += point[row] * point[column];
            }
        }
    }
    let (values, vectors) = symmetric_eigen(spread);
    let mut order = [0, 1, 2];
    order.sort_by(|&a, &b| values[b].total_cmp(&values[a]));
    let mut axes = order.map(|k| [vectors[0][k], vectors[1][k], vectors[2][k]]);
    let [a, b, c] = axes;
    let handedness = (a[1] * b[2] - a[2] * b[1]) * c[0] + (a[2] * b[0] - a[0] * b[2]) * c[1]
        + (a[0] * b[1] - a[1] * b[0]) * c[2];
    if handedness < 0.0 {
        axes[2] = c.map(|v| -v);
    }
    centred
        .iter()
        .map(|point| axes.map(|axis| (axis[0] * point[0] + axis[1] * point[1] + axis[2] * point[2]) as f32))
        .collect()
}

/// Eigenvalues and column eigenvectors of a symmetric 3×3 matrix, by cyclic
/// Jacobi rotations.
fn symmetric_eigen(mut matrix: [[f64; 3]; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let multiply = |lhs: [[f64; 3]; 3], rhs: [[f64; 3]; 3]| {
        let mut out = [[0.0_f64; 3]; 3];
        for row in 0..3 {
            for column in 0..3 {
                out[row][column] = (0..3).map(|k| lhs[row][k] * rhs[k][column]).sum();
            }
        }
        out
    };
    let mut vectors = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..JACOBI_SWEEPS {
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if matrix[p][q] == 0.0 {
                continue;
            }
            let angle = 0.5 * (2.0 * matrix[p][q]).atan2(matrix[q][q] - matrix[p][p]);
            let (sin, cos) = angle.sin_cos();
            let mut turn = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
            turn[p][p] = cos;
            turn[q][q] = cos;
            turn[p][q] = sin;
            turn[q][p] = -sin;
            let mut back = turn;
            (back[p][q], back[q][p]) = (turn[q][p], turn[p][q]);
            matrix = multiply(back, multiply(matrix, turn));
            vectors = multiply(vectors, turn);
        }
    }
    ([matrix[0][0], matrix[1][1], matrix[2][2]], vectors)
}

/// Node indices of each connected component over the model's edges, in
/// first-appearance order; isolated nodes are components of one.
fn connected_components(model: &GraphModel) -> Vec<Vec<usize>> {
    let count = model.nodes.len();
    let mut neighbours = vec![Vec::new(); count];
    for edge in &model.edges {
        let (a, b) = (edge.source as usize, edge.target as usize);
        if a < count && b < count && a != b {
            neighbours[a].push(b);
            neighbours[b].push(a);
        }
    }
    let mut seen = vec![false; count];
    let mut components = Vec::new();
    for start in 0..count {
        if seen[start] {
            continue;
        }
        let mut members = Vec::new();
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(node) = stack.pop() {
            members.push(node);
            for &next in &neighbours[node] {
                if !seen[next] {
                    seen[next] = true;
                    stack.push(next);
                }
            }
        }
        members.sort_unstable();
        components.push(members);
    }
    components
}

/// `count` points spread evenly over a sphere by the golden angle, in the same
/// order the GPU renderer spawns its bootstrap scene. Deterministic: no RNG.
pub fn fibonacci_sphere(count: usize, radius: f32) -> Vec<[f32; 3]> {
    let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
    let n = count as f32;
    (0..count)
        .map(|i| {
            let i_f = i as f32;
            let cos_phi = 1.0 - 2.0 * (i_f + 0.5) / n;
            let sin_phi = (1.0 - cos_phi * cos_phi).max(0.0).sqrt();
            let theta = golden * i_f;
            [
                sin_phi * theta.cos() * radius,
                sin_phi * theta.sin() * radius,
                cos_phi * radius,
            ]
        })
        .collect()
}

/// Project positions onto their two widest-spread axes and normalise each into
/// the unit square, for painters that have no third dimension. A planar layout
/// keeps its own axes: the flat axis has zero spread and is dropped.
pub fn project_2d(positions: &[[f32; 3]]) -> Vec<[f32; 2]> {
    if positions.is_empty() {
        return Vec::new();
    }
    if positions.len() == 1 {
        return vec![[0.5, 0.5]];
    }

    let mut spread = [0.0_f64; 3];
    for (axis, slot) in spread.iter_mut().enumerate() {
        let (lo, hi) = axis_range(positions, axis);
        *slot = hi - lo;
    }
    // Widest axis first; equal spreads resolve to the lower axis index.
    let mut order = [0_usize, 1, 2];
    order.sort_by(|&a, &b| spread[b].total_cmp(&spread[a]).then(a.cmp(&b)));

    let project = |axis: usize| {
        let (lo, hi) = axis_range(positions, axis);
        let span = hi - lo;
        positions
            .iter()
            .map(|point| {
                if span <= f64::EPSILON {
                    0.5
                } else {
                    ((f64::from(point[axis]) - lo) / span) as f32
                }
            })
            .collect::<Vec<_>>()
    };
    project(order[0])
        .into_iter()
        .zip(project(order[1]))
        .map(|(x, y)| [x, y])
        .collect()
}

/// Positions interpolated from `from` to `to` over `duration_ms` on a smooth
/// ease-in-out curve — the visible half of switching topos: the codomain's
/// layout is solved from the current positions, then the renderer walks there.
///
/// Reduced motion collapses the tween, and a `from` that does not match `to`
/// has nothing to interpolate, so both sample as `to` immediately.
#[derive(Clone, Debug, PartialEq)]
pub struct Tween {
    from: Vec<[f32; 3]>,
    to: Vec<[f32; 3]>,
    duration_ms: f32,
}

impl Tween {
    /// Start a tween. `reduced_motion` honours `prefers-reduced-motion` by
    /// making every sample the target.
    pub fn new(
        from: Vec<[f32; 3]>,
        to: Vec<[f32; 3]>,
        duration_ms: u32,
        reduced_motion: bool,
    ) -> Self {
        let from = if from.len() == to.len() && !to.is_empty() {
            from
        } else {
            to.clone()
        };
        Self {
            from,
            to,
            duration_ms: if reduced_motion { 0.0 } else { duration_ms as f32 },
        }
    }

    /// Positions `elapsed_ms` after the tween started.
    pub fn at(&self, elapsed_ms: u32) -> Vec<[f32; 3]> {
        let raw = if self.duration_ms <= 0.0 {
            1.0
        } else {
            (elapsed_ms as f32 / self.duration_ms).clamp(0.0, 1.0)
        };
        let eased = raw * raw * (3.0 - 2.0 * raw);
        self.from
            .iter()
            .zip(&self.to)
            .map(|(from, to)| {
                [
                    from[0] + (to[0] - from[0]) * eased,
                    from[1] + (to[1] - from[1]) * eased,
                    from[2] + (to[2] - from[2]) * eased,
                ]
            })
            .collect()
    }

    /// Whether `elapsed_ms` has run the tween out.
    pub fn is_complete(&self, elapsed_ms: u32) -> bool {
        self.duration_ms <= 0.0 || elapsed_ms as f32 >= self.duration_ms
    }
}

/// Node indices partitioned by the requested node field, in first-appearance
/// order. `sort`, `label` and `id` are node fields; anything else leaves the
/// graph whole rather than inventing a grouping.
fn groups<'a>(model: &'a GraphModel, layout: &GraphLayout) -> Vec<Vec<usize>> {
    let whole = || vec![(0..model.nodes.len()).collect::<Vec<_>>()];
    let Some(field) = layout.group_by.as_deref() else {
        return whole();
    };
    if !matches!(field, "sort" | "label" | "id") {
        return whole();
    }
    let key = |node: &'a GraphNode| -> &'a str {
        match field {
            "sort" => &node.sort,
            "label" => &node.label,
            _ => &node.id,
        }
    };

    let mut order: Vec<&str> = Vec::new();
    let mut buckets: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, node) in model.nodes.iter().enumerate() {
        let sort = key(node);
        if !buckets.contains_key(sort) {
            order.push(sort);
        }
        buckets.entry(sort).or_default().push(index);
    }
    order
        .into_iter()
        .filter_map(|sort| buckets.remove(sort))
        .collect()
}

/// Solve one group's induced subgraph, returning positions in the group's own
/// declaration order.
fn solve_members(
    model: &GraphModel,
    layout: &GraphLayout,
    members: &[usize],
    seed: &[[f32; 3]],
) -> Vec<[f32; 3]> {
    let fallback: Vec<[f32; 3]> = members.iter().map(|&index| seed[index]).collect();
    if layout.algorithm == LayoutAlgorithm::Force {
        return force_solve(model, members, seed, &layout.params);
    }

    let (graph, order) = solver_graph(model, members);
    let Some(packed) = run_solver(layout, &graph) else {
        return fallback;
    };
    if packed.len() != members.len() * 3 {
        return fallback;
    }

    let mut out = vec![[0.0; 3]; members.len()];
    for (index, &[x, y, z]) in packed.as_chunks::<3>().0.iter().enumerate() {
        out[order[index]] = [x, y, z];
    }
    if out.iter().all(|point| point.iter().all(|v| v.is_finite())) {
        out
    } else {
        fallback
    }
}

/// Positions for `LayoutAlgorithm::Force`: a deterministic 3-D force solve
/// over one connected component.
///
/// Each of `FORCE_ITERATIONS` iterations sums, per node, the repulsion every
/// other node pushes on it (Fruchterman-Reingold `k²/d`, scaled by each node's
/// `repulsion` and normalised to unit mean), the springs of its edges (`weight`
/// times the overshoot of that edge's rest length), and a pull toward the
/// seed's centroid; `mass` is the inertia the sum is divided by. Damping and a
/// cooling displacement cap settle it. The forces are all in units of
/// [`FORCE_LENGTH`], the canonical layout length, so the arrangement a solve
/// settles at does not depend on the scale of the seed it started from.
///
/// The seed is the starting arrangement only, and comes back at its own centre
/// and spread: the seed owns the scale, the forces own the shape, so a re-solve
/// seeded from a solved layout lands on the same shape. There is no RNG — the
/// fixed-seed jitter that lifts a line, plane or point seed off its own plane is
/// the only source of irregularity.
///
/// A seed that is not finite or has collapsed to a point returns it unchanged,
/// and a result that is not finite falls back to the seed, so the caller never
/// paints a broken graph.
fn force_solve(
    model: &GraphModel,
    members: &[usize],
    seed: &[[f32; 3]],
    params: &BTreeMap<String, f64>,
) -> Vec<[f32; 3]> {
    let count = members.len();
    let fallback: Vec<[f32; 3]> = members.iter().map(|&index| seed[index]).collect();
    let mut points: Vec<[f64; 3]> = members
        .iter()
        .map(|&index| seed[index].map(f64::from))
        .collect();
    if count < 2 || points.iter().flatten().any(|value| !value.is_finite()) {
        return fallback;
    }

    let mut centre = [0.0_f64; 3];
    for point in &points {
        for axis in 0..3 {
            centre[axis] += point[axis];
        }
    }
    for axis in centre.iter_mut() {
        *axis /= count as f64;
    }
    let spread = points
        .iter()
        .map(|point| distance(point, &centre))
        .sum::<f64>()
        / count as f64;
    if spread <= FORCE_MIN_SPREAD {
        return fallback;
    }
    // The solve runs centred on the seed; the forces below are in absolute
    // [`FORCE_LENGTH`] units, so the shape it settles at is a property of the
    // model and not of the scale the caller happened to seed with.
    for point in &mut points {
        for axis in 0..3 {
            point[axis] -= centre[axis];
        }
    }

    let declared: f64 = members
        .iter()
        .map(|&index| model.nodes[index].repulsion)
        .filter(|repulsion| repulsion.is_finite() && *repulsion > 0.0)
        .sum();
    let unit = if declared > 0.0 {
        declared / count as f64
    } else {
        1.0
    };
    let repulsion: Vec<f64> = members
        .iter()
        .map(|&index| {
            let value = model.nodes[index].repulsion;
            if value.is_finite() && value > 0.0 {
                value / unit
            } else {
                1.0
            }
        })
        .collect();
    let mass: Vec<f64> = members
        .iter()
        .map(|&index| {
            let value = model.nodes[index].mass;
            if value.is_finite() {
                value.max(FORCE_MIN_MASS)
            } else {
                1.0
            }
        })
        .collect();
    let springs: Vec<(usize, usize, f64, f64)> = solver_links(model, members)
        .into_iter()
        .filter(|link| link.source != link.target)
        .map(|link| {
            let edge = &model.edges[link.edge];
            // Stiffness is per unit length, matching the repulsion it balances.
            let weight = if edge.weight.is_finite() {
                FORCE_STIFFNESS * edge.weight.max(0.0) / FORCE_LENGTH
            } else {
                FORCE_STIFFNESS / FORCE_LENGTH
            };
            // `rest_length` is dimensionless: it counts canonical layout lengths.
            let rest = if edge.rest_length.is_finite() {
                edge.rest_length.clamp(0.0, FORCE_MAX_REST) * FORCE_LENGTH
            } else {
                FORCE_LENGTH
            };
            (link.source, link.target, weight, rest)
        })
        .collect();

    jitter(&mut points, spread);
    let iterations = number(params, "iterations", FORCE_ITERATIONS).clamp(1.0, 4000.0) as usize;
    let cooling = number(params, "cooling", FORCE_COOLING).clamp(0.5, 0.999);
    let damping = number(params, "damping", FORCE_DAMPING).clamp(0.0, 1.0);
    // The displacement cap cools across the solve, so it starts wide enough to
    // cover the distance the longest rest length asks for.
    let longest = springs
        .iter()
        .map(|&(_, _, _, rest)| rest)
        .fold(spread, f64::max);
    let mut cap = number(params, "step", FORCE_STEP) * longest;
    let mut velocity = vec![[0.0_f64; 3]; count];
    let mut force = vec![[0.0_f64; 3]; count];
    for _ in 0..iterations {
        force.fill([0.0; 3]);
        for i in 0..count {
            for j in i + 1..count {
                let away = sub(points[j], points[i]);
                let length = norm(away);
                if length <= FORCE_MIN_DISTANCE {
                    continue;
                }
                // Fruchterman-Reingold repulsion, `k^2/d`, along the line joining
                // the pair.
                let push = FORCE_REPULSION * repulsion[i] * repulsion[j] * FORCE_LENGTH / length;
                for axis in 0..3 {
                    let share = push * away[axis] / length;
                    force[i][axis] -= share;
                    force[j][axis] += share;
                }
            }
        }
        for &(source, target, weight, rest) in &springs {
            let along = sub(points[target], points[source]);
            let length = norm(along);
            if length <= FORCE_MIN_DISTANCE {
                continue;
            }
            // Hooke spring along the edge: attraction past `rest`, repulsion short of it.
            let pull = weight * (length - rest);
            for axis in 0..3 {
                let share = pull * along[axis] / length;
                force[source][axis] += share;
                force[target][axis] -= share;
            }
        }
        for i in 0..count {
            for axis in 0..3 {
                let step = (velocity[i][axis] + force[i][axis] / mass[i]
                    - FORCE_CENTER * points[i][axis] / FORCE_LENGTH)
                    * damping;
                let step = step.clamp(-cap, cap);
                velocity[i][axis] = step;
                points[i][axis] += step;
            }
        }
        cap *= cooling;
    }

    // The seed owns the scale: the solved shape comes back at the seed's own
    // spread, so a topos switch keeps the layout the size it already was.
    let settled = points
        .iter()
        .map(|point| norm(*point))
        .sum::<f64>()
        / count as f64;
    let scale = if settled > FORCE_MIN_SPREAD {
        spread / settled
    } else {
        1.0
    };
    let solved: Vec<[f32; 3]> = points
        .iter()
        .map(|point| {
            [
                (point[0] * scale + centre[0]) as f32,
                (point[1] * scale + centre[1]) as f32,
                (point[2] * scale + centre[2]) as f32,
            ]
        })
        .collect();
    if solved.iter().all(|point| point.iter().all(|value| value.is_finite())) {
        solved
    } else {
        fallback
    }
}

/// Nudge every node off its seed position by a fixed-seed jitter, scaled to the
/// seed's spread: a seed that is a line, a plane or a point has no out-of-plane
/// direction to relax along.
fn jitter(points: &mut [[f64; 3]], spread: f64) {
    let mut state = FORCE_JITTER_SEED;
    let amplitude = FORCE_JITTER * spread;
    for point in points.iter_mut() {
        for axis in point.iter_mut() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            *axis += amplitude * ((state >> 11) as f64 / (1_u64 << 53) as f64 - 0.5);
        }
    }
}

fn sub(lhs: [f64; 3], rhs: [f64; 3]) -> [f64; 3] {
    [lhs[0] - rhs[0], lhs[1] - rhs[1], lhs[2] - rhs[2]]
}

fn norm(vector: [f64; 3]) -> f64 {
    (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt()
}

fn distance(lhs: &[f64; 3], rhs: &[f64; 3]) -> f64 {
    norm([lhs[0] - rhs[0], lhs[1] - rhs[1], lhs[2] - rhs[2]])
}

/// Build the vendored solver's graph over `members` and return the permutation
/// its id-sorted node order imposes: `order[i]` is the slot within `members`
/// whose node the solvers place at position `i`.
///
/// Node ids are the model's own, disambiguated with the declaration index when
/// a model repeats one, so the map keys stay one-to-one. Edges outside the
/// group (or outside the model) drop out; each carries the model's weight and
/// morphism kind.
fn solver_graph(model: &GraphModel, members: &[usize]) -> (Arc<Graph>, Vec<usize>) {
    let ids = solver_ids(model, members);
    let links = solver_links(model, members);

    let mut fingerprint = ids.join("\u{1}");
    let mut endpoints: Vec<String> = links
        .iter()
        .map(|link| format!("{}>{}", link.source, link.target))
        .collect();
    endpoints.sort_unstable();
    fingerprint.push('\u{2}');
    fingerprint.push_str(&endpoints.join("\u{1}"));

    let graph = GRAPHS.with(|cache| {
        let mut cached = cache.borrow_mut();
        if cached.len() >= GRAPH_CACHE_LIMIT {
            cached.clear();
        }
        cached
            .entry(fingerprint)
            .or_insert_with(|| Arc::new(build_graph(model, members, &ids, &links)))
            .clone()
    });

    let mut order: Vec<(String, usize)> = ids.into_iter().enumerate().map(|(s, id)| (id, s)).collect();
    order.sort_by(|a, b| a.0.cmp(&b.0));
    (graph, order.into_iter().map(|(_, slot)| slot).collect())
}

/// One model edge that stays inside a group, addressed by group slot.
struct SolverLink {
    source: usize,
    target: usize,
    edge: usize,
}

/// The model's edges whose endpoints are both in `members`, in model order.
fn solver_links(model: &GraphModel, members: &[usize]) -> Vec<SolverLink> {
    let slot: BTreeMap<usize, usize> = members
        .iter()
        .enumerate()
        .map(|(slot, &index)| (index, slot))
        .collect();
    model
        .edges
        .iter()
        .enumerate()
        .filter_map(|(edge, model_edge)| {
            let source = slot.get(&(model_edge.source as usize)).copied()?;
            let target = slot.get(&(model_edge.target as usize)).copied()?;
            Some(SolverLink {
                source,
                target,
                edge,
            })
        })
        .collect()
}

/// Materialise one solver graph. Node positions are left unset: none of the
/// eight static solvers reads them, and leaving them out is what lets one
/// topology map to one graph (see [`GRAPHS`]).
fn build_graph(
    model: &GraphModel,
    members: &[usize],
    ids: &[String],
    links: &[SolverLink],
) -> Graph {
    let mut nodes = BTreeMap::new();
    for (id, index) in ids.iter().zip(members) {
        let node = &model.nodes[*index];
        let mut built = Node::new(id.clone());
        built.label = node.label.clone();
        built.r#type = node.sort.clone();
        nodes.insert(id.clone(), built);
    }

    let mut edges = BTreeMap::new();
    for link in links {
        let model_edge = &model.edges[link.edge];
        let id = format!("e{:08}", link.edge);
        let mut built = Edge::new(id.clone(), ids[link.source].clone(), ids[link.target].clone());
        built.weight = model_edge.weight;
        built.r#type = model_edge.kind.clone();
        edges.insert(id, built);
    }

    Graph {
        nodes: nodes.into_iter().collect(),
        edges: edges.into_iter().collect(),
    }
}

/// Unique solver ids for `members`: the model's own, disambiguated with the
/// declaration index when a model repeats one.
fn solver_ids(model: &GraphModel, members: &[usize]) -> Vec<String> {
    let mut used: BTreeSet<String> = BTreeSet::new();
    members
        .iter()
        .map(|&index| {
            let base = &model.nodes[index].id;
            let id = if used.insert(base.clone()) {
                base.clone()
            } else {
                let unique = format!("{base}#{index:08}");
                used.insert(unique.clone());
                unique
            };
            id
        })
        .collect()
}

/// Run the vendored solver `layout` names, with its parameters applied.
fn run_solver(layout: &GraphLayout, graph: &Graph) -> Option<Vec<f32>> {
    let params = &layout.params;
    let solved = match layout.algorithm {
        LayoutAlgorithm::Force => return None,
        LayoutAlgorithm::Circle => CircleLayout::solve(
            &CircleSettings {
                radius: number(params, "radius", 200.0) as f32,
                axis: variant(&[CircleAxis::X, CircleAxis::Y, CircleAxis::Z], params, "axis", 2),
            },
            graph,
        ),
        LayoutAlgorithm::Grid => GridLayout::solve(
            &GridSettings {
                spacing: number(params, "spacing", 50.0) as f32,
                aspect: number(params, "aspect", 1.0) as f32,
                layers: number(params, "layers", 1.0).max(1.0) as u32,
                center: flag(params, "center", true),
            },
            graph,
        ),
        LayoutAlgorithm::Concentric => ConcentricLayout::solve(
            &ConcentricSettings {
                metric: variant(
                    &[
                        ConcentricMetric::Degree,
                        ConcentricMetric::InDegree,
                        ConcentricMetric::OutDegree,
                        ConcentricMetric::Alphabetical,
                    ],
                    params,
                    "metric",
                    0,
                ),
                min_radius: number(params, "min_radius", 50.0) as f32,
                level_spacing: number(params, "level_spacing", 80.0) as f32,
                clockwise: flag(params, "clockwise", true),
                bucket_count: number(params, "bucket_count", 0.0).max(0.0) as u32,
            },
            graph,
        ),
        LayoutAlgorithm::Spectral => SpectralLayout::solve(
            &SpectralSettings {
                radius: number(params, "radius", 300.0) as f32,
                iterations: number(params, "iterations", 200.0).max(1.0) as u32,
                three_d: flag(params, "three_d", true),
            },
            graph,
        ),
        LayoutAlgorithm::Dagre => DagreLayout::solve(
            &DagreSettings {
                rank_direction: variant(
                    &[
                        RankDirection::TB,
                        RankDirection::BT,
                        RankDirection::LR,
                        RankDirection::RL,
                    ],
                    params,
                    "rank_direction",
                    0,
                ),
                ranker: variant(
                    &[
                        DagreRanker::NetworkSimplex,
                        DagreRanker::TightTree,
                        DagreRanker::LongestPath,
                    ],
                    params,
                    "ranker",
                    0,
                ),
                rank_separation: number(params, "rank_separation", 50.0),
                node_separation: number(params, "node_separation", 50.0),
                acyclic: flag(params, "acyclic", true),
            },
            graph,
        ),
        LayoutAlgorithm::Fcose => FcoseLayout::solve(
            &FcoseSettings {
                node_repulsion: number(params, "node_repulsion", 4500.0),
                ideal_edge_length: number(params, "ideal_edge_length", 50.0),
                node_overlap: number(params, "node_overlap", 10.0),
                quality: variant(
                    &[FcoseQuality::Draft, FcoseQuality::Default, FcoseQuality::Proof],
                    params,
                    "quality",
                    1,
                ),
                seed: number(params, "seed", 0x5EED_F0CE_u64 as f64) as u64,
            },
            graph,
        ),
        LayoutAlgorithm::Sphere => SphereLayout::solve(
            &SphereSettings {
                radius: number(params, "radius", 200.0) as f32,
                jitter: number(params, "jitter", 0.0) as f32,
                seed: number(params, "seed", 0xC0FFEE_u64 as f64) as u64,
            },
            graph,
        ),
    };
    solved.ok()
}

/// Place one group's sub-solve inside the whole. Concentric layouts give each
/// sort its own ring, `dagre` gives each sort its own rank band along the
/// direction it grows ranks, and the rest line the sorts up in a row.
fn compose(
    layout: &GraphLayout,
    group: usize,
    solved: &[[f32; 3]],
    out: &mut [[f32; 3]],
    members: &[usize],
) {
    let [cx, cy, cz] = centroid(solved);
    match layout.algorithm {
        LayoutAlgorithm::Concentric | LayoutAlgorithm::Circle | LayoutAlgorithm::Sphere => {
            let spacing = number(&layout.params, "level_spacing", 80.0).max(1.0) as f32;
            let min_radius = number(&layout.params, "min_radius", 50.0).max(0.0) as f32;
            let radius = min_radius + spacing * group as f32;
            let reach = solved
                .iter()
                .map(|point| (point[0] - cx).hypot(point[1] - cy))
                .fold(0.0_f32, f32::max);
            let scale = if reach > 0.0 { radius / reach } else { 0.0 };
            for (slot, point) in members.iter().zip(solved) {
                out[*slot] = [
                    (point[0] - cx) * scale,
                    (point[1] - cy) * scale,
                    point[2] - cz,
                ];
            }
        }
        LayoutAlgorithm::Dagre => {
            let separation = number(&layout.params, "rank_separation", 50.0) as f32;
            let (axis, sign) = rank_axis(&layout.params);
            for (slot, point) in members.iter().zip(solved) {
                let mut placed = [point[0] - cx, point[1] - cy, point[2] - cz];
                placed[axis] += sign * separation * group as f32;
                out[*slot] = placed;
            }
        }
        _ => {
            let span = number(&layout.params, "group_span", 600.0).max(1.0) as f32;
            for (slot, point) in members.iter().zip(solved) {
                out[*slot] = [
                    point[0] - cx + span * group as f32,
                    point[1] - cy,
                    point[2] - cz,
                ];
            }
        }
    }
}

/// The axis `dagre` grows ranks along and the direction, from `rank_direction`.
fn rank_axis(params: &BTreeMap<String, f64>) -> (usize, f32) {
    match number(params, "rank_direction", 0.0).max(0.0) as usize % 4 {
        1 => (1, -1.0),
        2 => (0, 1.0),
        3 => (0, -1.0),
        _ => (1, 1.0),
    }
}

fn centroid(points: &[[f32; 3]]) -> [f32; 3] {
    if points.is_empty() {
        return [0.0; 3];
    }
    let mut sum = [0.0_f64; 3];
    for point in points {
        for axis in 0..3 {
            sum[axis] += f64::from(point[axis]);
        }
    }
    let count = points.len() as f32;
    [sum[0] as f32 / count, sum[1] as f32 / count, sum[2] as f32 / count]
}

fn axis_range(positions: &[[f32; 3]], axis: usize) -> (f64, f64) {
    positions.iter().fold((f64::MAX, f64::MIN), |(lo, hi), point| {
        let value = f64::from(point[axis]);
        (lo.min(value), hi.max(value))
    })
}

fn number(params: &BTreeMap<String, f64>, key: &str, default: f64) -> f64 {
    params.get(key).copied().filter(|v| v.is_finite()).unwrap_or(default)
}

fn flag(params: &BTreeMap<String, f64>, key: &str, default: bool) -> bool {
    number(params, key, f64::from(default)) >= 0.5
}

/// One of a solver's enum settings, chosen by its declaration order.
fn variant<T: Copy>(
    variants: &[T],
    params: &BTreeMap<String, f64>,
    key: &str,
    default: usize,
) -> T {
    let index = number(params, key, default as f64).max(0.0) as usize;
    variants[index % variants.len()]
}