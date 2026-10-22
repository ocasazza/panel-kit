//! Fork lanes derived from an item's events: each rule of [`derive_forks`],
//! its invariants over random items, and what the layout does with them.

use std::collections::{BTreeMap, HashMap, HashSet};

use proptest::prelude::*;
use rstest::rstest;

use super::super::*;
use super::fixtures::*;
use crate::widgets::trace::model::{
    Edge, EdgeKind, Event, EventKind, Lane, LaneKind, SessionTree, Source,
};
use crate::widgets::trace::query::GraphState;
use EventKind::{RunCancelled, RunFailed, RunFinished, RunStarted, RunTool};

const ITEM: &str = "imp-1";

/// An event of [`ITEM`] `secs` seconds after the window opens.
fn at_s(secs: i64, kind: EventKind, agent: &str) -> Event {
    Event::new(uid(), since() + secs * SEC, kind)
        .agent(agent)
        .item(ITEM)
}

fn note(secs: i64, agent: &str) -> Event {
    at_s(secs, EventKind::Note, agent)
}

fn commit(secs: i64, agent: &str, branch: &str) -> Event {
    at_s(secs, EventKind::Commit, agent).branch(branch)
}

fn merged(secs: i64, agent: &str, branch: &str) -> Event {
    at_s(secs, EventKind::PrMerged, agent).branch(branch)
}

fn handoff(secs: i64, to: &str, from: &str) -> Event {
    at_s(secs, EventKind::Handoff, to).from(from)
}

fn run(secs: i64, kind: EventKind, agent: &str, run: &str) -> Event {
    at_s(secs, kind, agent).run(run)
}

fn status(secs: i64, agent: &str, text: &str) -> Event {
    at_s(secs, EventKind::ItemStatus, agent).status(text)
}

fn derive(events: &[Event]) -> (Vec<Lane>, Vec<Edge>) {
    let refs: Vec<&Event> = events.iter().collect();
    derive_forks(ITEM, &refs)
}

/// Kind, label, `from` and `to` in seconds, and verdict of each lane.
fn shape(lanes: &[Lane]) -> Vec<(LaneKind, &str, i64, i64, Option<bool>)> {
    lanes
        .iter()
        .map(|lane| {
            (
                lane.kind,
                lane.label.as_str(),
                (lane.from - since()) / SEC,
                (lane.to - since()) / SEC,
                lane.chosen,
            )
        })
        .collect()
}

/// Kind, lane id and instant in seconds of each elbow.
fn elbows(edges: &[Edge]) -> Vec<(EdgeKind, &str, i64)> {
    edges
        .iter()
        .map(|edge| (edge.kind, edge.lane.as_str(), (edge.at - since()) / SEC))
        .collect()
}

// -- branches -------------------------------------------------------------------

#[rstest]
#[case::merged(
    vec![commit(1, "omp", "feat/x"), merged(5, "omp", "feat/x"), note(9, "omp")],
    Some(true),
    Some(5)
)]
#[case::open(vec![commit(1, "omp", "feat/x"), note(9, "omp")], None, None)]
#[case::open_while_the_item_is_not_done(
    vec![commit(1, "omp", "feat/x"), status(9, "omp", "in_progress")],
    None,
    None
)]
#[case::dropped_once_the_item_is_done(
    vec![commit(1, "omp", "feat/x"), status(9, "omp", "done")],
    Some(false),
    None
)]
#[case::dropped_once_the_item_is_closed(
    vec![commit(1, "omp", "feat/x"), status(9, "omp", "closed")],
    Some(false),
    None
)]
#[case::dropped_when_another_branch_landed(
    vec![commit(1, "omp", "feat/x"), commit(2, "omp", "feat/y"), merged(3, "omp", "feat/y")],
    Some(false),
    None
)]
#[case::open_when_it_holds_the_last_event(
    vec![commit(1, "omp", "feat/x"), status(1, "omp", "done")],
    None,
    None
)]
fn a_branch_lane_runs_from_its_first_to_its_last_event_and_is_judged_by_how_the_item_ended(
    #[case] events: Vec<Event>,
    #[case] chosen: Option<bool>,
    #[case] merge: Option<i64>,
) {
    let (lanes, edges) = derive(&events);
    let lane = &lanes[0];
    assert_eq!(lane.id, "imp-1/branch/feat/x");
    assert_eq!((lane.kind, lane.label.as_str()), (LaneKind::Branch, "feat/x"));
    assert_eq!((lane.from - since()) / SEC, 1);
    assert_eq!(lane.chosen, chosen);
    let mine: Vec<_> = elbows(&edges)
        .into_iter()
        .filter(|(_, id, _)| *id == lane.id)
        .collect();
    let mut want = vec![(EdgeKind::Fork, lane.id.as_str(), 1)];
    want.extend(merge.map(|at| (EdgeKind::Merge, lane.id.as_str(), at)));
    assert_eq!(mine, want);
}

#[rstest]
#[case::trunk("main")]
#[case::old_trunk("master")]
#[case::empty("")]
#[case::blank("  ")]
fn the_trunk_and_blank_branches_fork_nothing(#[case] branch: &str) {
    let (lanes, edges) = derive(&[commit(1, "omp", branch), note(2, "omp")]);
    assert!(lanes.is_empty() && edges.is_empty());
}

#[test]
fn a_branch_lane_names_the_agent_of_its_first_event() {
    let events = [commit(4, "clara", "feat/x"), commit(2, "omp", "feat/x")];
    let (lanes, _) = derive(&events);
    assert_eq!(lanes[0].agent.as_deref(), Some("omp"));
}

// -- handoffs and collaborators -------------------------------------------------

#[test]
fn a_handoff_forks_a_lane_for_the_agent_that_took_the_item() {
    let events = [
        note(0, "omp"),
        handoff(5, "clara", "omp"),
        note(9, "clara"),
        note(12, "omp"),
    ];
    let (lanes, edges) = derive(&events);
    assert_eq!(
        shape(&lanes),
        [(LaneKind::Handoff, "clara", 5, 9, None)],
        "no collab lane for the agent a handoff names, none for the first agent"
    );
    assert_eq!(lanes[0].id, "imp-1/handoff/clara");
    assert_eq!(lanes[0].agent.as_deref(), Some("clara"));
    assert_eq!(elbows(&edges), [(EdgeKind::Fork, "imp-1/handoff/clara", 5)]);
}

#[test]
fn only_the_first_handoff_to_an_agent_forks_and_a_handoff_to_oneself_is_not_one() {
    let events = [
        note(0, "omp"),
        handoff(3, "clara", "omp"),
        handoff(6, "clara", "omp"),
        handoff(7, "worker", "worker"),
    ];
    let (lanes, _) = derive(&events);
    assert_eq!(
        shape(&lanes),
        [
            (LaneKind::Handoff, "clara", 3, 6, None),
            (LaneKind::Collab, "worker", 7, 7, None)
        ]
    );
}

#[test]
fn another_agent_forks_a_collab_lane_that_merges_back_while_the_item_continues() {
    let events = [
        note(0, "omp"),
        note(3, "clara"),
        note(6, "clara"),
        note(9, "omp"),
    ];
    let (lanes, edges) = derive(&events);
    assert_eq!(shape(&lanes), [(LaneKind::Collab, "clara", 3, 6, None)]);
    assert_eq!(lanes[0].id, "imp-1/collab/clara");
    assert_eq!(
        elbows(&edges),
        [
            (EdgeKind::Fork, "imp-1/collab/clara", 3),
            (EdgeKind::Merge, "imp-1/collab/clara", 6)
        ]
    );
}

#[test]
fn a_collab_lane_that_holds_the_last_event_never_merges() {
    let (lanes, edges) = derive(&[note(0, "omp"), note(3, "clara")]);
    assert_eq!(shape(&lanes), [(LaneKind::Collab, "clara", 3, 3, None)]);
    assert_eq!(elbows(&edges), [(EdgeKind::Fork, "imp-1/collab/clara", 3)]);
}

#[test]
fn unattributed_events_name_no_first_agent_and_no_lane() {
    let events = [at_s(0, EventKind::Note, "  "), note(2, "omp"), note(4, "clara")];
    let (lanes, _) = derive(&events);
    assert_eq!(shape(&lanes), [(LaneKind::Collab, "clara", 4, 4, None)]);
}

#[test]
fn a_single_agent_and_no_other_fact_derive_nothing() {
    let events = [note(0, "omp"), note(5, "omp"), commit(7, "omp", "main")];
    assert_eq!(derive(&events), (Vec::new(), Vec::new()));
    assert_eq!(derive(&[]), (Vec::new(), Vec::new()));
}

// -- runs -----------------------------------------------------------------------

#[test]
fn a_failed_run_is_a_dead_path_and_the_next_finished_run_a_kept_one_that_retries_it() {
    let events = [
        run(1, RunStarted, "omp", "r1"),
        run(2, RunTool, "omp", "r1"),
        run(3, RunFailed, "omp", "r1"),
        run(4, RunStarted, "omp", "r2"),
        run(6, RunFinished, "omp", "r2"),
    ];
    let (lanes, edges) = derive(&events);
    assert_eq!(
        shape(&lanes),
        [
            (LaneKind::Path, "r1", 1, 3, Some(false)),
            (LaneKind::Path, "r2", 4, 6, Some(true))
        ]
    );
    assert_eq!(lanes[0].parent, None);
    assert_eq!(lanes[1].parent.as_deref(), Some("imp-1/path/r1"));
    assert_eq!(
        elbows(&edges),
        [
            (EdgeKind::Fork, "imp-1/path/r1", 1),
            (EdgeKind::Fork, "imp-1/path/r2", 4),
            (EdgeKind::Merge, "imp-1/path/r2", 6)
        ]
    );
}

#[test]
fn a_cancelled_run_counts_as_failed() {
    let events = [
        run(1, RunStarted, "omp", "r1"),
        run(2, RunCancelled, "omp", "r1"),
    ];
    let (lanes, _) = derive(&events);
    assert_eq!(shape(&lanes), [(LaneKind::Path, "r1", 1, 2, Some(false))]);
}

#[rstest]
#[case::a_run_that_never_ends(vec![run(1, RunStarted, "omp", "r1"), run(2, RunTool, "omp", "r1")])]
#[case::a_finished_run_nothing_retried(vec![run(1, RunStarted, "omp", "r1"), run(2, RunFinished, "omp", "r1")])]
#[case::a_failure_that_ends_no_run(vec![at_s(1, RunFailed, "omp")])]
#[case::a_run_that_failed_then_went_on(vec![
    run(1, RunFailed, "omp", "r1"),
    run(2, RunTool, "omp", "r1"),
])]
fn runs_that_do_not_end_in_failure_or_retry_a_failure_fork_nothing(#[case] events: Vec<Event>) {
    let (lanes, edges) = derive(&events);
    assert!(lanes.is_empty() && edges.is_empty(), "{lanes:?}");
}

#[test]
fn only_the_first_finished_run_after_failures_retries_the_latest_one() {
    let events = [
        run(1, RunFailed, "omp", "r1"),
        run(2, RunFailed, "omp", "r2"),
        run(3, RunFinished, "omp", "r3"),
        run(4, RunFinished, "omp", "r4"),
    ];
    let (lanes, _) = derive(&events);
    assert_eq!(
        shape(&lanes),
        [
            (LaneKind::Path, "r1", 1, 1, Some(false)),
            (LaneKind::Path, "r2", 2, 2, Some(false)),
            (LaneKind::Path, "r3", 3, 3, Some(true))
        ]
    );
    assert_eq!(lanes[2].parent.as_deref(), Some("imp-1/path/r2"));
}

#[test]
fn a_failed_run_whose_events_carry_a_branch_hangs_off_that_lane() {
    let events = [
        commit(0, "omp", "feat/x"),
        run(1, RunStarted, "omp", "r1"),
        run(2, RunFailed, "omp", "r1").branch("feat/x"),
    ];
    let (lanes, _) = derive(&events);
    assert_eq!(
        shape(&lanes),
        [
            (LaneKind::Branch, "feat/x", 0, 2, None),
            (LaneKind::Path, "r1", 1, 2, Some(false))
        ]
    );
    assert_eq!(lanes[1].parent.as_deref(), Some("imp-1/branch/feat/x"));
}

// -- the cap --------------------------------------------------------------------

fn names(lanes: &[Lane]) -> HashSet<&str> {
    lanes.iter().map(|lane| lane.label.as_str()).collect()
}

#[test]
fn ten_candidates_keep_eight_and_the_least_recent_collab_lanes_go_first() {
    let mut events = vec![note(0, "first")];
    events.extend(["b1", "b2", "b3"].iter().enumerate().map(|(i, name)| commit(10 + i as i64, "first", name)));
    events.push(handoff(20, "h1", "first"));
    events.push(handoff(21, "h2", "first"));
    events.extend(["c1", "c2", "c3", "c4", "c5"].iter().enumerate().map(|(i, name)| note(30 + 2 * i as i64, name)));
    let (lanes, edges) = derive(&events);
    assert_eq!(lanes.len(), FORK_CAP);
    assert_eq!(
        names(&lanes),
        HashSet::from(["b1", "b2", "b3", "h1", "h2", "c3", "c4", "c5"])
    );
    assert!(edges.iter().all(|edge| lanes.iter().any(|lane| lane.id == edge.lane)));
}

#[test]
fn past_the_collab_and_handoff_lanes_the_least_recent_branches_go() {
    let events: Vec<Event> = (1..=10)
        .map(|n| commit(n, "omp", &format!("feat/b{n:02}")))
        .collect();
    let (lanes, _) = derive(&events);
    let kept: Vec<&str> = lanes.iter().map(|lane| lane.label.as_str()).collect();
    assert_eq!(
        kept,
        ["feat/b03", "feat/b04", "feat/b05", "feat/b06", "feat/b07", "feat/b08", "feat/b09", "feat/b10"]
    );
}

#[test]
fn a_parent_the_cap_removed_is_the_item_line_again() {
    let mut events = vec![run(0, RunFailed, "omp", "r1").branch("feat/b1")];
    events.extend((1..=9).map(|n| commit(n, "omp", &format!("feat/b{n}"))));
    let (lanes, _) = derive(&events);
    assert_eq!(lanes.len(), FORK_CAP);
    let path = lanes.iter().find(|lane| lane.kind == LaneKind::Path).expect("path");
    assert_eq!(path.parent, None, "feat/b1 and feat/b2 were the least recent branches");
    assert!(!names(&lanes).contains("feat/b1"));
}

// -- order ---------------------------------------------------------------------

#[test]
fn lanes_come_in_the_order_they_fork() {
    let events = [
        note(0, "omp"),
        note(8, "clara"),
        commit(2, "omp", "feat/x"),
        handoff(5, "worker", "omp"),
    ];
    let (lanes, _) = derive(&events);
    assert_eq!(
        shape(&lanes),
        [
            (LaneKind::Branch, "feat/x", 2, 2, None),
            (LaneKind::Handoff, "worker", 5, 5, None),
            (LaneKind::Collab, "clara", 8, 8, None)
        ]
    );
}

#[test]
fn events_at_the_same_instant_in_either_order_give_the_same_lanes() {
    let a = [
        note(0, "omp"),
        commit(1, "omp", "feat/x"),
        handoff(1, "clara", "omp"),
        note(1, "worker"),
    ];
    let mut b: Vec<Event> = a.to_vec();
    b.reverse();
    assert_eq!(derive(&a), derive(&b));
}

// -- random items ---------------------------------------------------------------

const AGENTS: [Option<&str>; 9] = [
    None,
    Some(""),
    Some("omp"),
    Some("clara"),
    Some("worker"),
    Some("scheduler"),
    Some("reviewer"),
    Some("indexer"),
    Some("planner"),
];
const BRANCHES: [Option<&str>; 14] = [
    None,
    None,
    None,
    Some("main"),
    Some(" "),
    Some("feat/a"),
    Some("feat/b"),
    Some("feat/c"),
    Some("fix/d"),
    Some("fix/e"),
    Some("chore/f"),
    Some("chore/g"),
    Some("refactor/h"),
    Some("refactor/i"),
];
const RUNS: [Option<&str>; 7] = [None, None, Some("r1"), Some("r2"), Some("r3"), Some("r4"), Some("r5")];
const STATUSES: [Option<&str>; 4] = [None, None, Some("done"), Some("claimed")];

#[derive(Clone, Debug)]
struct Spec {
    secs: i64,
    kind: usize,
    agent: usize,
    branch: usize,
    run: usize,
    from: usize,
    status: usize,
    seq: u64,
    git: bool,
}

fn arb_spec() -> impl Strategy<Value = Spec> {
    (
        // A handful of instants, so ties are common.
        0i64..9,
        0..EventKind::ALL.len(),
        0..AGENTS.len(),
        0..BRANCHES.len(),
        0..RUNS.len(),
        0..AGENTS.len(),
        0..STATUSES.len(),
        0u64..3,
        any::<bool>(),
    )
        .prop_map(|(secs, kind, agent, branch, run, from, status, seq, git)| Spec {
            secs,
            kind,
            agent,
            branch,
            run,
            from,
            status,
            seq,
            git,
        })
}

/// One item's events from `specs`, ids distinct, in generation order.
fn item_events(specs: &[Spec]) -> Vec<Event> {
    specs
        .iter()
        .enumerate()
        .map(|(i, spec)| {
            let mut event = Event::new(format!("e{i}"), since() + spec.secs * SEC, EventKind::ALL[spec.kind])
                .item(ITEM)
                .seq(spec.seq)
                .source(if spec.git { Source::Git } else { Source::Ledger });
            event.agent = AGENTS[spec.agent].map(str::to_owned);
            event.branch = BRANCHES[spec.branch].map(str::to_owned);
            event.run_id = RUNS[spec.run].map(str::to_owned);
            event.detail.from = AGENTS[spec.from].map(str::to_owned);
            event.detail.status = STATUSES[spec.status].map(str::to_owned);
            event
        })
        .collect()
}

fn arb_item() -> impl Strategy<Value = Vec<Event>> {
    proptest::collection::vec(arb_spec(), 0..40).prop_map(|specs| item_events(&specs))
}

proptest! {
    #[test]
    fn derived_lanes_and_edges_hold_their_invariants(events in arb_item()) {
        let (lanes, edges) = derive(&events);

        prop_assert!(lanes.len() <= FORK_CAP);
        let ids: HashSet<&str> = lanes.iter().map(|lane| lane.id.as_str()).collect();
        prop_assert_eq!(ids.len(), lanes.len(), "ids are unique");
        prop_assert!(lanes.windows(2).all(|pair| pair[0].from <= pair[1].from), "forks in time order");
        let by_id: HashMap<&str, &Lane> = lanes.iter().map(|lane| (lane.id.as_str(), lane)).collect();
        for lane in &lanes {
            prop_assert!(lane.from <= lane.to, "{lane:?}");
            prop_assert_eq!(&lane.item_id, ITEM);
            prop_assert_eq!(&lane.id, &format!("{ITEM}/{}/{}", lane.kind.as_str(), lane.label));
            if let Some(parent) = &lane.parent {
                prop_assert!(by_id.contains_key(parent.as_str()), "{parent} is not a lane of the item");
                prop_assert_ne!(parent, &lane.id);
            }
            if lane.kind == LaneKind::Handoff || lane.kind == LaneKind::Collab {
                prop_assert_eq!(lane.agent.as_deref(), Some(lane.label.as_str()));
            }
        }
        let mut forks: BTreeMap<&str, usize> = BTreeMap::new();
        let mut merges: BTreeMap<&str, usize> = BTreeMap::new();
        for edge in &edges {
            let lane = by_id.get(edge.lane.as_str());
            prop_assert!(lane.is_some(), "{edge:?} names no lane");
            let lane = lane.unwrap();
            prop_assert_eq!(&edge.item_id, ITEM);
            prop_assert!(lane.from <= edge.at && edge.at <= lane.to, "{edge:?} outside {lane:?}");
            match edge.kind {
                EdgeKind::Fork => {
                    prop_assert_eq!(edge.at, lane.from);
                    *forks.entry(edge.lane.as_str()).or_default() += 1;
                }
                EdgeKind::Merge => {
                    prop_assert!(lane.chosen != Some(false), "a dead lane never merges");
                    *merges.entry(edge.lane.as_str()).or_default() += 1;
                }
            }
        }
        for lane in &lanes {
            prop_assert_eq!(forks.get(lane.id.as_str()), Some(&1), "one fork per lane");
            prop_assert!(merges.get(lane.id.as_str()).is_none_or(|n| *n == 1));
        }
    }

    #[test]
    fn the_order_events_arrive_in_never_changes_the_lanes(
        events in arb_item(),
        keys in proptest::collection::vec(any::<u32>(), 40),
    ) {
        let mut order: Vec<usize> = (0..events.len()).collect();
        order.sort_by_key(|i| keys[*i % keys.len()]);
        let shuffled: Vec<Event> = order.iter().map(|i| events[*i].clone()).collect();
        prop_assert_eq!(derive(&events), derive(&shuffled));
    }
}

// -- the layout -----------------------------------------------------------------

fn branchy() -> Vec<Event> {
    vec![
        commit(1, "omp", "feat/x").item("imp-1"),
        note(40, "omp").item("imp-1"),
        note(2, "omp").item("imp-2"),
    ]
}

fn explicit() -> SessionTree {
    let lane = Lane {
        id: "imp-2/collab/clara".into(),
        item_id: "imp-2".into(),
        kind: LaneKind::Collab,
        label: "clara".into(),
        from: at(0.0),
        to: at(1.0),
        chosen: None,
        parent: None,
        agent: Some("clara".into()),
    };
    SessionTree {
        agent: None,
        edges: vec![Edge {
            kind: EdgeKind::Fork,
            lane: lane.id.clone(),
            item_id: "imp-2".into(),
            at: lane.from,
        }],
        lanes: vec![lane],
    }
}

fn forks_of<'g>(graph: &'g Graph, id: &str) -> Vec<&'g str> {
    graph
        .items
        .iter()
        .find(|item| item.id == id)
        .map(|item| item.forks.iter().map(|fork| fork.id.as_str()).collect())
        .unwrap_or_default()
}

#[test]
fn with_the_tree_off_no_lane_is_derived() {
    let graph = layout(&branchy());
    assert!(graph.items.iter().all(|item| item.forks.is_empty() && item.edges.is_empty()));
    assert_eq!(graph.rows, graph.lanes);
}

#[test]
fn with_the_tree_on_items_without_lanes_get_theirs_from_their_events() {
    let tree = SessionTree::default();
    let graph = Graph::layout(&branchy(), &opts().tree(&tree));
    assert_eq!(forks_of(&graph, "imp-1"), ["imp-1/branch/feat/x"]);
    assert!(forks_of(&graph, "imp-2").is_empty());
    let item = graph.items.iter().find(|item| item.id == "imp-1").unwrap();
    assert_eq!(item.edges.len(), 1);
    assert!(graph.rows > graph.lanes, "the fork keeps a row under its item");

    let off = Graph::layout(&branchy(), &opts().tree(&tree).derive_forks(false));
    assert!(off.items.iter().all(|item| item.forks.is_empty()));
}

#[test]
fn explicit_lanes_win_over_derived_ones() {
    let mut events = branchy();
    events.push(commit(3, "omp", "feat/y").item("imp-2"));
    events.push(note(4, "clara").item("imp-2"));
    let tree = explicit();
    let graph = Graph::layout(&events, &opts().tree(&tree));
    assert_eq!(forks_of(&graph, "imp-2"), ["imp-2/collab/clara"]);
    assert_eq!(forks_of(&graph, "imp-1"), ["imp-1/branch/feat/x"]);
}

#[test]
fn the_callers_status_decides_whether_the_item_landed() {
    let events = [commit(1, "omp", "feat/x"), note(5, "omp")];
    let tree = SessionTree::default();
    let verdict = |statuses: Option<&HashMap<String, String>>| {
        let mut options = opts().tree(&tree);
        if let Some(statuses) = statuses {
            options = options.statuses(statuses);
        }
        let graph = Graph::layout(&events, &options);
        graph.items[0].forks[0].chosen
    };
    assert_eq!(verdict(None), None);
    let done = HashMap::from([(ITEM.to_owned(), "done".to_owned())]);
    assert_eq!(verdict(Some(&done)), Some(false));
}

#[test]
fn a_long_derived_caption_reserves_room_at_the_right_edge() {
    let near_now = |hours: f64, kind: EventKind| {
        Event::new(uid(), at(hours), kind).agent("omp").item(ITEM)
    };
    let events = [
        near_now(166.0, EventKind::Commit).branch("feat/a-branch-with-a-long-name"),
        near_now(167.0, EventKind::Note),
    ];
    let tree = SessionTree::default();
    let x1 = |options: Options<'_>| Graph::layout(&events, &options).items[0].x1;
    let off = x1(opts());
    assert_eq!(x1(opts().tree(&tree).derive_forks(false)), off);
    let on = x1(opts().tree(&tree));
    assert!(on < off - 20.0, "the axis gives way to the caption: {on} vs {off}");
}

/// Items with branches, handoffs and failed runs, shuffled in time.
fn busy() -> Vec<Event> {
    let mut events = Vec::new();
    for n in 0..40i64 {
        let item = format!("imp-{n}");
        events.push(at_s(n, EventKind::ItemCreated, "omp").item(&item));
        events.push(commit(n + 3, "omp", &format!("feat/b{}", n % 5)).item(&item));
        if n % 2 == 0 {
            events.push(handoff(n + 5, "clara", "omp").item(&item));
        }
        if n % 3 == 0 {
            events.push(run(n + 6, RunFailed, "worker", &format!("r{n}")).item(&item));
            events.push(run(n + 8, RunFinished, "worker", &format!("s{n}")).item(&item));
        }
        events.push(merged(n + 20, "omp", &format!("feat/b{}", n % 5)).item(&item));
    }
    events
}

#[test]
fn a_lazy_graph_derives_the_same_forks_row_heights_and_axis_as_a_built_one() {
    let events = busy();
    let tree = SessionTree::default();
    let built = Graph::layout(&events, &opts().tree(&tree));
    let lazy = Graph::layout_slice(&events, |_| true, &opts().tree(&tree).lazy(true));
    let capped = Graph::layout(&events, &opts().tree(&tree).limit(Some(5)));

    assert!(built.items.iter().any(|item| item.forks.len() >= 3));
    assert_eq!((lazy.rows, lazy.lanes, lazy.height), (built.rows, built.lanes, built.height));
    assert_eq!((&lazy.ticks, &lazy.gaps), (&built.ticks, &built.gaps));
    assert_eq!((&capped.ticks, &capped.gaps), (&built.ticks, &built.gaps));
    assert_eq!(lazy.row_spans().len(), built.items.len());
    for span in lazy.row_spans() {
        let item = lazy.item(&events, span.item);
        let want = built.items.iter().find(|one| one.id == item.id).unwrap();
        assert_eq!(&*item, want, "{}", item.id);
        assert_eq!(span.height, (1 + want.forks.len()) as f64 * 14.0);
    }
    for item in &capped.items {
        let want = built.items.iter().find(|one| one.id == item.id).unwrap();
        assert_eq!(item.forks.len(), want.forks.len());
        assert_eq!(item.edges.len(), want.edges.len());
    }
}

#[test]
fn the_view_derives_forks_for_a_host_that_holds_no_tree() {
    let state = GraphState {
        window: "7d".to_owned(),
        graph_width: 1400,
        tree: true,
        ..GraphState::default()
    };
    let events = branchy();
    let now = now() / SEC;
    let on = view::graph(&events, &state, None, now);
    assert_eq!(forks_of(&on, "imp-1"), ["imp-1/branch/feat/x"]);
    let off = view::graph(&events, &GraphState { tree: false, ..state }, None, now);
    assert!(off.items.iter().all(|item| item.forks.is_empty()));
}

proptest! {
    #[test]
    fn a_graph_of_derived_forks_keeps_every_item_inside_its_row_block(
        events in proptest::collection::vec(arb_spec(), 0..60)
    ) {
        let events: Vec<Event> = item_events(&events)
            .into_iter()
            .enumerate()
            .map(|(i, event)| event.item(format!("imp-{}", i % 6)))
            .collect();
        let tree = SessionTree::default();
        let built = Graph::layout(&events, &opts().tree(&tree));
        let lazy = Graph::layout_slice(&events, |_| true, &opts().tree(&tree).lazy(true));

        prop_assert_eq!((lazy.rows, lazy.lanes), (built.rows, built.lanes));
        for item in &built.items {
            prop_assert!(item.forks.len() <= FORK_CAP);
            for (i, fork) in item.forks.iter().enumerate() {
                prop_assert_eq!(fork.y, item.y + (i + 1) as f64 * 14.0);
                prop_assert!(fork.x1 >= fork.x0);
            }
            prop_assert!(item.edges.len() <= 2 * item.forks.len());
        }
        let extras: usize = built.items.iter().map(|item| item.forks.len()).sum();
        prop_assert!(built.rows <= built.lanes + extras);
    }
}
