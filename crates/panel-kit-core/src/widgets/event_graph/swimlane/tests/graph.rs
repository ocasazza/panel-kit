//! `graph_test.exs` (horizontal) ported, and the layout facts it implies:
//! rows, bursts, strip, forks, caps, margins, clearances, and the spec's
//! open-question answers.

use std::collections::{BTreeSet, HashMap};

use rstest::rstest;

use super::super::*;
use super::fixtures::*;
use crate::widgets::event_graph::model::{
    Edge, EdgeKind, Event, EventKind, Lane, LaneKind, SessionTree,
};
use crate::widgets::event_graph::query::{legend, show_all_label, Tone};

fn item<'a>(graph: &'a Graph, id: &str) -> &'a Item {
    graph
        .items
        .iter()
        .find(|item| item.id == id)
        .expect("item in the graph")
}

fn lane(graph: &Graph, id: &str) -> usize {
    item(graph, id).lane
}

fn of_kind(kind: EventKind, event: Event) -> Event {
    Event { kind, ..event }
}

fn fork_lane(item: &str, from: f64, to: f64, label: &str) -> Lane {
    Lane {
        id: format!("{item}/collab/{label}"),
        item_id: item.to_string(),
        kind: LaneKind::Collab,
        label: label.to_string(),
        from: at(from),
        to: at(to),
        chosen: None,
        parent: None,
        agent: Some(label.to_string()),
    }
}

fn tree(lanes: Vec<Lane>, edges: Vec<Edge>) -> SessionTree {
    SessionTree {
        agent: None,
        lanes,
        edges,
    }
}

fn edge(kind: EdgeKind, lane: &Lane, hour: f64) -> Edge {
    Edge {
        kind,
        lane: lane.id.clone(),
        item_id: lane.item_id.clone(),
        at: at(hour),
    }
}

fn with_tree(events: &[Event], tree: &SessionTree) -> Graph {
    Graph::layout(events, &opts().tree(tree))
}

// -- lanes ------------------------------------------------------------------

#[test]
fn items_never_open_together_share_a_row_and_no_more_rows_are_used_than_ever_overlap() {
    let graph = layout(&[
        ev("imp-1", 0.0),
        ev("imp-1", 20.0),
        ev("imp-2", 10.0),
        ev("imp-2", 100.0),
        ev("imp-3", 60.0),
        ev("imp-3", 80.0),
    ]);

    assert_eq!(graph.lanes, 2);
    assert_eq!(lane(&graph, "imp-1"), lane(&graph, "imp-3"));
    assert_ne!(lane(&graph, "imp-1"), lane(&graph, "imp-2"));
}

#[test]
fn a_branch_that_starts_under_the_previous_ones_tip_label_takes_another_row() {
    let graph = layout(&[
        ev("imp-1", 0.0),
        ev("imp-1", 10.0),
        ev("imp-2", 10.0),
        ev("imp-2", 40.0),
    ]);

    assert_ne!(lane(&graph, "imp-1"), lane(&graph, "imp-2"));
}

#[test]
fn a_branch_that_starts_after_the_label_has_ended_reuses_the_row() {
    let graph = layout(&[
        ev("imp-1", 0.0),
        ev("imp-1", 10.0),
        ev("imp-2", 60.0),
        ev("imp-2", 90.0),
    ]);

    assert_eq!(graph.lanes, 1);
}

// -- branches ---------------------------------------------------------------

#[test]
fn a_branch_is_drawn_in_the_status_its_latest_event_left_it_in() {
    let graph = layout(&[
        ev("imp-1", 0.0).status("in_progress"),
        ev("imp-1", 30.0).status("blocked"),
    ]);

    assert_eq!(graph.items.len(), 1);
    assert_eq!(graph.items[0].status.as_deref(), Some("blocked"));
    assert_eq!(graph.items[0].tone, Tone::Red);
}

#[test]
fn a_merged_pr_ends_the_branch_as_done_whatever_the_last_status_said() {
    let graph = layout(&[
        ev("imp-1", 0.0).status("review"),
        of_kind(EventKind::PrMerged, ev("imp-1", 30.0)),
    ]);

    assert!(graph.items[0].done);
}

#[test]
fn events_closer_than_a_dot_merge_into_one_keeping_every_title_and_the_severest_tone() {
    let graph = layout(&[
        of_kind(
            EventKind::ItemStatus,
            ev("imp-1", 0.0).status("blocked").summary("failed"),
        ),
        of_kind(EventKind::Note, ev("imp-1", 0.0).summary("retrying")),
        of_kind(EventKind::Note, ev("imp-1", 90.0)),
    ]);

    let dots = &graph.items[0].dots;
    assert_eq!(dots.len(), 2);
    let burst = &dots[0];
    assert_eq!(burst.count, 2);
    assert_eq!(burst.tone, Tone::Red);
    assert!(burst.title.contains("failed"));
    assert!(burst.title.contains("retrying"));
}

#[test]
fn events_with_no_work_item_collect_on_their_own_row_not_as_a_branch() {
    let graph = layout(&[loose_ev(EventKind::Commit, 5.0), ev("imp-1", 5.0)]);

    assert_eq!(graph.items.len(), 1);
    assert_eq!(graph.items[0].id, "imp-1");
    assert!(graph.loose.is_empty());
    assert_eq!(graph.strip.len(), 1);
    assert_eq!(
        (graph.strip[0].kind, graph.strip[0].count),
        (EventKind::Commit, 1)
    );

    let open = Graph::layout(
        &[loose_ev(EventKind::Commit, 5.0)],
        &opts().loose_open(true),
    );
    assert_eq!(open.loose.len(), 1);
}

#[test]
fn an_empty_window_lays_out_an_empty_graph() {
    let graph = layout(&[]);

    assert!(graph.items.is_empty() && graph.loose.is_empty());
    assert_eq!(graph.lanes, 0);
    assert!(graph.is_empty());
}

// A week where only the last day had work spends the width on that day.
#[test]
fn idle_days_collapse_a_week_with_one_busy_day_is_not_drawn_as_a_week() {
    let graph = layout(&[ev("imp-1", 6.0 * 24.0), ev("imp-1", 6.0 * 24.0 + 6.0)]);
    let it = &graph.items[0];

    assert!(it.x1 - it.x0 > 600.0);
    assert!(graph
        .gaps
        .iter()
        .map(|gap| gap.ms)
        .max()
        .is_some_and(|ms| ms > 5 * 86_400_000));
    assert!(graph
        .gaps
        .iter()
        .all(|gap| (gap.x1 - gap.x0 - 24.0).abs() < 1e-6));
}

// -- clusters ---------------------------------------------------------------

#[test]
fn five_coincident_events_on_one_row_are_one_glyph_with_the_count() {
    let mut events = vec![ev("imp-1", 29.0)];
    events.extend((0..5).map(|_| ev("imp-1", 30.0)));
    events.push(ev("imp-1", 120.0));
    let graph = layout(&events);

    let dots = &graph.items[0].dots;
    let mut counts: Vec<u32> = dots.iter().map(|dot| dot.count).collect();
    counts.sort_unstable();
    assert_eq!(counts, vec![1, 1, 5]);

    let clusters: Vec<&Dot> = dots.iter().filter(|dot| dot.is_cluster()).collect();
    assert_eq!(clusters.len(), 1);
    assert_eq!(clusters[0].count, 5);
    assert!(clusters[0].cluster_title().starts_with("5 events\n"));
}

#[test]
fn events_6px_or_more_apart_stay_separate() {
    let graph = layout(&[ev("imp-1", 0.0), ev("imp-1", 1.0), ev("imp-1", 1.5)]);

    let xs: Vec<f64> = graph.items[0].dots.iter().map(|dot| dot.x).collect();
    assert_eq!(xs.len(), 3);
    assert!(xs.windows(2).all(|pair| pair[1] - pair[0] >= 6.0));
}

// -- the legend -------------------------------------------------------------

const KINDS: [EventKind; 17] = [
    EventKind::ItemCreated,
    EventKind::ItemClaimed,
    EventKind::ItemReleased,
    EventKind::ItemStatus,
    EventKind::Note,
    EventKind::Handoff,
    EventKind::RunStarted,
    EventKind::RunTool,
    EventKind::RunThought,
    EventKind::RunReasoning,
    EventKind::RunModel,
    EventKind::RunFinished,
    EventKind::Commit,
    EventKind::BranchHead,
    EventKind::PrOpened,
    EventKind::PrReviewed,
    EventKind::PrMerged,
];

const STATUSES: [&str; 6] = [
    "open",
    "claimed",
    "in_progress",
    "review",
    "blocked",
    "done",
];

fn drawn_tones(graph: &Graph) -> BTreeSet<Tone> {
    graph
        .items
        .iter()
        .flat_map(|item| item.dots.iter().map(|dot| dot.tone))
        .collect()
}

fn legend_tones() -> BTreeSet<Tone> {
    legend().iter().map(|entry| entry.tone).collect()
}

#[test]
fn the_legend_lists_every_colour_the_renderer_emits() {
    // The Elixir fixture: one item per kind, all six statuses at one instant.
    let events: Vec<Event> = KINDS
        .iter()
        .enumerate()
        .flat_map(|(n, kind)| {
            STATUSES.iter().map(move |status| {
                of_kind(
                    *kind,
                    ev(&format!("imp-{n}"), n as f64 * 3.0).status(*status),
                )
            })
        })
        .collect();
    let graph = layout(&events);

    let drawn = drawn_tones(&graph);
    assert!(drawn.len() >= 4);
    assert!(drawn.is_subset(&legend_tones()));
    assert!(legend_tones().is_subset(&drawn));
}

#[test]
fn every_kind_and_status_pair_on_its_own_item_draws_a_legend_tone_and_all_are_used() {
    let mut events = Vec::new();
    for (n, kind) in KINDS.iter().enumerate() {
        for (s, status) in STATUSES.iter().enumerate() {
            events.push(of_kind(
                *kind,
                ev(&format!("imp-{n}-{s}"), (n * 7 + s) as f64 * 3.0).status(*status),
            ));
        }
    }
    let drawn = drawn_tones(&layout(&events));

    assert_eq!(drawn, legend_tones());
}

#[test]
fn the_legend_names_what_the_colours_mean_from_the_same_tables() {
    let means = |tone: Tone| -> Vec<String> {
        legend()
            .iter()
            .find(|entry| entry.tone == tone)
            .map(|entry| entry.means.clone())
            .unwrap_or_default()
    };

    assert!(means(Tone::Blue).contains(&"claimed".to_string()));
    assert!(means(Tone::Green).contains(&"pr merged".to_string()));
    assert!(means(Tone::Yellow).contains(&"handoff".to_string()));
    assert!(means(Tone::Red).contains(&"blocked".to_string()));
    assert!(means(Tone::Hollow).contains(&"item released".to_string()));
    assert_eq!(
        legend()
            .iter()
            .find(|entry| entry.tone == Tone::Hollow)
            .map(|e| e.colour),
        Some("hollow")
    );
}

// -- the no-work-item strip ---------------------------------------------------

#[test]
fn the_strip_collapses_to_a_density_bar_with_the_count_not_a_row_of_dots() {
    let mut events: Vec<Event> = (0..10)
        .map(|h| loose_ev(EventKind::Commit, 20.0 + f64::from(h / 5)))
        .collect();
    events.push(ev("imp-1", 21.0));
    let graph = layout(&events);

    assert_eq!(graph.strip.len(), 1);
    let row = &graph.strip[0];
    assert_eq!(row.kind, EventKind::Commit);
    assert_eq!(row.count, 10);
    assert!(graph.loose.is_empty());
    assert!(!row.cells.is_empty());
    assert_eq!(row.cells.iter().map(|cell| cell.count).sum::<usize>(), 10);
    assert!(!row.open);
    assert_eq!(row.label(), "no item: commit");
    assert_eq!(row.title(), "10 commit events with no work item");
}

#[test]
fn the_strip_expands_to_dots_when_open() {
    let events = [ev("imp-1", 21.0), loose_ev(EventKind::Commit, 21.0)];
    let graph = Graph::layout(&events, &opts().loose_open(true));

    assert_eq!(graph.loose.len(), 1);
    assert_eq!(graph.loose[0].dots.len(), 1);
    assert!(graph.strip[0].open);
}

#[test]
fn a_busy_strip_is_darker_where_more_events_fall() {
    let mut events = vec![
        ev("imp-1", 100.0),
        ev("imp-1", 160.0),
        loose_ev(EventKind::Commit, 130.0),
    ];
    events.extend((0..8).map(|_| loose_ev(EventKind::Commit, 100.0)));
    let graph = layout(&events);

    let alphas: Vec<f64> = graph.strip[0].cells.iter().map(|cell| cell.alpha).collect();
    assert_eq!(alphas.iter().copied().fold(f64::MIN, f64::max), 1.0);
    assert!(alphas.iter().copied().fold(f64::MAX, f64::min) < 0.5);
}

#[test]
fn each_kind_is_its_own_strip_row_closed_by_default() {
    // The Elixir fixture used a free-form `:branch`; the closed vocabulary's
    // nearest kind is `branch_head`.
    let events = [
        loose_ev(EventKind::Commit, 5.0),
        loose_ev(EventKind::BranchHead, 5.0),
    ];
    let graph = layout(&events);

    let kinds: Vec<EventKind> = graph.strip.iter().map(|row| row.kind).collect();
    assert_eq!(kinds, vec![EventKind::BranchHead, EventKind::Commit]);
    assert!(graph.strip.iter().all(|row| !row.open));

    let kinds = [EventKind::Commit];
    let opened = Graph::layout(&events, &opts().loose_open(LooseOpen::Kinds(&kinds)));
    let open: Vec<(EventKind, bool)> = opened
        .strip
        .iter()
        .map(|row| (row.kind, row.open))
        .collect();
    assert_eq!(
        open,
        vec![(EventKind::BranchHead, false), (EventKind::Commit, true)]
    );
    assert_eq!(opened.loose.len(), 1);
    assert_eq!(opened.loose[0].kind, EventKind::Commit);
}

// -- rows -------------------------------------------------------------------

#[test]
fn a_row_that_was_touched_later_sits_above_one_touched_earlier() {
    let graph = layout(&[ev("a", 0.0), ev("a", 100.0), ev("b", 5.0), ev("b", 150.0)]);

    assert!(lane(&graph, "b") < lane(&graph, "a"));
}

fn thirty_items() -> Vec<Event> {
    (1..=30)
        .map(|n| ev(&format!("imp-{n}"), f64::from(n) * 4.0))
        .collect()
}

#[test]
fn a_limit_keeps_the_newest_items_and_counts_the_rest() {
    let events = thirty_items();
    let graph = Graph::layout(&events, &opts().limit(Some(20)));

    assert_eq!(graph.items.len(), 20);
    assert_eq!(graph.hidden, 10);
    assert_eq!(graph.total_items, 30);
    assert!(graph.items.iter().any(|item| item.id == "imp-30"));
    assert!(graph.items.iter().all(|item| item.id != "imp-1"));
    assert_eq!(layout(&events).hidden, 0);
}

#[test]
fn show_all_appears_only_when_rows_are_hidden_and_counts_every_item() {
    let events = thirty_items();
    let capped = Graph::layout(&events, &opts().limit(Some(20)));
    assert!(capped.hidden > 0);
    assert_eq!(show_all_label(false, capped.total_items), "show all (30)");

    let few = layout(&events[..5]);
    assert_eq!(few.hidden, 0);
}

// -- segment labels ---------------------------------------------------------

#[test]
fn both_ends_of_a_segment_are_labelled_a_single_dot_is_labelled_once() {
    let graph = layout(&[ev("imp-9", 10.0), ev("imp-9", 30.0), ev("imp-3", 50.0)]);

    let spanned = item(&graph, "imp-9");
    let single = item(&graph, "imp-3");
    assert!(spanned.start_label_x.is_some_and(|at| at < spanned.x0));
    assert_eq!(single.start_label_x, None);
}

#[test]
fn items_sharing_a_row_never_overlap_start_label_included() {
    let events: Vec<Event> = (1..=12)
        .flat_map(|n| {
            [n * 7, n * 7 + 9]
                .into_iter()
                .map(move |h| ev(&format!("imp-{n}"), f64::from(h)))
        })
        .collect();
    let graph = layout(&events);

    assert_rows_do_not_overlap(&graph, |item| {
        let width = label_width(&item.id);
        (
            item.start_label_x.unwrap_or(item.x0) - width,
            item.label_x + width,
        )
    });
}

fn label_width(id: &str) -> f64 {
    id.chars().count() as f64 * 6.6
}

/// Within each row, items sorted by their left end have `right_i < left_(i+1)`.
fn assert_rows_do_not_overlap(graph: &Graph, extent: impl Fn(&Item) -> (f64, f64)) {
    let mut by_lane: HashMap<usize, Vec<(f64, f64)>> = HashMap::new();
    for item in &graph.items {
        by_lane.entry(item.lane).or_default().push(extent(item));
    }
    assert!(!by_lane.is_empty());
    for (lane, mut spans) in by_lane {
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        for pair in spans.windows(2) {
            assert!(
                pair[0].1 < pair[1].0,
                "row {lane}: {:?} runs under {:?}",
                pair[0],
                pair[1]
            );
        }
    }
}

// -- the drawn graph ----------------------------------------------------------

#[test]
fn every_branch_carries_the_id_its_link_is_built_from() {
    let graph = layout(&[ev("imp-7", 0.0), ev("imp-7", 10.0), ev("imp-9", 50.0)]);

    let ids: Vec<&str> = graph.items.iter().map(|item| item.id.as_str()).collect();
    assert!(ids.contains(&"imp-7") && ids.contains(&"imp-9"));
}

#[test]
fn a_window_with_nothing_in_it_says_so_instead_of_drawing_an_empty_frame() {
    let graph = layout(&[]);

    assert!(graph.is_empty());
    assert_eq!(graph.aria_label(), "0 items in 0 rows");
}

// -- forks, clearance and start labels ----------------------------------------

/// The text a tip node draws past its lane's end: half the dot, the gap and
/// the caption, at the graph's monospace advance.
fn fork_tip_end(fork: &Fork) -> f64 {
    fork.x1 + 7.5 + fork.label.chars().count() as f64 * 6.6
}

#[test]
fn items_sharing_a_row_never_run_under_each_others_fork_tip_nodes() {
    // Spacings sweep the window between "starts after the previous id label
    // ends" and "starts after its fork caption ends".
    let steps = [0.6, 0.8, 1.0, 1.2, 1.4, 1.6, 1.8, 2.0, 2.4, 2.8];
    let mut start = 0.0;
    let mut events = Vec::new();
    let mut lanes = Vec::new();
    for n in 1..=40 {
        start += steps[(n - 1) % steps.len()];
        let id = format!("imp-{n}");
        events.push(ev(&id, start));
        events.push(ev(&id, start + 0.2));
        lanes.push(fork_lane(&id, start, start + 0.2, "orchestrator"));
    }
    let tree = tree(lanes, vec![]);
    let graph = with_tree(&events, &tree);

    assert_eq!(graph.items.len(), 40);
    assert_rows_do_not_overlap(&graph, |item| {
        let right = item
            .forks
            .iter()
            .map(fork_tip_end)
            .fold(item.label_x + label_width(&item.id), f64::max);
        (item.start_label_x.unwrap_or(item.x0), right)
    });
}

#[test]
fn the_right_margin_leaves_room_for_the_widest_fork_tip_node() {
    let lane = fork_lane("imp-1", 160.0, 160.0, "orchestrator");
    let tree = tree(vec![lane], vec![]);
    let graph = with_tree(&[ev("imp-1", 0.0), ev("imp-1", 160.0)], &tree);

    let fork = &graph.items[0].forks[0];
    assert!(fork_tip_end(fork) <= f64::from(graph.width));
}

/// The last instant of the window: the axis ends here, at `width - right`.
const WINDOW_END_HOURS: f64 = 168.0;

#[test]
fn the_right_margin_holds_a_fork_tip_node_that_ends_at_the_very_end_of_the_axis() {
    let lane = fork_lane("imp-1", 160.0, WINDOW_END_HOURS, "orchestrator");
    let tree = tree(vec![lane], vec![]);
    let graph = with_tree(&[ev("imp-1", 0.0), ev("imp-1", WINDOW_END_HOURS)], &tree);

    let fork = &graph.items[0].forks[0];
    let reserve = 6.0 + 7.5 + 12.0 * 6.6 + 5.0;
    assert!(
        (fork.x1 - (1400.0 - reserve)).abs() < 1e-6,
        "x1 {}",
        fork.x1
    );
    assert!(fork_tip_end(fork) + 5.0 <= f64::from(graph.width));
}

#[test]
fn the_right_margin_holds_a_seven_character_id_at_the_very_end_of_the_axis() {
    let graph = layout(&[
        ev("imp-123", 0.0),
        ev("imp-123", WINDOW_END_HOURS),
        ev("imp-123", WINDOW_END_HOURS),
    ]);

    let it = &graph.items[0];
    assert!(it.dots.last().is_some_and(|dot| dot.count >= 2));
    assert!(it.label_x + 7.0 * 6.6 <= f64::from(graph.width));
}

fn outer(dot: &Dot) -> f64 {
    if dot.count >= 2 {
        5.75
    } else {
        3.75
    }
}

#[test]
fn a_tip_label_stands_at_least_1px_clear_of_its_dot_a_cluster_included() {
    // Two events at one instant merge into a cluster, the wider glyph.
    let graph = layout(&[
        ev("imp-1", 0.0),
        ev("imp-1", 50.0),
        ev("imp-1", 50.0),
        ev("imp-2", 10.0),
    ]);

    for it in &graph.items {
        let last = it.dots.last().expect("dots");
        assert!(it.label_x - it.x1 >= outer(last) + 1.0);
    }
    assert!(graph
        .items
        .iter()
        .any(|it| it.dots.last().is_some_and(|dot| dot.count >= 2)));
}

#[test]
fn a_start_label_stands_at_least_1px_clear_of_the_first_dot() {
    let graph = layout(&[ev("imp-1", 0.0), ev("imp-1", 0.0), ev("imp-1", 150.0)]);

    let it = &graph.items[0];
    let first = it.dots.first().expect("dots");
    assert!(first.count >= 2);
    assert!(it
        .start_label_x
        .is_some_and(|at| it.x0 - at >= outer(first) + 1.0));
}

#[test]
fn an_id_is_drawn_at_the_start_too_only_when_its_line_is_at_least_120px_long() {
    // Hourly filler keeps the axis evenly busy, so pixels follow hours.
    let mut events = vec![
        ev("imp-1", 0.0),
        ev("imp-1", 1.0),
        ev("imp-2", 0.0),
        ev("imp-2", 150.0),
    ];
    events.extend((0..=150).map(|h| ev("imp-9", f64::from(h))));
    let graph = layout(&events);

    let mut seen = BTreeSet::new();
    for it in &graph.items {
        let long = it.x1 - it.x0 >= 120.0;
        assert_eq!(it.start_label_x.is_some(), long, "{}", it.id);
        seen.insert(long);
    }
    assert_eq!(seen, BTreeSet::from([false, true]));
}

// -- geometry the painter reads -----------------------------------------------

#[test]
fn the_empty_graph_is_one_axis_row_tall() {
    let graph = layout(&[]);

    assert_eq!(
        (
            graph.width,
            graph.rows,
            graph.lanes,
            graph.total_items,
            graph.hidden
        ),
        (1400, 0, 0, 0, 0)
    );
    assert_eq!(graph.axis_y, 4.0);
    assert_eq!(graph.height, 32);
    assert_eq!(graph.loose_y, None);
    assert!(graph.strip.is_empty());
}

#[test]
fn the_window_ends_make_one_gap_across_an_empty_week() {
    let graph = layout(&[]);

    assert_eq!(graph.gaps.len(), 1);
    assert_eq!(graph.gaps[0].ms, 7 * 86_400_000);
}

#[test]
fn closed_strip_rows_still_occupy_a_row_each_above_the_items() {
    let events = [
        loose_ev(EventKind::Commit, 5.0),
        loose_ev(EventKind::BranchHead, 5.0),
        loose_ev(EventKind::PrMerged, 5.0),
        ev("imp-1", 5.0),
    ];
    let graph = layout(&events);

    assert_eq!(
        graph.strip.iter().map(|row| row.y0).collect::<Vec<_>>(),
        vec![0.0, 14.0, 28.0]
    );
    assert_eq!(graph.loose_y, Some(7.0));
    assert_eq!(graph.items[0].y, 3.0 * 14.0 + 7.0);
    assert_eq!(graph.axis_y, 3.0 * 14.0 + 14.0 + 4.0);
    assert_eq!(graph.height, 3 * 14 + 14 + 4 + 28);
}

#[test]
fn open_strip_rows_sit_on_the_middle_of_their_row() {
    let events = [
        loose_ev(EventKind::Commit, 5.0),
        loose_ev(EventKind::BranchHead, 5.0),
    ];
    let graph = Graph::layout(&events, &opts().loose_open(true));

    assert_eq!(
        graph
            .loose
            .iter()
            .map(|row| (row.kind, row.y))
            .collect::<Vec<_>>(),
        vec![(EventKind::BranchHead, 7.0), (EventKind::Commit, 21.0),]
    );
}

#[test]
fn extra_loose_events_join_the_strip_like_item_less_ones() {
    let extra = [loose_ev(EventKind::Commit, 6.0)];
    let events = [loose_ev(EventKind::Commit, 5.0), ev("imp-1", 5.0)];
    let graph = Graph::layout(&events, &opts().loose(&extra));

    assert_eq!(graph.strip[0].count, 2);
}

#[test]
fn the_hit_rectangle_runs_label_to_label() {
    let graph = layout(&[ev("imp-9", 10.0), ev("imp-9", 30.0), ev("imp-3", 50.0)]);

    let spanned = item(&graph, "imp-9");
    let left_edge = spanned.start_label_x.expect("start label") - 5.0 * 6.6;
    let rect = spanned.hit_rect();
    assert_eq!(rect.x, left_edge - 4.0);
    assert_eq!(rect.y, spanned.y - 6.0);
    assert_eq!(rect.h, 12.0);
    assert_eq!(rect.w, spanned.label_x - left_edge + 5.0 * 6.6 + 8.0);

    let single = item(&graph, "imp-3");
    assert_eq!(single.left_edge(), single.x0);
}

#[test]
fn an_items_tooltip_names_id_title_and_status() {
    let graph = layout(&[
        ev("imp-1", 0.0).title("fix the thing").status("review"),
        ev("imp-2", 5.0),
        ev("imp-3", 6.0).title("").status("done"),
    ]);

    assert_eq!(
        item(&graph, "imp-1").tooltip(),
        "imp-1 · fix the thing · review"
    );
    assert_eq!(item(&graph, "imp-2").tooltip(), "imp-2");
    assert_eq!(item(&graph, "imp-3").tooltip(), "imp-3 · done");
}

#[test]
fn the_newest_title_wins_and_an_empty_one_counts() {
    let graph = layout(&[
        ev("imp-1", 0.0).title("old"),
        ev("imp-1", 1.0).title("new"),
        ev("imp-1", 2.0),
        ev("imp-2", 0.0).title("kept"),
        ev("imp-2", 1.0).title(""),
    ]);

    assert_eq!(item(&graph, "imp-1").title.as_deref(), Some("new"));
    assert_eq!(item(&graph, "imp-2").title.as_deref(), Some(""));
}

#[rstest]
#[case("done", Tone::Green, true)]
#[case("review", Tone::Yellow, false)]
#[case("blocked", Tone::Red, false)]
#[case("in_progress", Tone::Blue, false)]
#[case("claimed", Tone::Blue, false)]
#[case("open", Tone::Hollow, false)]
#[case("whatever", Tone::Hollow, false)]
fn a_status_colours_the_line_and_only_done_lands(
    #[case] status: &str,
    #[case] tone: Tone,
    #[case] done: bool,
) {
    let graph = layout(&[ev("imp-1", 0.0).status(status)]);

    assert_eq!((graph.items[0].tone, graph.items[0].done), (tone, done));
}

// -- fork lanes and elbows -----------------------------------------------------

#[test]
fn a_fork_lane_takes_the_row_under_its_item_and_its_elbows_join_the_line() {
    let lane = fork_lane("imp-1", 10.0, 40.0, "omp");
    let tree = tree(
        vec![lane.clone()],
        vec![
            edge(EdgeKind::Fork, &lane, 10.0),
            edge(EdgeKind::Merge, &lane, 40.0),
        ],
    );
    let graph = with_tree(
        &[ev("imp-1", 0.0), ev("imp-1", 40.0), ev("imp-1", 90.0)],
        &tree,
    );

    assert_eq!((graph.lanes, graph.rows), (1, 2));
    let it = &graph.items[0];
    assert_eq!(it.forks.len(), 1);
    let fork = &it.forks[0];
    assert_eq!(fork.id, "imp-1/collab/omp");
    assert_eq!(
        (fork.kind, fork.label.as_str(), fork.title.as_str()),
        (LaneKind::Collab, "omp", "collab omp")
    );
    assert_eq!(fork.y, it.y + 14.0);
    assert!(fork.x1 > fork.x0 && !fork.dead);
    assert_eq!(fork.tone, Tone::Hollow);
    assert_eq!(fork.node_left(), round_half_up(fork.x1 - 3.5, 1));

    assert_eq!(it.edges.len(), 2);
    assert_eq!(it.edges[0].kind, EdgeKind::Fork);
    assert_eq!(
        it.edges[0].d,
        format!(
            "M{} {}V{}H{}",
            fmt1(fork.x0),
            fmt1(it.y),
            fmt1(fork.y),
            fmt1(fork.x0)
        )
    );
    assert_eq!(it.edges[1].kind, EdgeKind::Merge);
    assert_eq!(
        it.edges[1].d,
        format!(
            "M{} {}H{}V{}",
            fmt1(fork.x1),
            fmt1(fork.y),
            fmt1(fork.x1),
            fmt1(it.y)
        )
    );
    // The graph is 28px of axis under 2 rows.
    assert_eq!(graph.axis_y, 2.0 * 14.0 + 4.0);
}

#[test]
fn path_numbers_print_one_decimal() {
    let lane = fork_lane("imp-1", 0.0, 40.0, "omp");
    let tree = tree(vec![lane.clone()], vec![edge(EdgeKind::Fork, &lane, 0.0)]);
    let graph = with_tree(&[ev("imp-1", 0.0), ev("imp-1", 40.0)], &tree);

    let d = &graph.items[0].edges[0].d;
    assert!(d.starts_with('M') && d.contains('V') && d.contains('H'));
    for number in d
        .split(|c: char| c.is_ascii_alphabetic() || c == ' ')
        .filter(|n| !n.is_empty())
    {
        let (_, fraction) = number.split_once('.').expect("a decimal point");
        assert_eq!(fraction.len(), 1, "{d}");
    }
    // The item's line is at y 7 and its fork lane one row under.
    assert!(
        d.contains("M59.9 7.0V21.0H59.9") || d.contains("M60.0 7.0V21.0H60.0"),
        "{d}"
    );
}

#[test]
fn a_lane_that_lost_is_dead_and_so_are_its_elbows() {
    let mut lane = fork_lane("imp-1", 10.0, 40.0, "omp");
    lane.chosen = Some(false);
    let tree = tree(vec![lane.clone()], vec![edge(EdgeKind::Fork, &lane, 10.0)]);
    let graph = with_tree(&[ev("imp-1", 0.0), ev("imp-1", 90.0)], &tree);

    let it = &graph.items[0];
    assert!(it.forks[0].dead && it.edges[0].dead);
    assert_eq!(it.forks[0].chosen, Some(false));
}

#[test]
fn an_edge_naming_no_lane_of_the_item_is_dropped() {
    let lane = fork_lane("imp-1", 10.0, 40.0, "omp");
    let stray = Edge {
        kind: EdgeKind::Fork,
        lane: "imp-1/collab/nobody".into(),
        item_id: "imp-1".into(),
        at: at(10.0),
    };
    let elsewhere = Edge {
        item_id: "imp-2".into(),
        ..edge(EdgeKind::Fork, &lane, 10.0)
    };
    let tree = tree(
        vec![lane.clone()],
        vec![stray, elsewhere, edge(EdgeKind::Fork, &lane, 10.0)],
    );
    let graph = with_tree(&[ev("imp-1", 0.0), ev("imp-1", 90.0)], &tree);

    assert_eq!(graph.items[0].edges.len(), 1);
}

#[test]
fn of_two_lanes_sharing_an_id_the_later_one_takes_the_edge() {
    let first = fork_lane("imp-1", 10.0, 20.0, "omp");
    let second = Lane {
        id: first.id.clone(),
        ..fork_lane("imp-1", 40.0, 50.0, "omp")
    };
    let tree = tree(
        vec![first.clone(), second.clone()],
        vec![edge(EdgeKind::Fork, &first, 40.0)],
    );
    let graph = with_tree(&[ev("imp-1", 0.0), ev("imp-1", 90.0)], &tree);

    let it = &graph.items[0];
    assert_eq!(it.forks.len(), 2);
    let later = &it.forks[1];
    assert_eq!(
        it.edges[0].d,
        format!(
            "M{} {}V{}H{}",
            fmt1(later.x0),
            fmt1(it.y),
            fmt1(later.y),
            fmt1(later.x0)
        )
    );
}

#[test]
fn a_lane_ending_before_it_starts_is_drawn_no_shorter_than_a_point() {
    let lane = fork_lane("imp-1", 40.0, 10.0, "omp");
    let tree = tree(vec![lane], vec![]);
    let graph = with_tree(&[ev("imp-1", 0.0), ev("imp-1", 90.0)], &tree);

    let fork = &graph.items[0].forks[0];
    assert_eq!(fork.x1, fork.x0);
}

#[test]
fn a_row_is_as_tall_as_the_most_forks_any_item_in_it_has() {
    // imp-1 and imp-2 overlap (two rows); the newer, imp-2, sits on top and
    // owns two forks, so imp-1's row starts three rows down.
    let tree = tree(
        vec![
            fork_lane("imp-2", 20.0, 30.0, "a"),
            fork_lane("imp-2", 20.0, 30.0, "b"),
        ],
        vec![],
    );
    let events = [
        ev("imp-1", 0.0),
        ev("imp-1", 80.0),
        ev("imp-2", 10.0),
        ev("imp-2", 90.0),
    ];
    let graph = with_tree(&events, &tree);

    assert_eq!(graph.lanes, 2);
    assert_eq!(graph.rows, 4);
    let (top, bottom) = (item(&graph, "imp-2"), item(&graph, "imp-1"));
    assert_eq!((top.lane, bottom.lane), (0, 1));
    assert_eq!(top.y, 7.0);
    assert_eq!(bottom.y, 3.0 * 14.0 + 7.0);
    assert_eq!(
        top.forks.iter().map(|fork| fork.y).collect::<Vec<_>>(),
        vec![21.0, 35.0]
    );
    assert_eq!(graph.axis_y, 4.0 * 14.0 + 4.0);
}

#[test]
fn fork_lanes_of_items_cut_by_the_limit_are_not_drawn_and_need_no_rows() {
    let tree = tree(vec![fork_lane("imp-1", 4.0, 8.0, "omp")], vec![]);
    let events = thirty_items();
    let graph = Graph::layout(&events, &opts().limit(Some(20)).tree(&tree));

    assert_eq!(graph.hidden, 10);
    assert!(graph.items.iter().all(|item| item.forks.is_empty()));
}

// -- open questions -----------------------------------------------------------

#[test]
fn q2_a_trailing_event_without_a_status_keeps_the_newest_status_the_item_had() {
    let graph = layout(&[
        ev("imp-1", 0.0).status("in_progress"),
        ev("imp-1", 30.0).status("blocked"),
        ev("imp-1", 40.0),
    ]);

    assert_eq!(graph.items[0].status.as_deref(), Some("blocked"));
    assert_eq!(graph.items[0].tone, Tone::Red);
}

#[test]
fn q2_a_caller_supplied_status_wins_over_the_events() {
    let statuses = HashMap::from([("imp-1".to_string(), "done".to_string())]);
    let events = [
        ev("imp-1", 0.0).status("blocked"),
        ev("imp-2", 1.0).status("blocked"),
    ];
    let graph = Graph::layout(&events, &opts().statuses(&statuses));

    let (one, two) = (item(&graph, "imp-1"), item(&graph, "imp-2"));
    assert_eq!(
        (one.status.as_deref(), one.tone, one.done),
        (Some("done"), Tone::Green, true)
    );
    assert_eq!(
        (two.status.as_deref(), two.tone, two.done),
        (Some("blocked"), Tone::Red, false)
    );
}

#[test]
fn q4_a_window_start_before_the_first_event_shows_as_a_gap() {
    let graph = layout(&[ev("imp-1", 100.0), ev("imp-1", 101.0)]);

    assert!(graph.gaps.iter().any(|gap| gap.ms == 100 * 3_600_000));
}

#[test]
fn q5_rows_with_equal_newest_times_keep_the_lower_row_first() {
    // Two identical items overlap, so they take rows 0 and 1 in id order and
    // keep that order when their newest times tie.
    let events = [
        ev("imp-a", 0.0),
        ev("imp-a", 50.0),
        ev("imp-b", 0.0),
        ev("imp-b", 50.0),
    ];
    let graph = layout(&events);

    assert_eq!(
        graph
            .items
            .iter()
            .map(|item| (item.id.as_str(), item.lane))
            .collect::<Vec<_>>(),
        vec![("imp-a", 0), ("imp-b", 1)]
    );
}

#[test]
fn q5_a_cap_keeps_the_larger_id_among_items_last_touched_at_one_instant() {
    let events = [ev("imp-1", 5.0), ev("imp-2", 5.0), ev("imp-3", 5.0)];
    let graph = Graph::layout(&events, &opts().limit(Some(2)));

    let mut ids: Vec<&str> = graph.items.iter().map(|item| item.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec!["imp-2", "imp-3"]);
    assert_eq!((graph.total_items, graph.hidden), (3, 1));
}

#[test]
fn q5_items_starting_together_are_listed_in_id_order() {
    let events = [ev("imp-b", 5.0), ev("imp-a", 5.0), ev("imp-c", 5.0)];
    let graph = layout(&events);

    assert_eq!(
        graph
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["imp-a", "imp-b", "imp-c"]
    );
}

// -- window and options ---------------------------------------------------------

#[test]
fn a_query_interval_in_seconds_becomes_the_axis_window() {
    let open = Range::from_secs(1_000, None, 5_000);
    assert_eq!((open.since, open.now), (1_000_000_000, 5_000_000_000));

    let closed = Range::from_secs(1_000, Some(3_000), 5_000);
    assert_eq!((closed.since, closed.now), (1_000_000_000, 3_000_000_000));
}

#[test]
fn layout_is_pure_the_same_inputs_give_the_same_graph() {
    let events = [
        ev("imp-1", 0.0),
        ev("imp-1", 20.0).status("blocked"),
        loose_ev(EventKind::Commit, 3.0),
        ev("imp-2", 5.0),
    ];
    let tree = tree(vec![fork_lane("imp-1", 2.0, 9.0, "omp")], vec![]);

    assert_eq!(with_tree(&events, &tree), with_tree(&events, &tree));
}

#[test]
fn a_narrower_container_squeezes_every_item_inside_it_when_there_is_room() {
    let events: Vec<Event> = (1..=5)
        .map(|n| ev(&format!("imp-{n}"), f64::from(n) * 20.0))
        .collect();
    let narrow = Graph::layout(&events, &Options::new(700, Range::new(since(), now())));

    assert_eq!(narrow.width, 700);
    assert_eq!(narrow.items.len(), 5);
    assert!(narrow.items.iter().all(|item| item.x1 <= 700.0 - 59.95));
    assert!(narrow.items.iter().all(|item| item.x0 >= 59.95 - 1e-6));
}

#[rstest]
#[case(1200)]
#[case(733)]
#[case(1900)]
fn the_graph_is_laid_out_at_the_width_it_is_given(#[case] width: u32) {
    let events = [ev("imp-1", 0.0), ev("imp-1", 50.0)];
    let graph = Graph::layout(&events, &Options::new(width, Range::new(since(), now())));

    assert_eq!(graph.width, width);
    assert!(graph.items[0].x1 <= f64::from(width));
}

#[test]
fn with_no_tree_an_item_has_no_forks_and_no_elbows() {
    let events = [ev("imp-1", 0.0), ev("imp-1", 50.0)];
    let graph = layout(&events);

    assert!(graph.items[0].forks.is_empty() && graph.items[0].edges.is_empty());
    assert_eq!(graph.rows, graph.lanes);
}

#[test]
fn q12_events_within_one_millisecond_share_an_x_but_keep_their_microsecond_order() {
    let first = Event {
        at: at(5.0) + 100,
        ..ev("imp-1", 5.0).summary("first")
    };
    let second = Event {
        at: at(5.0) + 600,
        ..ev("imp-1", 5.0).summary("second")
    };
    let forward = layout(&[first.clone(), second.clone()]);
    let backward = layout(&[second, first]);

    let dot = &forward.items[0].dots[0];
    assert_eq!(dot.count, 2);
    assert!(dot.title.find("first") < dot.title.find("second"));
    assert_eq!(forward.items[0].dots, backward.items[0].dots);
}
