//! The vertical rail's suite: `graph_test.exs` "vertical", the legend tone table,
//! the board's rail assertions, and the algorithm's own edge cases.

use std::collections::HashSet;

use proptest::prelude::*;
use rstest::rstest;

use super::super::model::Edge;
use super::super::time::from_utc;
use super::*;

// -- fixtures -----------------------------------------------------------------

const MIN: Micros = 60_000_000;

fn since() -> Micros {
    from_utc(2026, 10, 1, 0, 0, 0)
}

fn at(minute: i64) -> Micros {
    since() + minute * MIN
}

/// `vev`: a note on `imp-16` by `agent` at `minute`, summary `"<id> said"`.
fn vev(id: &str, minute: i64, agent: &str) -> Event {
    Event::new(id, at(minute), EventKind::Note)
        .agent(agent)
        .item("imp-16")
        .summary(format!("{id} said"))
}

fn vlane(id: &str, from: i64, to: i64, agent: Option<&str>, kind: LaneKind, label: &str) -> Lane {
    Lane {
        id: id.to_owned(),
        item_id: "imp-16".to_owned(),
        kind,
        label: label.to_owned(),
        from: at(from),
        to: at(to),
        chosen: None,
        parent: None,
        agent: agent.map(str::to_owned),
    }
}

fn collab(from: i64, to: i64, agent: &str) -> Lane {
    vlane(
        &format!("imp-16/collab/{agent}"),
        from,
        to,
        Some(agent),
        LaneKind::Collab,
        agent,
    )
}

fn edge(kind: EdgeKind, lane: &str, minute: i64) -> Edge {
    Edge {
        kind,
        lane: lane.to_owned(),
        item_id: "imp-16".to_owned(),
        at: at(minute),
    }
}

fn fork_at(lane: &str, minute: i64) -> Edge {
    edge(EdgeKind::Fork, lane, minute)
}

fn merge_at(lane: &str, minute: i64) -> Edge {
    edge(EdgeKind::Merge, lane, minute)
}

fn vtree(lanes: Vec<Lane>, edges: Vec<Edge>) -> SessionTree {
    SessionTree {
        agent: Some("clara".to_owned()),
        lanes,
        edges,
    }
}

fn vgraph(events: &[Event], tree: &SessionTree) -> VerticalGraph {
    vertical(
        events,
        &ItemRef {
            id: "imp-16".to_owned(),
            status: Some("claimed".to_owned()),
        },
        tree,
    )
}

fn row_ids(g: &VerticalGraph) -> Vec<&str> {
    g.item.rows.iter().map(|r| r.id.as_str()).collect()
}

fn cols(g: &VerticalGraph) -> Vec<u32> {
    g.item.rows.iter().map(|r| r.col).collect()
}

// -- 10.1 vertical ------------------------------------------------------------

#[test]
fn one_row_per_event_in_time_order_other_agents_sit_in_their_lane_column() {
    let events = [
        vev("e3", 30, "clara"),
        vev("e1", 0, "clara"),
        vev("e2", 10, "omp"),
        vev("x", 5, "clara").item("imp-99"),
    ];
    let tree = vtree(
        vec![collab(10, 10, "omp")],
        vec![
            fork_at("imp-16/collab/omp", 10),
            merge_at("imp-16/collab/omp", 10),
        ],
    );
    let g = vgraph(&events, &tree);
    assert_eq!(row_ids(&g), ["e1", "e2", "e3"]);
    assert_eq!(cols(&g), [0, 1, 0]);
    assert_eq!(g.item.agent.as_deref(), Some("clara"));
    let ys: Vec<i32> = g.item.rows.iter().map(|r| r.y).collect();
    assert!(ys.windows(2).all(|w| w[1] - w[0] == 22));
}

#[test]
fn first_row_in_a_lane_names_the_lane_only_the_first() {
    let events = [
        vev("e1", 0, "clara"),
        vev("e2", 10, "omp"),
        vev("e3", 20, "omp"),
        vev("e4", 30, "clara"),
    ];
    let g = vgraph(&events, &vtree(vec![collab(10, 20, "omp")], vec![]));
    let names: Vec<Option<&str>> = g.item.rows.iter().map(|r| r.lane.as_deref()).collect();
    assert_eq!(names, [None, Some("omp"), None, None]);
    assert_eq!(g.item.rows[1].chip().as_deref(), Some("[OMP]"));
    assert_eq!(g.item.rows[2].chip(), None);
}

#[test]
fn a_lane_with_no_event_of_its_own_draws_one_unselectable_row() {
    let events = [vev("e1", 0, "clara"), vev("e2", 30, "clara")];
    let tree = vtree(
        vec![vlane(
            "imp-16/branch/feat",
            10,
            20,
            None,
            LaneKind::Branch,
            "feat/x",
        )],
        vec![],
    );
    let g = vgraph(&events, &tree);
    let markers: Vec<&Row> = g
        .item
        .rows
        .iter()
        .filter(|r| r.kind == RowKind::Lane)
        .collect();
    assert_eq!(markers.len(), 1);
    assert!(!markers[0].selectable);
    assert_eq!(markers[0].label, "branch feat/x");
    assert_eq!(markers[0].title, "branch feat/x");
    assert_eq!(markers[0].id, "lane:imp-16/branch/feat");
    assert_eq!(markers[0].tone, Tone::Hollow);
    assert_eq!(markers[0].detail, None);
    assert!(g.find_dot(&markers[0].id).is_none());
    assert_eq!(g.find_dot("e1").map(|r| r.id.as_str()), Some("e1"));
    assert_eq!(row_ids(&g), ["e1", "lane:imp-16/branch/feat", "e2"]);
}

#[test]
fn the_marker_row_prints_its_own_text_so_it_gets_no_chip() {
    let events = [vev("e1", 0, "clara")];
    let tree = vtree(
        vec![vlane("b", 10, 20, None, LaneKind::Branch, "feat/x")],
        vec![],
    );
    let g = vgraph(&events, &tree);
    assert!(g.item.rows.iter().all(|r| r.lane.is_none()));
}

#[test]
fn fork_elbow_leaves_the_item_line_merge_returns_open_lane_has_no_merge() {
    let events = [
        vev("e1", 0, "clara"),
        vev("e2", 10, "omp"),
        vev("e3", 20, "clara"),
        vev("e4", 30, "bob"),
    ];
    let tree = vtree(
        vec![collab(10, 10, "omp"), collab(30, 30, "bob")],
        vec![
            fork_at("imp-16/collab/omp", 10),
            merge_at("imp-16/collab/omp", 10),
            fork_at("imp-16/collab/bob", 30),
        ],
    );
    let g = vgraph(&events, &tree);
    let merged = g.item.forks.iter().find(|f| f.id == "imp-16/collab/omp");
    let bob = g.item.forks.iter().find(|f| f.id == "imp-16/collab/bob");
    assert!(g.item.x < merged.map_or(0, |f| f.x));
    let kinds: Vec<EdgeKind> = g.item.edges.iter().map(|e| e.kind).collect();
    assert_eq!(kinds.iter().filter(|k| **k == EdgeKind::Fork).count(), 2);
    assert_eq!(kinds.iter().filter(|k| **k == EdgeKind::Merge).count(), 1);
    assert!(g.item.edges[0].d.starts_with("M7.0 "));
    assert!(bob.is_some_and(|f| f.open));
    assert!(merged.is_some_and(|f| !f.open));
}

#[test]
fn rows_crossing_midnight_stamp_the_day() {
    let same_day = [vev("a", 0, "clara"), vev("b", 60, "clara")];
    assert_eq!(vgraph(&same_day, &vtree(vec![], vec![])).stamp, Stamp::Time);
    let across = [vev("a", 0, "clara"), vev("b", 30 * 60, "clara")];
    assert_eq!(
        vgraph(&across, &vtree(vec![], vec![])).stamp,
        Stamp::DayTime
    );
}

#[test]
fn an_item_with_an_owner_and_no_events_is_still_a_graph_with_no_rows() {
    let g = vertical(
        &[],
        &ItemRef {
            id: "imp-16".into(),
            status: Some("open".into()),
        },
        &vtree(vec![], vec![]),
    );
    assert!(g.item.rows.is_empty());
    assert_eq!(g.item.agent.as_deref(), Some("clara"));
    let view = g.view(&RailOptions::default());
    assert_eq!(view.empty, Some("nothing recorded yet"));
    assert!(view.canvas.is_none());
    assert_eq!(
        view.heading,
        Some(RailHeading {
            title: "session tree".into(),
            meta: None
        })
    );
    assert_eq!((g.height, g.gutter, g.stamp), (0, 14, Stamp::Time));
    assert_eq!((g.item.y0, g.item.y1), (None, None));
}

#[test]
fn rows_are_buttons_only_when_selectable_exactly_the_selected_one_is_marked() {
    let events = [vev("e1", 0, "clara"), vev("e2", 10, "clara")];
    let g = vgraph(&events, &vtree(vec![], vec![]));

    let plain = g.view(&RailOptions {
        dom_id: "st".into(),
        heading: false,
        ..RailOptions::default()
    });
    let canvas = plain.canvas.as_ref().expect("rows");
    assert!(canvas
        .rows
        .iter()
        .all(|r| r.hit == Hit::Static && !r.selected));
    assert_eq!(plain.heading, None);

    let live = g.view(&RailOptions {
        dom_id: "st".into(),
        on_select: true,
        selected: Some("e2".into()),
        ..RailOptions::default()
    });
    let canvas = live.canvas.as_ref().expect("rows");
    assert_eq!(
        canvas.rows.iter().filter(|r| r.hit == Hit::Button).count(),
        2
    );
    assert_eq!(canvas.rows.iter().filter(|r| r.selected).count(), 1);
    let e2 = canvas.rows.iter().find(|r| r.dom_id == "st-row-e2");
    assert!(e2.is_some_and(|r| r.selected));
}

#[test]
fn a_marker_row_is_a_button_once_interactive_but_is_never_selected() {
    let tree = vtree(
        vec![vlane("b", 10, 20, None, LaneKind::Branch, "x")],
        vec![],
    );
    let g = vgraph(&[vev("e1", 0, "clara")], &tree);
    let view = g.view(&RailOptions {
        on_select: true,
        selected: Some("lane:b".into()),
        ..RailOptions::default()
    });
    let marker = view
        .canvas
        .as_ref()
        .and_then(|c| c.rows.iter().find(|r| r.lane_marker));
    assert!(marker.is_some_and(|r| r.hit == Hit::Button && !r.selected));
    let plain = g.view(&RailOptions::default());
    let marker = plain
        .canvas
        .as_ref()
        .and_then(|c| c.rows.iter().find(|r| r.lane_marker));
    assert!(marker.is_some_and(|r| r.hit == Hit::Static && !r.selected));
}

// -- selection ----------------------------------------------------------------

#[test]
fn select_node_toggles_ignores_unknown_ids_and_switches() {
    let events = [vev("e1", 0, "clara"), vev("e2", 10, "clara")];
    let tree = vtree(vec![vlane("b", 5, 6, None, LaneKind::Branch, "x")], vec![]);
    let g = vgraph(&events, &tree);
    assert_eq!(g.select_node(None, "e1").as_deref(), Some("e1"));
    assert_eq!(g.select_node(Some("e1"), "e1"), None);
    assert_eq!(g.select_node(Some("e1"), "e2").as_deref(), Some("e2"));
    assert_eq!(g.select_node(Some("e1"), "nope").as_deref(), Some("e1"));
    assert_eq!(g.select_node(None, "nope"), None);
    assert_eq!(g.select_node(None, "lane:b"), None);
}

// -- header text --------------------------------------------------------------

#[rstest]
#[case(7, 6, "7 events · 6 lanes")]
#[case(1, 0, "1 event")]
#[case(3, 0, "3 events")]
#[case(0, 0, "0 events")]
#[case(1, 1, "1 event · 1 lane")]
#[case(2, 1, "2 events · 1 lane")]
fn header_texts(#[case] events: usize, #[case] lanes: usize, #[case] want: &str) {
    assert_eq!(header_text(events, lanes), want);
}

#[test]
fn header_counts_selectable_rows_and_every_lane_including_marker_only_ones() {
    let events = [
        vev("e1", 0, "clara"),
        vev("e2", 10, "omp"),
        vev("e3", 40, "clara"),
    ];
    let tree = vtree(
        vec![
            collab(10, 10, "omp"),
            vlane("b", 20, 30, None, LaneKind::Branch, "feat"),
        ],
        vec![],
    );
    let g = vgraph(&events, &tree);
    assert_eq!(g.item.rows.len(), 4);
    assert_eq!(g.header(), "3 events · 2 lanes");
    let view = g.view(&RailOptions::default());
    assert_eq!(
        view.heading.and_then(|h| h.meta).as_deref(),
        Some("3 events · 2 lanes")
    );
}

// -- lane packing -------------------------------------------------------------

fn lane_cols(g: &VerticalGraph) -> Vec<(String, u32)> {
    g.item.forks.iter().map(|f| (f.id.clone(), f.col)).collect()
}

#[test]
fn overlapping_lanes_take_columns_one_and_two_disjoint_ones_share_column_one() {
    let overlap = vtree(vec![collab(0, 20, "a"), collab(10, 30, "b")], vec![]);
    let g = vgraph(&[], &overlap);
    assert_eq!(
        lane_cols(&g),
        [("imp-16/collab/a".into(), 1), ("imp-16/collab/b".into(), 2)]
    );
    assert_eq!(g.gutter, col_x(2) + 7);

    let disjoint = vtree(vec![collab(0, 10, "a"), collab(20, 30, "b")], vec![]);
    let g = vgraph(&[], &disjoint);
    assert_eq!(
        lane_cols(&g),
        [("imp-16/collab/a".into(), 1), ("imp-16/collab/b".into(), 1)]
    );
    assert_eq!(g.gutter, col_x(1) + 7);
}

#[test]
fn touching_spans_overlap_because_the_test_is_inclusive() {
    let tree = vtree(vec![collab(0, 10, "a"), collab(10, 20, "b")], vec![]);
    let g = vgraph(&[], &tree);
    assert_eq!(lane_cols(&g)[1].1, 2);
}

#[test]
fn packing_is_independent_of_input_order() {
    let lanes = vec![collab(10, 30, "b"), collab(0, 20, "a"), collab(40, 50, "c")];
    let mut reversed = lanes.clone();
    reversed.reverse();
    let a = vgraph(&[], &vtree(lanes, vec![]));
    let b = vgraph(&[], &vtree(reversed, vec![]));
    assert_eq!(a, b);
}

#[test]
fn equal_starts_order_by_id() {
    let tree = vtree(vec![collab(0, 10, "z"), collab(0, 10, "a")], vec![]);
    let g = vgraph(&[], &tree);
    assert_eq!(
        lane_cols(&g),
        [("imp-16/collab/a".into(), 1), ("imp-16/collab/z".into(), 2)]
    );
}

#[test]
fn a_merge_edge_ends_the_span_used_for_packing_but_not_for_membership() {
    // a: 0..100 merged at 10, so b (20..30) fits column 1 after it.
    let tree = vtree(
        vec![collab(0, 100, "a"), collab(20, 30, "b")],
        vec![merge_at("imp-16/collab/a", 10)],
    );
    let g = vgraph(&[], &tree);
    assert_eq!(
        lane_cols(&g),
        [("imp-16/collab/a".into(), 1), ("imp-16/collab/b".into(), 1)]
    );
    // membership still uses lane.to: a's event at minute 50 stays in column 1.
    let events = [vev("e", 50, "a")];
    let g = vgraph(&events, &tree);
    assert_eq!(
        g.item.rows.iter().find(|r| r.id == "e").map(|r| r.col),
        Some(1)
    );
}

#[test]
fn events_outside_their_lane_span_or_by_the_owner_stay_on_the_trunk() {
    let events = [
        vev("early", 0, "omp"),
        vev("in", 15, "omp"),
        vev("mine", 15, "clara"),
        vev("anon", 15, "x").agent("clara"),
    ];
    let g = vgraph(&events, &vtree(vec![collab(10, 20, "omp")], vec![]));
    let col = |id: &str| g.item.rows.iter().find(|r| r.id == id).map(|r| r.col);
    assert_eq!(
        (col("early"), col("in"), col("mine"), col("anon")),
        (Some(0), Some(1), Some(0), Some(0))
    );
}

#[test]
fn an_unattributed_event_stays_on_the_trunk_and_without_owner_other_agents_may_enter_lanes() {
    let mut anon = vev("anon", 15, "x");
    anon.agent = None;
    let g = vgraph(
        &[anon, vev("o", 15, "omp")],
        &vtree(vec![collab(10, 20, "omp")], vec![]),
    );
    assert_eq!(
        g.item.rows.iter().map(|r| r.col).collect::<Vec<_>>(),
        [0, 1]
    );

    let mut tree = vtree(vec![collab(10, 20, "omp")], vec![]);
    tree.agent = None;
    let g = vgraph(&[vev("o", 15, "omp")], &tree);
    assert_eq!(g.item.rows[0].col, 1);
    assert_eq!(g.item.agent, None);
}

// -- edges and fork geometry ----------------------------------------------------

#[test]
fn nested_lane_elbows_start_at_the_parents_rail() {
    let mut child = vlane("c", 10, 20, None, LaneKind::Path, "child");
    child.parent = Some("imp-16/collab/p".into());
    let parent = collab(0, 30, "p");
    let tree = vtree(
        vec![parent, child],
        vec![
            fork_at("imp-16/collab/p", 0),
            fork_at("c", 10),
            merge_at("c", 20),
        ],
    );
    let events = [
        vev("a", 0, "clara"),
        vev("b", 10, "clara"),
        vev("c", 20, "clara"),
    ];
    let g = vgraph(&events, &tree);
    let p = g
        .item
        .forks
        .iter()
        .find(|f| f.id == "imp-16/collab/p")
        .expect("p");
    let c = g.item.forks.iter().find(|f| f.id == "c").expect("c");
    assert_eq!((p.x, c.x), (col_x(1), col_x(2)));
    let paths: Vec<&str> = g.item.edges.iter().map(|e| e.d.as_str()).collect();
    assert_eq!(paths[0], "M7.0 11.0H21.0");
    assert_eq!(paths[1], "M21.0 55.0H35.0");
    assert_eq!(paths[2], "M35.0 99.0H21.0");
}

#[test]
fn fork_paths_print_one_decimal_and_merge_runs_back() {
    let events = [
        vev("e1", 0, "clara"),
        vev("e2", 10, "omp"),
        vev("e3", 20, "clara"),
    ];
    let tree = vtree(
        vec![collab(10, 10, "omp")],
        vec![
            fork_at("imp-16/collab/omp", 10),
            merge_at("imp-16/collab/omp", 10),
        ],
    );
    let g = vgraph(&events, &tree);
    assert_eq!(g.item.edges[0].d, "M7.0 33.0H21.0");
    assert_eq!(g.item.edges[1].d, "M21.0 33.0H7.0");
    assert_eq!(g.item.edges[0].kind, EdgeKind::Fork);
    assert_eq!(g.item.edges[1].kind, EdgeKind::Merge);
}

#[test]
fn an_open_lane_runs_to_four_px_above_the_bottom_and_never_above_its_start() {
    let events = [
        vev("e1", 0, "clara"),
        vev("e2", 10, "clara"),
        vev("e3", 20, "clara"),
    ];
    let tree = vtree(vec![collab(0, 5, "a")], vec![fork_at("imp-16/collab/a", 0)]);
    let g = vgraph(&events, &tree);
    let f = &g.item.forks[0];
    assert_eq!((f.y0, f.y1, g.height), (11, 84, 88));
    // fork instant after every row: y0 is the last row, y1 clamped up to it.
    let late = vtree(
        vec![collab(0, 5, "a")],
        vec![fork_at("imp-16/collab/a", 1000)],
    );
    let g = vgraph(&events, &late);
    assert_eq!(g.item.forks[0].y0, 77);
    assert!(g.item.forks[0].y1 >= g.item.forks[0].y0);
}

#[test]
fn a_lane_without_a_fork_edge_starts_at_its_first_instant() {
    let events = [
        vev("e1", 0, "clara"),
        vev("e2", 10, "clara"),
        vev("e3", 20, "clara"),
    ];
    let tree = vtree(vec![collab(10, 20, "a")], vec![]);
    let g = vgraph(&events, &tree);
    assert_eq!(g.item.forks[0].y0, 33);
}

#[test]
fn row_y_picks_the_first_row_at_or_after_the_instant_else_the_last() {
    let g = vgraph(
        &[vev("a", 0, "clara"), vev("b", 10, "clara")],
        &vtree(vec![], vec![]),
    );
    let rows = &g.item.rows;
    assert_eq!(row_y(rows, at(0)), 11);
    assert_eq!(row_y(rows, at(5)), 33);
    assert_eq!(row_y(rows, at(10)), 33);
    assert_eq!(row_y(rows, at(11)), 33);
    assert_eq!(row_y(&[], at(0)), 0);
}

#[test]
fn trunk_spans_first_to_last_row_centre() {
    let g = vgraph(
        &[
            vev("a", 0, "clara"),
            vev("b", 10, "clara"),
            vev("c", 20, "clara"),
        ],
        &vtree(vec![], vec![]),
    );
    assert_eq!((g.item.x, g.item.y0, g.item.y1), (7, Some(11), Some(55)));
    let one = vgraph(&[vev("a", 0, "clara")], &vtree(vec![], vec![]));
    assert_eq!((one.item.y0, one.item.y1), (Some(11), Some(11)));
}

#[test]
fn the_owners_own_events_stay_on_the_trunk_even_inside_a_lane_it_owns() {
    let events = [vev("mine", 15, "clara")];
    let tree = vtree(
        vec![vlane("b", 10, 20, Some("clara"), LaneKind::Branch, "feat")],
        vec![],
    );
    let g = vgraph(&events, &tree);
    let mine = g.item.rows.iter().find(|r| r.id == "mine");
    assert_eq!(mine.map(|r| r.col), Some(0));
    assert!(g.item.rows.iter().any(|r| r.kind == RowKind::Lane));
}

#[test]
fn a_merge_before_the_fork_never_draws_the_line_upwards() {
    let events = [vev("e1", 0, "clara"), vev("e2", 10, "clara")];
    let tree = vtree(
        vec![collab(0, 10, "a")],
        vec![
            fork_at("imp-16/collab/a", 10),
            merge_at("imp-16/collab/a", 0),
        ],
    );
    let f = &vgraph(&events, &tree).item.forks[0];
    assert!(f.y1 >= f.y0);
    assert_eq!((f.y0, f.y1), (55, 55));
}

// -- dead lanes -----------------------------------------------------------------

#[test]
fn rows_in_a_dead_lane_are_dead_and_chosen_none_or_true_are_not() {
    let mut dead = collab(10, 20, "omp");
    dead.chosen = Some(false);
    let mut live = collab(30, 40, "bob");
    live.chosen = Some(true);
    let events = [
        vev("e1", 0, "clara"),
        vev("e2", 15, "omp"),
        vev("e3", 35, "bob"),
        vev("e4", 50, "clara"),
    ];
    let g = vgraph(
        &events,
        &vtree(vec![dead, live, collab(60, 70, "z")], vec![]),
    );
    let flags: Vec<(&str, bool)> = g
        .item
        .rows
        .iter()
        .map(|r| (r.id.as_str(), r.dead))
        .collect();
    assert_eq!(
        flags,
        [
            ("e1", false),
            ("e2", true),
            ("e3", false),
            ("e4", false),
            ("lane:imp-16/collab/z", false)
        ]
    );
    let forks: Vec<(bool, Option<bool>)> =
        g.item.forks.iter().map(|f| (f.dead, f.chosen)).collect();
    assert_eq!(
        forks,
        [(true, Some(false)), (false, Some(true)), (false, None)]
    );
    assert!(g.item.edges.iter().take(1).all(|e| e.dead));
    let view = g.view(&RailOptions::default());
    let dead_rows: Vec<&str> = view
        .canvas
        .iter()
        .flat_map(|c| &c.rows)
        .filter(|r| r.dead)
        .map(|r| r.row_id.as_str())
        .collect();
    assert_eq!(dead_rows, ["e2"]);
}

// -- tone ---------------------------------------------------------------------

#[rstest]
#[case(EventKind::ItemClaimed, Tone::Blue)]
#[case(EventKind::RunStarted, Tone::Blue)]
#[case(EventKind::RunTool, Tone::Blue)]
#[case(EventKind::RunThought, Tone::Blue)]
#[case(EventKind::RunReasoning, Tone::Blue)]
#[case(EventKind::BranchHead, Tone::Blue)]
#[case(EventKind::PrOpened, Tone::Blue)]
#[case(EventKind::PrReviewed, Tone::Blue)]
#[case(EventKind::RunFinished, Tone::Green)]
#[case(EventKind::PrMerged, Tone::Green)]
#[case(EventKind::Handoff, Tone::Yellow)]
#[case(EventKind::RunCancelled, Tone::Yellow)]
#[case(EventKind::RunFailed, Tone::Red)]
#[case(EventKind::ItemCreated, Tone::Hollow)]
#[case(EventKind::ItemReleased, Tone::Hollow)]
#[case(EventKind::Note, Tone::Hollow)]
#[case(EventKind::Commit, Tone::Hollow)]
#[case(EventKind::Heartbeat, Tone::Hollow)]
#[case(EventKind::RunModel, Tone::Hollow)]
fn tone_by_kind(#[case] kind: EventKind, #[case] want: Tone) {
    assert_eq!(Event::new("e", 0, kind).tone(), want);
}

#[rstest]
#[case(Some("done"), Tone::Green)]
#[case(Some("review"), Tone::Yellow)]
#[case(Some("blocked"), Tone::Red)]
#[case(Some("in_progress"), Tone::Blue)]
#[case(Some("claimed"), Tone::Blue)]
#[case(Some("open"), Tone::Hollow)]
#[case(None, Tone::Hollow)]
fn item_status_tone_follows_its_status(#[case] status: Option<&str>, #[case] want: Tone) {
    let mut event = Event::new("e", 0, EventKind::ItemStatus);
    event.detail.status = status.map(str::to_owned);
    assert_eq!(event.tone(), want);
}

#[test]
fn a_status_on_another_kind_does_not_recolour_it() {
    let event = Event::new("e", 0, EventKind::Note).status("blocked");
    assert_eq!(event.tone(), Tone::Hollow);
}

#[test]
fn colour_names_and_severity() {
    let names: Vec<&str> = Tone::ALL.iter().map(|t| t.colour()).collect();
    assert_eq!(names, ["blue", "green", "yellow", "red", "hollow"]);
    let sev: Vec<u8> = Tone::ALL.iter().map(|t| t.severity()).collect();
    assert_eq!(sev, [1, 2, 3, 4, 0]);
    assert_eq!(Tone::Red.name(), "err");
}

#[test]
fn every_event_kind_has_a_unique_name_and_label() {
    let names: HashSet<&str> = EventKind::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(names.len(), EventKind::ALL.len());
    assert_eq!(EventKind::PrMerged.label(), "pr merged");
    assert_eq!(EventKind::RunReasoning.label(), "run reasoning");
}

#[test]
fn drawn_tones_cover_every_colour_the_renderer_emits() {
    let mut events = Vec::new();
    for (i, kind) in EventKind::ALL.iter().enumerate() {
        events.push(Event::new(format!("k{i}"), at(i as i64), *kind).item("imp-16"));
    }
    for (i, status) in [
        "done",
        "review",
        "blocked",
        "in_progress",
        "claimed",
        "open",
    ]
    .iter()
    .enumerate()
    {
        events.push(
            Event::new(format!("s{i}"), at(100 + i as i64), EventKind::ItemStatus)
                .item("imp-16")
                .status(*status),
        );
    }
    let g = vgraph(&events, &vtree(vec![], vec![]));
    let drawn: HashSet<Tone> = g.item.rows.iter().map(|r| r.tone).collect();
    assert_eq!(drawn, Tone::ALL.iter().copied().collect::<HashSet<_>>());
}

// -- row text -----------------------------------------------------------------

#[test]
fn the_tooltip_joins_stamp_kind_agent_summary() {
    let g = vgraph(&[vev("e2", 10, "omp")], &vtree(vec![], vec![]));
    assert_eq!(g.item.rows[0].title, "Oct 1 00:10 · note · omp · e2 said");
    assert_eq!(g.item.rows[0].detail.as_deref(), Some("e2 said"));
}

#[test]
fn the_tooltip_drops_absent_and_empty_parts() {
    let mut e = Event::new("e", at(0), EventKind::PrMerged)
        .item("imp-16")
        .summary("");
    e.agent = None;
    let g = vgraph(&[e], &vtree(vec![], vec![]));
    assert_eq!(g.item.rows[0].title, "Oct 1 00:00 · pr merged");
    assert_eq!(g.item.rows[0].label, "pr merged");
}

#[rstest]
#[case(Some("  fix\n   the\tbug  "), "fix the bug")]
#[case(Some("   \n "), "run tool")]
#[case(Some(""), "run tool")]
#[case(None, "run tool")]
#[case(Some("one"), "one")]
fn the_caption_is_the_one_line_summary_else_the_kind(
    #[case] summary: Option<&str>,
    #[case] want: &str,
) {
    let mut e = Event::new("e", at(0), EventKind::RunTool).item("imp-16");
    e.summary = summary.map(str::to_owned);
    let g = vgraph(&[e], &vtree(vec![], vec![]));
    assert_eq!(g.item.rows[0].label, want);
}

#[test]
fn ties_on_the_same_instant_break_by_id_bytes() {
    let events = [
        vev("b", 5, "clara"),
        vev("a", 5, "clara"),
        vev("B", 5, "clara"),
    ];
    let g = vgraph(&events, &vtree(vec![], vec![]));
    assert_eq!(row_ids(&g), ["B", "a", "b"]);
}

#[test]
fn stamps_pick_the_format_of_the_whole_rail() {
    let events = [vev("a", 0, "clara"), vev("b", 30 * 60, "clara")];
    let g = vgraph(&events, &vtree(vec![], vec![]));
    let view = g.view(&RailOptions::default());
    let stamps: Vec<&str> = view
        .canvas
        .iter()
        .flat_map(|c| &c.rows)
        .map(|r| r.stamp.as_str())
        .collect();
    assert_eq!(stamps, ["10-01 00:00", "10-02 06:00"]);
    let isos: Vec<&str> = view
        .canvas
        .iter()
        .flat_map(|c| &c.rows)
        .map(|r| r.iso.as_str())
        .collect();
    assert_eq!(isos, ["2026-10-01T00:00:00Z", "2026-10-02T06:00:00Z"]);

    let same = vgraph(
        &[vev("a", 0, "clara"), vev("b", 5, "clara")],
        &vtree(vec![], vec![]),
    );
    let stamps: Vec<String> = same
        .view(&RailOptions::default())
        .canvas
        .iter()
        .flat_map(|c| c.rows.clone())
        .map(|r| r.stamp)
        .collect();
    assert_eq!(stamps, ["00:00", "00:05"]);
}

#[test]
fn two_dates_apart_by_a_minute_across_midnight_are_day_time() {
    let a = from_utc(2026, 10, 1, 23, 59, 0);
    let events = [
        Event::new("a", a, EventKind::Note).item("imp-16"),
        Event::new("b", a + MIN, EventKind::Note).item("imp-16"),
    ];
    assert_eq!(
        vgraph(&events, &vtree(vec![], vec![])).stamp,
        Stamp::DayTime
    );
}

#[test]
fn the_chip_rule_brackets_and_uppercases() {
    assert_eq!(chip_text("clara"), "[CLARA]");
    assert_eq!(chip_text("feat/x"), "[FEAT/X]");
}

#[test]
fn view_chips_appear_only_on_a_lanes_first_row() {
    let events = [
        vev("e1", 0, "clara"),
        vev("e2", 10, "omp"),
        vev("e3", 20, "omp"),
    ];
    let g = vgraph(&events, &vtree(vec![collab(10, 20, "omp")], vec![]));
    let view = g.view(&RailOptions::default());
    let chips: Vec<Option<&str>> = view
        .canvas
        .iter()
        .flat_map(|c| &c.rows)
        .map(|r| r.chip.as_deref())
        .collect();
    assert_eq!(chips, [None, Some("[OMP]"), None]);
}

// -- item summary ---------------------------------------------------------------

#[test]
fn item_status_wins_else_the_last_events_status() {
    let events = [
        vev("a", 0, "clara").status("blocked").title("old"),
        vev("b", 10, "clara").status("done"),
    ];
    let with_item = vgraph(&events, &vtree(vec![], vec![]));
    assert_eq!(
        (with_item.item.status.as_deref(), with_item.item.tone),
        (Some("claimed"), Tone::Blue)
    );

    let no_status = vertical(
        &events,
        &ItemRef {
            id: "imp-16".into(),
            status: None,
        },
        &vtree(vec![], vec![]),
    );
    assert_eq!(
        (
            no_status.item.status.as_deref(),
            no_status.item.tone,
            no_status.item.done
        ),
        (Some("done"), Tone::Green, true)
    );
    assert_eq!(no_status.item.title.as_deref(), Some("old"));
    assert_eq!(no_status.item.last, Some(at(10)));
}

#[test]
fn the_title_is_the_newest_one_and_a_merged_pr_finishes_the_item() {
    let events = [
        vev("a", 0, "clara").title("first"),
        vev("b", 5, "clara").title("second"),
        Event::new("c", at(9), EventKind::PrMerged).item("imp-16"),
    ];
    let g = vgraph(&events, &vtree(vec![], vec![]));
    assert_eq!(g.item.title.as_deref(), Some("second"));
    assert!(g.item.done);
    assert!(!vgraph(&events[..2], &vtree(vec![], vec![])).item.done);
}

#[test]
fn a_status_run_has_a_neutral_item_line() {
    let g = vertical(
        &[vev("a", 0, "clara")],
        &ItemRef {
            id: "imp-16".into(),
            status: Some("run".into()),
        },
        &vtree(vec![], vec![]),
    );
    assert_eq!(g.item.tone, Tone::Hollow);
}

// -- view ---------------------------------------------------------------------

#[test]
fn the_canvas_carries_gutter_height_and_trunk() {
    let events = [vev("e1", 0, "clara"), vev("e2", 10, "omp")];
    let g = vgraph(&events, &vtree(vec![collab(10, 10, "omp")], vec![]));
    let view = g.view(&RailOptions {
        separators: true,
        ..RailOptions::default()
    });
    let canvas = view.canvas.expect("rows");
    assert_eq!((canvas.gutter, canvas.height), (28, 44));
    assert_eq!(
        (canvas.trunk_x, canvas.trunk_y0, canvas.trunk_y1),
        (7, 11, 33)
    );
    assert!(canvas.rows.iter().all(|r| r.separator));
    assert_eq!(canvas.tone, Tone::Blue);
    assert!(!canvas.done);
    assert_eq!(
        view.heading.and_then(|h| h.meta).as_deref(),
        Some("2 events · 1 lane")
    );
}

#[test]
fn default_options_match_the_session_tree_rail() {
    let o = RailOptions::default();
    assert_eq!(
        (
            o.title.as_str(),
            o.dom_id.as_str(),
            o.heading,
            o.on_select,
            o.separators
        ),
        ("session tree", "activity-graph", true, false, false)
    );
    assert_eq!(o.selected, None);
}

#[test]
fn the_view_serializes_for_a_painter() {
    let g = vgraph(&[vev("e1", 0, "clara")], &vtree(vec![], vec![]));
    let json = serde_json::to_value(g.view(&RailOptions::default())).expect("serialize");
    assert_eq!(json["canvas"]["rows"][0]["kind"], "note");
    assert_eq!(json["canvas"]["rows"][0]["hit"], "static");
    assert_eq!(json["canvas"]["rows"][0]["tone"], "hollow");
    assert_eq!(json["empty"], serde_json::Value::Null);
}

#[test]
fn inputs_round_trip_and_reject_unknown_fields() {
    let tree = vtree(
        vec![collab(1, 2, "omp")],
        vec![fork_at("imp-16/collab/omp", 1)],
    );
    let json = serde_json::to_string(&tree).expect("serialize");
    assert_eq!(
        serde_json::from_str::<SessionTree>(&json).expect("parse"),
        tree
    );
    assert!(serde_json::from_str::<SessionTree>(r#"{"agent":"a","bogus":1}"#).is_err());
    let ev: Event = serde_json::from_str(r#"{"id":"e","at":5,"kind":"pr_merged"}"#).expect("parse");
    assert_eq!(ev.kind, EventKind::PrMerged);
    let default: SessionTree = serde_json::from_str("{}").expect("parse");
    assert_eq!(default, SessionTree::default());
}

// -- open nodes ---------------------------------------------------------------

fn claimed() -> ItemRef {
    ItemRef {
        id: "imp-16".to_owned(),
        status: Some("claimed".to_owned()),
    }
}

#[test]
fn vertical_with_a_closed_set_is_identical_to_vertical() {
    let events = [
        vev("e1", 0, "clara"),
        vev("e2", 10, "omp"),
        vev("e3", 20, "clara"),
    ];
    let tree = vtree(
        vec![collab(10, 10, "omp")],
        vec![
            fork_at("imp-16/collab/omp", 10),
            merge_at("imp-16/collab/omp", 10),
        ],
    );
    let g = vgraph(&events, &tree);
    assert_eq!(
        g,
        vertical_with(&events, &claimed(), &tree, &Expansion::new(), facts::WRAP_COLS)
    );
}

#[test]
fn an_open_row_adds_its_fact_lines_and_shifts_everything_below() {
    let events = [vev("e1", 0, "clara"), vev("e2", 10, "clara"), vev("e3", 20, "clara")];
    let tree = vtree(vec![], vec![]);
    let mut open = Expansion::new();
    open.open_all(["e1"]);
    let added = facts::line_count(&facts::event_facts(&events[0], facts::WRAP_COLS)) as i32;
    let g = vertical_with(&events, &claimed(), &tree, &open, facts::WRAP_COLS);
    let ys: Vec<i32> = g.item.rows.iter().map(|r| r.y).collect();
    assert_eq!(ys, [11, 33 + added * 22, 55 + added * 22]);
    assert_eq!(g.height, 3 * 22 + added * 22);
    assert_eq!((g.item.y0, g.item.y1), (Some(11), Some(55 + added * 22)));
    assert_eq!(
        g.item.rows[0].facts,
        facts::event_facts(&events[0], facts::WRAP_COLS)
    );
    assert!(g.item.rows[1].facts.is_empty());
}

#[test]
fn lanes_and_edges_span_the_grown_canvas_through_open_blocks() {
    let events = [
        vev("e1", 0, "clara"),
        vev("e2", 10, "omp"),
        vev("e3", 20, "clara"),
    ];
    let tree = vtree(
        vec![collab(10, 10, "omp"), collab(20, 20, "eve")],
        vec![
            fork_at("imp-16/collab/omp", 10),
            merge_at("imp-16/collab/omp", 10),
            fork_at("imp-16/collab/eve", 20),
        ],
    );
    let mut open = Expansion::new();
    open.open_all(["e1"]);
    let added = facts::line_count(&facts::event_facts(&events[0], facts::WRAP_COLS)) as i32;
    let g = vertical_with(&events, &claimed(), &tree, &open, facts::WRAP_COLS);
    let shifted = 33 + added * 22;
    // the merge elbow lands on the row it merged at, now shifted
    assert_eq!(g.item.edges[0].d, format!("M7.0 {shifted}.0H21.0"));
    assert_eq!(g.item.edges[1].d, format!("M21.0 {shifted}.0H7.0"));
    // the open lane still runs to 4px above the grown bottom
    assert_eq!(g.item.forks[1].y1, g.height - 4);
}

#[test]
fn an_open_lane_marker_carries_its_lane_facts() {
    let events = [vev("e1", 0, "clara"), vev("e2", 30, "clara")];
    let tree = vtree(
        vec![vlane("b", 10, 20, None, LaneKind::Branch, "feat/x")],
        vec![],
    );
    let mut open = Expansion::new();
    open.open_all(["lane:b"]);
    let added = facts::line_count(&facts::lane_facts(&tree.lanes[0], facts::WRAP_COLS)) as i32;
    let g = vertical_with(&events, &claimed(), &tree, &open, facts::WRAP_COLS);
    let marker = g
        .item
        .rows
        .iter()
        .find(|r| r.kind == RowKind::Lane)
        .unwrap();
    assert_eq!(marker.facts, facts::lane_facts(&tree.lanes[0], facts::WRAP_COLS));
    let ys: Vec<i32> = g.item.rows.iter().map(|r| r.y).collect();
    // the open marker keeps its own row; only the rows below it shift
    assert_eq!(ys, [11, 33, 55 + added * 22]);
    assert_eq!(g.height, 66 + added * 22);
}

#[test]
fn the_view_resolves_open_facts_and_detail_ids() {
    let events = [vev("e1", 0, "clara")];
    let tree = vtree(vec![vlane("b", 10, 20, None, LaneKind::Branch, "x")], vec![]);
    let mut open = Expansion::new();
    open.open_all(["e1"]);
    let g = vertical_with(&events, &claimed(), &tree, &open, facts::WRAP_COLS);
    let view = g.view(&RailOptions {
        dom_id: "st".into(),
        on_select: true,
        ..RailOptions::default()
    });
    let canvas = view.canvas.as_ref().expect("rows");
    let e1 = canvas.rows.iter().find(|r| r.row_id == "e1").unwrap();
    assert!(e1.open);
    assert_eq!(e1.facts, facts::event_facts(&events[0], facts::WRAP_COLS));
    assert_eq!(e1.detail_dom_id, "st-detail-e1");
    let marker = canvas.rows.iter().find(|r| r.lane_marker).unwrap();
    assert!(!marker.open && marker.facts.is_empty());
    assert_eq!(marker.detail_dom_id, "st-detail-lane:b");
    let plain = vertical(&events, &claimed(), &tree).view(&RailOptions::default());
    let canvas = plain.canvas.as_ref().expect("rows");
    assert!(canvas.rows.iter().all(|r| !r.open && r.facts.is_empty()));
}

// -- invariants -----------------------------------------------------------------

#[derive(Clone, Debug)]
struct LaneSpec {
    from: i64,
    len: i64,
    agent: usize,
    kind: usize,
    chosen: Option<bool>,
    merged: bool,
    forked: bool,
}

const AGENTS: [&str; 4] = ["clara", "omp", "bob", "eve"];

fn lane_spec() -> impl Strategy<Value = LaneSpec> {
    (
        0i64..120,
        0i64..40,
        0usize..4,
        0usize..4,
        proptest::option::of(any::<bool>()),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(
            |(from, len, agent, kind, chosen, merged, forked)| LaneSpec {
                from,
                len,
                agent,
                kind,
                chosen,
                merged,
                forked,
            },
        )
}

fn build(events: &[(i64, usize, usize, bool)], lanes: &[LaneSpec]) -> (Vec<Event>, SessionTree) {
    let evs: Vec<Event> = events
        .iter()
        .enumerate()
        .map(|(i, (minute, agent, kind, other_item))| {
            let mut e = Event::new(
                format!("e{i}"),
                at(*minute),
                EventKind::ALL[*kind % EventKind::ALL.len()],
            )
            .agent(AGENTS[*agent])
            .item(if *other_item { "imp-99" } else { "imp-16" });
            e.summary = Some(format!("s{i}"));
            e
        })
        .collect();
    let kinds = [
        LaneKind::Path,
        LaneKind::Branch,
        LaneKind::Handoff,
        LaneKind::Collab,
    ];
    let mut tree_lanes = Vec::new();
    let mut edges = Vec::new();
    for (i, spec) in lanes.iter().enumerate() {
        let id = format!("l{i}");
        let mut lane = vlane(
            &id,
            spec.from,
            spec.from + spec.len,
            Some(AGENTS[spec.agent]),
            kinds[spec.kind],
            &format!("lab{i}"),
        );
        lane.chosen = spec.chosen;
        if i > 0 && spec.len % 2 == 0 {
            lane.parent = Some(format!("l{}", i - 1));
        }
        tree_lanes.push(lane);
        if spec.forked {
            edges.push(fork_at(&id, spec.from));
        }
        if spec.merged {
            edges.push(merge_at(&id, spec.from + spec.len));
        }
    }
    (evs, vtree(tree_lanes, edges))
}

proptest! {
    #[test]
    fn rails_hold_their_geometry_and_selection_invariants(
        events in proptest::collection::vec((0i64..200, 0usize..4, 0usize..20, any::<bool>()), 0..25),
        lanes in proptest::collection::vec(lane_spec(), 0..6),
    ) {
        let (evs, tree) = build(&events, &lanes);
        let g = vgraph(&evs, &tree);
        let rows = &g.item.rows;

        // rows: sorted, one per own event plus one marker per eventless lane, y strictly spaced
        let own = evs.iter().filter(|e| e.item_id.as_deref() == Some("imp-16")).count();
        let markers = rows.iter().filter(|r| r.kind == RowKind::Lane).count();
        prop_assert_eq!(rows.len(), own + markers);
        prop_assert!(markers <= tree.lanes.len());
        prop_assert!(rows.windows(2).all(|w| (w[0].at, &w[0].id) <= (w[1].at, &w[1].id)));
        for (i, row) in rows.iter().enumerate() {
            prop_assert_eq!(row.y, i as i32 * 22 + 11);
            prop_assert_eq!(row.x, col_x(row.col));
            prop_assert_eq!(row.selectable, row.kind != RowKind::Lane);
        }
        prop_assert_eq!(g.height, rows.len() as i32 * 22);

        // no two lanes share a column while their spans overlap
        let merges: std::collections::HashMap<&str, i64> = tree.edges.iter().filter(|e| e.kind == EdgeKind::Merge).map(|e| (e.lane.as_str(), e.at)).collect();
        let span = |f: &Fork| {
            let lane = tree.lanes.iter().find(|l| l.id == f.id).expect("lane");
            (lane.from, merges.get(f.id.as_str()).copied().unwrap_or(lane.to))
        };
        for (i, a) in g.item.forks.iter().enumerate() {
            prop_assert!(a.col >= 1 && a.col <= tree.lanes.len() as u32);
            for b in &g.item.forks[i + 1..] {
                if a.col == b.col {
                    let (sa, sb) = (span(a), span(b));
                    prop_assert!(!(sa.0 <= sb.1 && sa.1 >= sb.0), "{} and {} overlap in column {}", a.id, b.id, a.col);
                }
            }
        }

        // columns pack leftwards: every column below a used one is used
        let used: HashSet<u32> = g.item.forks.iter().map(|f| f.col).collect();
        let max = used.iter().copied().max().unwrap_or(0);
        prop_assert!((1..=max).all(|c| used.contains(&c)));
        prop_assert_eq!(g.gutter, col_x(max) + 7);

        // every lane has a fork elbow, and a merge elbow unless open; each starts on its parent rail
        prop_assert_eq!(g.item.edges.iter().filter(|e| e.kind == EdgeKind::Fork).count(), g.item.forks.len());
        prop_assert_eq!(g.item.edges.iter().filter(|e| e.kind == EdgeKind::Merge).count(), g.item.forks.iter().filter(|f| !f.open).count());
        let mut paths = g.item.edges.iter();
        for fork in &g.item.forks {
            let parent_x = fork.parent.as_deref().and_then(|p| g.item.forks.iter().find(|f| f.id == p)).map_or(7, |p| p.x);
            let fork_path = paths.next().expect("fork edge");
            prop_assert_eq!(fork_path.kind, EdgeKind::Fork);
            prop_assert_eq!(&fork_path.d, &format!("M{parent_x}.0 {}.0H{}.0", fork.y0, fork.x));
            prop_assert_eq!(fork_path.dead, fork.dead);
            if !fork.open {
                let merge_path = paths.next().expect("merge edge");
                prop_assert_eq!(merge_path.kind, EdgeKind::Merge);
                prop_assert_eq!(&merge_path.d, &format!("M{}.0 {}.0H{}.0", fork.x, fork.y1, parent_x));
            }
            prop_assert!(fork.y0 <= fork.y1);
            prop_assert!(fork.y1 <= g.height.max(0));
        }
        prop_assert!(paths.next().is_none());

        // selection: every selectable row is findable, markers never are
        for row in rows {
            prop_assert_eq!(g.find_dot(&row.id).is_some(), row.selectable);
        }

        // header counts
        let events_n = rows.iter().filter(|r| r.selectable).count();
        prop_assert_eq!(g.header(), header_text(events_n, g.item.forks.len()));

        // a named lane appears at most once and only on a non-marker row in that lane's column
        let named: Vec<&str> = rows.iter().filter_map(|r| r.lane.as_deref()).collect();
        let unique: HashSet<&str> = named.iter().copied().collect();
        prop_assert_eq!(named.len(), unique.len());
        prop_assert!(rows.iter().all(|r| r.lane.is_none() || (r.col > 0 && r.kind != RowKind::Lane)));

        // stamp format is whole-rail
        let days: HashSet<i64> = rows.iter().map(|r| time::day_index(r.at)).collect();
        prop_assert_eq!(g.stamp == Stamp::DayTime, days.len() > 1);
    }

    #[test]
    fn layout_is_independent_of_input_order(
        events in proptest::collection::vec((0i64..200, 0usize..4, 0usize..20, any::<bool>()), 0..15),
        lanes in proptest::collection::vec(lane_spec(), 0..5),
    ) {
        let (evs, tree) = build(&events, &lanes);
        let mut rev_events = evs.clone();
        rev_events.reverse();
        let mut rev_tree = tree.clone();
        rev_tree.lanes.reverse();
        rev_tree.edges.reverse();
        prop_assert_eq!(vgraph(&evs, &tree), vgraph(&rev_events, &rev_tree));
    }

    #[test]
    fn every_non_trunk_event_row_lies_inside_the_lane_of_its_column(
        events in proptest::collection::vec((0i64..200, 0usize..4, 0usize..20, any::<bool>()), 0..25),
        lanes in proptest::collection::vec(lane_spec(), 0..6),
    ) {
        let (evs, tree) = build(&events, &lanes);
        let g = vgraph(&evs, &tree);
        for row in g.item.rows.iter().filter(|r| r.col > 0) {
            let fork_in_col = g.item.forks.iter().any(|f| {
                let lane = tree.lanes.iter().find(|l| l.id == f.id).expect("lane");
                f.col == row.col && lane.from <= row.at && row.at <= lane.to
            });
            prop_assert!(fork_in_col);
        }
    }

    #[test]
    fn open_rails_shift_rows_and_grow_height_exactly(
        events in proptest::collection::vec((0i64..200, 0usize..4, 0usize..20, any::<bool>()), 1..10),
        lanes in proptest::collection::vec(lane_spec(), 0..4),
    ) {
        let (evs, tree) = build(&events, &lanes);
        let mut ids: Vec<String> = evs
            .iter()
            .filter(|e| e.item_id.as_deref() == Some("imp-16"))
            .map(|e| e.id.clone())
            .collect();
        ids.extend(tree.lanes.iter().map(|l| format!("lane:{}", l.id)));
        let mut open = Expansion::new();
        for (i, id) in ids.iter().enumerate() {
            if i % 2 == 0 {
                open.toggle(id);
            }
        }
        let g = vertical_with(&evs, &claimed(), &tree, &open, facts::WRAP_COLS);
        let rows = &g.item.rows;
        let mut total = 0;
        for row in rows {
            prop_assert_eq!(row.y, total + 11);
            prop_assert_eq!(!row.facts.is_empty(), open.is_open(&row.id));
            total += 22 + facts::line_count(&row.facts) as i32 * 22;
        }
        prop_assert_eq!(g.height, total);
        prop_assert!(rows.windows(2).all(|w| w[1].y > w[0].y));
        prop_assert!(g
            .item
            .forks
            .iter()
            .all(|f| f.y0 <= f.y1 && f.y1 <= g.height.max(0)));
    }
}
