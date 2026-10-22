//! The view pipeline over the toolbar's state: `activity_view_test.exs` and the
//! mode tests of `activity_live_test.exs`, on the models.

use std::collections::HashMap;

use super::super::view;
use super::super::*;
use super::fixtures::*;
use crate::widgets::trace::model::{Event, EventKind, Lane, LaneKind, SessionTree};
use crate::widgets::trace::query::{Filters, GraphAction, GraphState};
use crate::widgets::trace::time;

/// The clock of the suites, in epoch seconds: [`now`].
fn clock() -> i64 {
    now() / SEC
}

fn state_with(window: &str) -> GraphState {
    GraphState {
        window: window.to_string(),
        ..GraphState::default()
    }
}

fn filtered(filters: Filters) -> GraphState {
    GraphState {
        window: "7d".into(),
        filters,
        ..GraphState::default()
    }
}

fn ids(graph: &Graph) -> Vec<&str> {
    let mut ids: Vec<&str> = graph.items.iter().map(|item| item.id.as_str()).collect();
    ids.sort_unstable();
    ids
}

// -- graph --------------------------------------------------------------------

#[test]
fn the_graph_is_laid_out_at_1200_then_at_the_width_the_container_reports() {
    let events = [ev("imp-1", 0.0), ev("imp-1", 50.0)];
    let mut state = state_with("7d");

    assert_eq!(view::graph(&events, &state, None, clock()).width, 1200);
    for reported in [733, 1900] {
        state.reduce(GraphAction::GraphWidth(reported));
        assert_eq!(
            view::graph(&events, &state, None, clock()).width,
            reported as u32
        );
    }
}

#[test]
fn a_zero_report_changes_nothing() {
    let events = [ev("imp-1", 0.0)];
    let mut state = state_with("7d");
    state.reduce(GraphAction::GraphWidth(733));
    state.reduce(GraphAction::GraphWidth(0));

    assert_eq!(view::graph(&events, &state, None, clock()).width, 733);
}

#[test]
fn the_window_bounds_the_read_and_is_half_open() {
    // `7d` is [now - 7d, now): an event a second before it is out, one at its
    // start is in.
    let before = Event {
        at: since() - SEC,
        ..ev("imp-before", 0.0)
    };
    let at_start = ev("imp-start", 0.0);
    let inside = ev("imp-inside", 40.0);
    let events = [before, at_start, inside];

    let graph = view::graph(&events, &state_with("7d"), None, clock());

    assert_eq!(ids(&graph), vec!["imp-inside", "imp-start"]);
}

#[test]
fn a_closed_window_ends_at_its_until_and_the_axis_with_it() {
    // 2026-10-02 is [Oct 2 00:00, Oct 3 00:00); `now` is no longer the end.
    let noon = Event {
        at: time::from_utc(2026, 10, 2, 12, 0, 0),
        ..ev("imp-1", 0.0)
    };
    let next_day = Event {
        at: time::from_utc(2026, 10, 3, 0, 0, 0),
        ..ev("imp-2", 0.0)
    };
    let early = Event {
        at: time::from_utc(2026, 10, 1, 23, 59, 59),
        ..ev("imp-3", 0.0)
    };
    let events = [early, noon, next_day];

    let graph = view::graph(&events, &state_with("2026-10-02"), None, clock());

    assert_eq!(ids(&graph), vec!["imp-1"]);
    let idle: Vec<i64> = graph.gaps.iter().map(|gap| gap.ms).collect();
    assert_eq!(idle, vec![12 * 3_600_000, 12 * 3_600_000]);
}

#[test]
fn a_relative_window_ends_at_now_and_starts_a_window_before_it() {
    let graph = view::graph(&[], &state_with("24h"), None, clock());

    assert!(graph.is_empty());
    assert_eq!(
        graph.gaps.iter().map(|gap| gap.ms).collect::<Vec<_>>(),
        vec![24 * 3_600_000]
    );
}

#[test]
fn the_row_cap_holds_until_show_all_lifts_it() {
    let events: Vec<Event> = (1..=30)
        .map(|n| ev(&format!("imp-{n}"), f64::from(n) * 4.0))
        .collect();
    let mut state = state_with("7d");

    let capped = view::graph(&events, &state, None, clock());
    assert_eq!(
        (capped.items.len(), capped.hidden, capped.total_items),
        (20, 10, 30)
    );

    state.reduce(GraphAction::ToggleShowAll);
    let all = view::graph(&events, &state, None, clock());
    assert_eq!((all.item_count(), all.hidden, all.total_items), (30, 0, 30));
}

#[test]
fn the_filters_narrow_the_graph_and_an_unknown_agent_leaves_it_empty() {
    let events = [
        Event {
            agent: Some("omp".into()),
            ..ev("imp-1", 5.0)
        },
        ev("imp-2", 6.0),
        loose_ev(EventKind::Commit, 7.0),
    ];

    let by_agent = view::graph(
        &events,
        &filtered(Filters {
            agent: Some("omp".into()),
            ..Filters::default()
        }),
        None,
        clock(),
    );
    assert_eq!(ids(&by_agent), vec!["imp-1"]);
    assert!(by_agent.strip.is_empty());

    let by_item = view::graph(
        &events,
        &filtered(Filters {
            item: Some("imp-2".into()),
            ..Filters::default()
        }),
        None,
        clock(),
    );
    assert_eq!(ids(&by_item), vec!["imp-2"]);

    let by_kind = view::graph(
        &events,
        &filtered(Filters {
            kind: Some("commit".into()),
            ..Filters::default()
        }),
        None,
        clock(),
    );
    assert!(by_kind.items.is_empty());
    assert_eq!(by_kind.strip.len(), 1);

    let nobody = view::graph(
        &events,
        &filtered(Filters {
            agent: Some("nobody".into()),
            ..Filters::default()
        }),
        None,
        clock(),
    );
    assert!(nobody.is_empty());
}

#[test]
fn a_query_line_sets_filters_window_and_tree_in_one_go() {
    let events = [
        ev("imp-1", 5.0),
        Event {
            agent: Some("omp".into()),
            ..ev("imp-2", 6.0)
        },
    ];
    let mut state = GraphState::default();
    state.reduce(GraphAction::Query("agent:omp range:7d".into()));

    let graph = view::graph(&events, &state, None, clock());

    assert_eq!(ids(&graph), vec!["imp-2"]);
}

fn omp_fork() -> SessionTree {
    let lane = Lane {
        id: "imp-1/collab/omp".into(),
        item_id: "imp-1".into(),
        kind: LaneKind::Collab,
        label: "omp".into(),
        from: at(10.0),
        to: at(40.0),
        chosen: None,
        parent: None,
        agent: Some("omp".into()),
    };
    SessionTree {
        agent: None,
        lanes: vec![lane],
        edges: vec![],
    }
}

#[test]
fn forks_are_drawn_only_when_the_tree_toggle_is_on() {
    let events = [ev("imp-1", 0.0), ev("imp-1", 90.0)];
    let tree = omp_fork();
    let mut state = state_with("7d");

    let off = view::graph(&events, &state, Some(&tree), clock());
    assert!(off.items[0].forks.is_empty());

    state.reduce(GraphAction::ToggleTree);
    let on = view::graph(&events, &state, Some(&tree), clock());
    assert_eq!(on.items[0].forks.len(), 1);
    assert_eq!(on.rows, 2);

    let no_tree = view::graph(&events, &state, None, clock());
    assert!(no_tree.items[0].forks.is_empty());
}

#[test]
fn the_strip_rows_the_state_expanded_are_open_and_unknown_names_are_ignored() {
    let events = [
        loose_ev(EventKind::Commit, 5.0),
        loose_ev(EventKind::BranchHead, 5.0),
        ev("imp-1", 5.0),
    ];
    let mut state = state_with("7d");
    state.loose_kinds = vec!["commit".into(), "bogus".into()];

    let graph = view::graph(&events, &state, None, clock());

    let open: Vec<(EventKind, bool)> = graph.strip.iter().map(|row| (row.kind, row.open)).collect();
    assert_eq!(
        open,
        vec![(EventKind::BranchHead, false), (EventKind::Commit, true)]
    );
    assert_eq!(graph.loose.len(), 1);
}

// -- sequence -----------------------------------------------------------------

fn agents(sequence: &Sequence) -> Vec<&str> {
    sequence
        .lanes
        .iter()
        .map(|lane| lane.agent.as_str())
        .collect()
}

#[test]
fn the_sequence_is_windowed_and_filtered() {
    let events = [
        Event {
            agent: Some("worker".into()),
            ..ev("imp-1", 10.0)
        },
        Event {
            at: since() - SEC,
            agent: Some("old".into()),
            ..ev("imp-1", 0.0)
        },
        Event {
            agent: Some("omp".into()),
            ..ev("imp-1", 20.0)
        },
    ];

    let all = view::sequence(&events, &state_with("7d"), clock(), None);
    assert_eq!(agents(&all), ["worker", "omp"]);
    assert_eq!(all.read, 2, "the event before the window is not read");

    let only_omp = view::sequence(
        &events,
        &filtered(Filters {
            agent: Some("omp".into()),
            ..Filters::default()
        }),
        clock(),
        Some(false),
    );
    assert_eq!(only_omp.read, 1);
    assert!(only_omp.is_empty(), "one agent meets no one");
}

#[test]
fn an_unknown_agent_leaves_the_sequence_empty() {
    let events = [ev("imp-1", 5.0)];

    let sequence = view::sequence(
        &events,
        &filtered(Filters {
            agent: Some("nobody".into()),
            ..Filters::default()
        }),
        clock(),
        None,
    );

    assert!(sequence.is_empty());
    assert_eq!((sequence.read, sequence.lanes.len()), (0, 0));
}

// -- collaboration --------------------------------------------------------------

fn two_agents_on_imp_26() -> Vec<Event> {
    vec![
        Event {
            agent: Some("a".into()),
            ..ev("imp-26", 10.0)
        },
        Event {
            agent: Some("b".into()),
            ..ev("imp-26", 11.0)
        },
    ]
}

#[test]
fn the_collaboration_ignores_the_filters_a_filtered_table_is_about_nothing() {
    let events = two_agents_on_imp_26();
    let state = filtered(Filters {
        agent: Some("a".into()),
        ..Filters::default()
    });

    let collab = view::collaboration(&events, &state, clock());

    assert_eq!(collab.pairs.len(), 1);
    assert_eq!(
        (collab.pairs[0].a.as_str(), collab.pairs[0].b.as_str()),
        ("a", "b")
    );
    assert_eq!(collab.threads.len(), 1);
}

#[test]
fn the_collaboration_reads_only_the_window() {
    let mut events = two_agents_on_imp_26();
    events.push(Event {
        at: since() - HOUR,
        agent: Some("c".into()),
        ..ev("imp-26", 0.0)
    });

    let collab = view::collaboration(&events, &state_with("7d"), clock());

    assert_eq!(collab.threads[0].agents, vec!["a", "b"]);
}

/// `imp-8`: `a` claims it and goes quiet, `b` speaks a week later; the
/// item's last `item_status` is `status`, and sits outside the 14d window.
fn held_item(status: &str) -> Vec<Event> {
    vec![
        Event {
            kind: EventKind::ItemStatus,
            ..ev("imp-8", -40.0 * 24.0).agent("a").status(status)
        },
        Event {
            kind: EventKind::ItemClaimed,
            ..ev("imp-8", -3.0 * 24.0).agent("a").status("claimed")
        },
        ev("imp-8", -3.0 * 24.0 + 1.0).agent("a"),
        ev("imp-8", 6.0 * 24.0).agent("b"),
    ]
}

#[test]
fn items_are_judged_from_every_event_the_index_holds_the_window_or_not() {
    let state = state_with("14d");

    let held = view::collaboration(&held_item("in_progress"), &state, clock());
    assert_eq!(held.threads.len(), 1);
    let thread = &held.threads[0];
    assert_eq!(thread.status.as_deref(), Some("in_progress"));
    assert!(thread.stale);
    assert_eq!(thread.stale_agent.as_deref(), Some("a"));

    let done = view::collaboration(&held_item("done"), &state, clock());
    assert!(!done.threads[0].stale);
    assert_eq!(done.threads[0].status.as_deref(), Some("done"));
}

#[test]
fn an_empty_window_is_an_empty_collaboration() {
    let collab = view::collaboration(&[], &GraphState::default(), clock());

    assert_eq!(collab.size(), 0);
}

#[test]
fn the_items_of_the_index_are_what_the_collaboration_is_judged_against() {
    let events = two_agents_on_imp_26();
    let items: HashMap<String, ItemInfo> = current_items(&events);

    assert_eq!(items["imp-26"].status, Supplied::Present(None));
}
