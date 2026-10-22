//! Invariants of the layout over random windows, and its scaling at the size
//! of the board's real ledger (2351 items).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Instant;

use proptest::prelude::*;

use super::super::*;
use super::fixtures::*;
use crate::widgets::trace::model::{
    Edge, EdgeKind, Event, EventKind, Lane, LaneKind, Micros, SessionTree,
};

const STATUSES: [&str; 6] = [
    "open",
    "claimed",
    "in_progress",
    "review",
    "blocked",
    "done",
];

/// One generated event: `item` of `None` is loose; `secs` is its time in
/// seconds after the window start.
#[derive(Clone, Debug)]
struct Spec {
    item: Option<u8>,
    kind: usize,
    status: Option<usize>,
    secs: i64,
}

fn arb_spec() -> impl Strategy<Value = Spec> {
    (
        proptest::option::weighted(0.8, 0u8..10),
        0..EventKind::ALL.len(),
        proptest::option::of(0..STATUSES.len()),
        // Bursts of up to 2h every 5h across 14 days, so spans, gaps and
        // clusters all occur.
        (0i64..60, 0i64..7_200).prop_map(|(slot, jitter)| slot * 5 * 3_600 + jitter),
    )
        .prop_map(|(item, kind, status, secs)| Spec {
            item,
            kind,
            status,
            secs,
        })
}

/// Specs as events with distinct times and ids, in generation order.
fn events_of(specs: &[Spec]) -> Vec<Event> {
    let mut seen = BTreeSet::new();
    specs
        .iter()
        .filter(|spec| seen.insert(spec.secs))
        .map(|spec| {
            let mut event = Event::new(uid(), since() + spec.secs * SEC, EventKind::ALL[spec.kind])
                .agent("worker");
            event.item_id = spec.item.map(|n| format!("imp-{n}"));
            event.detail.status = spec.status.map(|s| STATUSES[s].to_string());
            event
        })
        .collect()
}

fn arb_events() -> impl Strategy<Value = Vec<Event>> {
    proptest::collection::vec(arb_spec(), 0..80).prop_map(|specs| events_of(&specs))
}

/// Fork lanes of the items `imp-0..imp-9`, a few per item, inside the window.
fn arb_tree() -> impl Strategy<Value = SessionTree> {
    proptest::collection::vec(
        (
            0u8..10,
            0i64..60,
            0i64..60,
            0usize..3,
            proptest::option::of(any::<bool>()),
        ),
        0..12,
    )
    .prop_map(|raw| {
        let mut lanes = Vec::new();
        let mut edges = Vec::new();
        for (n, (item, a, b, label, chosen)) in raw.into_iter().enumerate() {
            let label = ["omp", "orchestrator", "a-very-long-collaborator-name"][label];
            let (item_id, id) = (
                format!("imp-{item}"),
                format!("imp-{item}/collab/{label}-{n}"),
            );
            let lane = Lane {
                id: id.clone(),
                item_id: item_id.clone(),
                kind: LaneKind::Collab,
                label: label.to_string(),
                from: since() + a * 5 * 3_600 * SEC,
                to: since() + b * 5 * 3_600 * SEC,
                chosen,
                parent: None,
                agent: None,
            };
            edges.push(Edge {
                kind: EdgeKind::Fork,
                lane: id.clone(),
                item_id: item_id.clone(),
                at: lane.from,
            });
            edges.push(Edge {
                kind: EdgeKind::Merge,
                lane: id,
                item_id,
                at: lane.to,
            });
            lanes.push(lane);
        }
        SessionTree {
            agent: None,
            lanes,
            edges,
        }
    })
}

fn label_px(text: &str) -> f64 {
    text.chars().count() as f64 * 6.6
}

/// The extent an item's row space covers, read off the output alone.
fn extent(item: &Item) -> (f64, f64) {
    let first = item.dots.first().map_or(0.0, |dot| dot.clearance());
    let left = match item.start_label_x {
        None => item.x0 - first,
        Some(at) => at - label_px(&item.id) - 5.0,
    };
    let right = item
        .forks
        .iter()
        .map(|fork| fork.x1 + 7.5 + label_px(&fork.label))
        .fold(item.label_x + label_px(&item.id), f64::max);
    (left, right)
}

proptest! {
    #[test]
    fn clustering_conserves_every_event(events in arb_events()) {
        let graph = Graph::layout(&events, &opts().loose_open(true));

        let mut per_item: BTreeMap<&str, usize> = BTreeMap::new();
        let mut per_kind: HashMap<EventKind, usize> = HashMap::new();
        for event in &events {
            match event.item_id.as_deref() {
                Some(id) => *per_item.entry(id).or_default() += 1,
                None => *per_kind.entry(event.kind).or_default() += 1,
            }
        }
        prop_assert_eq!(graph.total_items, per_item.len());
        prop_assert_eq!(graph.items.len(), per_item.len());
        for item in &graph.items {
            let drawn: u32 = item.dots.iter().map(|dot| dot.count).sum();
            prop_assert_eq!(drawn as usize, per_item[item.id.as_str()]);
        }
        prop_assert_eq!(graph.strip.len(), per_kind.len());
        for row in &graph.strip {
            prop_assert_eq!(row.count, per_kind[&row.kind]);
            prop_assert_eq!(row.cells.iter().map(|cell| cell.count).sum::<usize>(), row.count);
        }
        prop_assert_eq!(graph.loose.len(), per_kind.len());
        for row in &graph.loose {
            prop_assert_eq!(row.dots.iter().map(|dot| dot.count as usize).sum::<usize>(), row.count);
        }
    }

    #[test]
    fn x_is_monotone_in_time_and_dots_stay_six_px_apart(events in arb_events()) {
        let graph = Graph::layout(&events, &opts());

        for item in &graph.items {
            for pair in item.dots.windows(2) {
                prop_assert!(pair[0].at <= pair[1].at);
                prop_assert!(pair[0].x <= pair[1].x);
                prop_assert!(pair[1].x - pair[0].x >= 6.0);
            }
            let (first, last) = (item.dots.first().expect("dots"), item.dots.last().expect("dots"));
            prop_assert_eq!((item.x0, item.x1), (first.x, last.x));
            prop_assert_eq!(item.label_x, last.x + last.clearance());
            prop_assert_eq!(item.start_label_x.is_some(), item.x1 - item.x0 >= 120.0);
        }
    }

    #[test]
    fn dots_stay_inside_the_axis_when_the_width_allows_it(events in arb_events()) {
        let graph = Graph::layout(&events, &Options::new(3000, Range::new(since(), now())));

        let (left, right) = (59.949999999999996, 3000.0 - 59.949999999999996);
        for dot in graph.items.iter().flat_map(|item| item.dots.iter()) {
            prop_assert!(dot.x >= left - 1e-6 && dot.x <= right + 1e-6, "{}", dot.x);
        }
        for gap in &graph.gaps {
            prop_assert!((gap.x1 - gap.x0 - 24.0).abs() < 1e-6);
        }
    }

    #[test]
    fn rows_never_overlap_and_use_exactly_the_peak_overlap(events in arb_events(), tree in arb_tree()) {
        let graph = Graph::layout(&events, &opts().tree(&tree));

        let mut by_lane: BTreeMap<usize, Vec<(f64, f64)>> = BTreeMap::new();
        for item in &graph.items {
            by_lane.entry(item.lane).or_default().push(extent(item));
        }
        prop_assert_eq!(by_lane.len(), graph.lanes);
        for spans in by_lane.values_mut() {
            spans.sort_by(|a, b| a.0.total_cmp(&b.0));
            for pair in spans.windows(2) {
                prop_assert!(pair[0].1 + 5.0 < pair[1].0, "{:?} under {:?}", pair[0], pair[1]);
            }
        }
        let extents: Vec<(f64, f64)> = graph.items.iter().map(extent).collect();
        let peak = extents
            .iter()
            .map(|(point, _)| extents.iter().filter(|(l, r)| l <= point && *point <= r + 5.0).count())
            .max()
            .unwrap_or(0);
        prop_assert_eq!(graph.lanes, peak);
    }

    #[test]
    fn rows_are_newest_first_and_stack_in_blocks_of_one_plus_the_most_forks(events in arb_events(), tree in arb_tree()) {
        let graph = Graph::layout(&events, &opts().tree(&tree));

        let mut newest: BTreeMap<usize, Micros> = BTreeMap::new();
        for item in &graph.items {
            let slot = newest.entry(item.lane).or_insert(Micros::MIN);
            *slot = (*slot).max(item.last);
        }
        let order: Vec<Micros> = newest.values().copied().collect();
        prop_assert!(order.windows(2).all(|pair| pair[0] >= pair[1]), "{order:?}");

        // One y per row; each row's block is its item line plus its most forks.
        let mut blocks: Vec<(f64, usize)> = Vec::new();
        for lane in 0..graph.lanes {
            let members: Vec<&Item> = graph.items.iter().filter(|item| item.lane == lane).collect();
            prop_assert!(!members.is_empty());
            prop_assert!(members.iter().all(|item| item.y == members[0].y));
            blocks.push((members[0].y, members.iter().map(|item| item.forks.len()).max().unwrap_or(0)));
        }
        for pair in blocks.windows(2) {
            prop_assert_eq!(pair[1].0 - pair[0].0, (1 + pair[0].1) as f64 * 14.0);
        }
        let extras: usize = blocks.iter().map(|(_, extra)| extra).sum();
        prop_assert_eq!(graph.rows, graph.lanes + extras);
        let top = graph.strip.len() as f64 * 14.0;
        prop_assert_eq!(graph.axis_y, top + graph.rows as f64 * 14.0 + 4.0);
        prop_assert_eq!(graph.height, (graph.axis_y + 28.0).round() as i64);
    }

    #[test]
    fn every_fork_hangs_under_its_item_and_every_edge_names_a_fork(events in arb_events(), tree in arb_tree()) {
        let graph = Graph::layout(&events, &opts().tree(&tree).derive_forks(false));

        for item in &graph.items {
            let lanes = tree.lanes.iter().filter(|lane| lane.item_id == item.id).count();
            prop_assert_eq!(item.forks.len(), lanes);
            for (i, fork) in item.forks.iter().enumerate() {
                prop_assert_eq!(fork.y, item.y + (i + 1) as f64 * 14.0);
                prop_assert!(fork.x1 >= fork.x0);
                prop_assert!(fork.label.chars().count() <= 16);
            }
            let want = tree.edges.iter().filter(|edge| edge.item_id == item.id).count();
            prop_assert_eq!(item.edges.len(), want);
        }
    }

    #[test]
    fn a_cap_keeps_exactly_the_newest_items(events in arb_events(), limit in 0usize..12) {
        let all = Graph::layout(&events, &opts());
        let capped = Graph::layout(&events, &opts().limit(Some(limit)));

        let mut ranked: Vec<(Micros, &str)> = all.items.iter().map(|item| (item.last, item.id.as_str())).collect();
        ranked.sort_by(|a, b| b.cmp(a));
        let want: BTreeSet<&str> = ranked.iter().take(limit).map(|(_, id)| *id).collect();
        let got: BTreeSet<&str> = capped.items.iter().map(|item| item.id.as_str()).collect();
        prop_assert_eq!(got, want);
        prop_assert_eq!(capped.total_items, all.total_items);
        prop_assert_eq!(capped.hidden, all.total_items - capped.items.len());
    }

    #[test]
    fn the_input_order_never_changes_the_graph(
        events in arb_events(),
        tree in arb_tree(),
        keys in proptest::collection::vec(any::<u32>(), 80),
    ) {
        let mut shuffled: Vec<Event> = events.clone();
        let mut order: Vec<usize> = (0..shuffled.len()).collect();
        order.sort_by_key(|i| keys[*i % keys.len()]);
        shuffled = order.iter().map(|i| events[*i].clone()).collect();

        let a = Graph::layout(&events, &opts().tree(&tree).loose_open(true));
        let b = Graph::layout(&shuffled, &opts().tree(&tree).loose_open(true));
        prop_assert_eq!(a, b);
    }

    #[test]
    fn strip_cells_are_four_px_columns_and_the_densest_is_opaque(events in arb_events()) {
        let graph = Graph::layout(&events, &opts());

        for row in &graph.strip {
            prop_assert!(row.cells.windows(2).all(|pair| pair[0].x < pair[1].x));
            prop_assert!(row.cells.iter().all(|cell| cell.x % 4 == 0 && cell.w == 4));
            prop_assert!(row.cells.iter().all(|cell| (0.25..=1.0).contains(&cell.alpha)));
            let peak = row.cells.iter().map(|cell| cell.count).max().expect("a cell");
            let darkest = row.cells.iter().map(|cell| cell.alpha).fold(f64::MIN, f64::max);
            prop_assert_eq!(darkest, 1.0);
            prop_assert!(row.cells.iter().filter(|cell| cell.count == peak).all(|cell| cell.alpha == 1.0));
        }
    }

    #[test]
    fn clustering_fresh_events_equals_merging_their_dots(
        raw in proptest::collection::vec((0.0f64..60.0, 0usize..18), 0..40)
    ) {
        let events: Vec<(Event, f64)> = raw
            .iter()
            .enumerate()
            .map(|(i, (x, kind))| (Event::new(format!("e{i}"), i as i64 * SEC, EventKind::ALL[*kind]).agent("a"), *x))
            .collect();

        let clustered = cluster(events.iter().map(|(event, x)| (event, *x)));
        let merged = merge(events.iter().map(|(event, x)| Dot::of(event, *x)).collect());
        prop_assert_eq!(&clustered, &merged);
        prop_assert_eq!(clustered.iter().map(|dot| dot.count as usize).sum::<usize>(), events.len());
        prop_assert!(clustered.windows(2).all(|pair| pair[1].x - pair[0].x >= 6.0));
        for dot in &clustered {
            prop_assert_eq!(dot.title.split('\n').count(), dot.count as usize);
        }
    }
}

// -- scale ----------------------------------------------------------------------

/// A ledger the size of the board's: 2351 items of 1 to 9 events across 60
/// days, 600 item-less events of five kinds, and a fork lane on every fifth item.
fn ledger() -> (Vec<Event>, SessionTree) {
    let mut events = Vec::new();
    for n in 0..2351u32 {
        let start = i64::from(n % 60) * 24 * 3_600 + i64::from(n % 17) * 1_900;
        for k in 0..=(n % 9) {
            let mut event = Event::new(
                uid(),
                since() + (start + i64::from(k) * 2_400 + i64::from(n % 7)) * SEC,
                EventKind::ALL[(n + k) as usize % EventKind::ALL.len()],
            )
            .agent("worker")
            .item(format!("imp-{n}"));
            if k % 3 == 0 {
                event.detail.status = Some(STATUSES[(n + k) as usize % 6].to_string());
            }
            events.push(event);
        }
    }
    for n in 0..600u32 {
        let kind = [
            EventKind::Commit,
            EventKind::BranchHead,
            EventKind::PrMerged,
            EventKind::PrOpened,
            EventKind::Note,
        ][n as usize % 5];
        events
            .push(Event::new(uid(), since() + (i64::from(n) * 8_641) * SEC, kind).agent("worker"));
    }
    let lanes = (0..2351u32)
        .step_by(5)
        .map(|n| {
            let start = i64::from(n % 60) * 24 * 3_600 + i64::from(n % 17) * 1_900;
            Lane {
                id: format!("imp-{n}/collab/omp"),
                item_id: format!("imp-{n}"),
                kind: LaneKind::Collab,
                label: "omp".into(),
                from: since() + (start + 1_200) * SEC,
                to: since() + (start + 7_200) * SEC,
                chosen: None,
                parent: None,
                agent: None,
            }
        })
        .collect();
    (
        events,
        SessionTree {
            agent: None,
            lanes,
            edges: vec![],
        },
    )
}

#[test]
fn the_boards_whole_ledger_lays_out_and_every_item_fits_a_row() {
    let (events, tree) = ledger();
    let range = Range::new(since(), since() + 61 * 24 * 3_600 * SEC);
    let options = Options::new(1400, range)
        .tree(&tree)
        .derive_forks(false)
        .loose_open(true);

    let started = Instant::now();
    let graph = Graph::layout(&events, &options);
    let elapsed = started.elapsed();

    assert_eq!(
        (graph.total_items, graph.items.len(), graph.hidden),
        (2351, 2351, 0)
    );
    assert_eq!(graph.strip.len(), 5);
    assert_eq!(graph.loose.len(), 5);
    assert!(graph.lanes > 1 && graph.lanes < 2351);
    assert_eq!(
        graph
            .items
            .iter()
            .map(|item| item.forks.len())
            .sum::<usize>(),
        tree.lanes.len()
    );
    let dots: u32 = graph
        .items
        .iter()
        .flat_map(|item| item.dots.iter())
        .map(|dot| dot.count)
        .sum();
    assert_eq!(
        dots as usize,
        events
            .iter()
            .filter(|event| event.item_id.is_some())
            .count()
    );
    // Loose by design: a quadratic pass over 2351 items in a debug build
    // takes seconds, the linear one well under it.
    assert!(elapsed.as_millis() < 1_500, "layout took {elapsed:?}");
}

#[test]
fn capping_the_board_to_its_newest_rows_draws_twenty() {
    let (events, tree) = ledger();
    let range = Range::new(since(), since() + 61 * 24 * 3_600 * SEC);
    let graph = Graph::layout(
        &events,
        &Options::new(1400, range).tree(&tree).limit(Some(20)),
    );

    assert_eq!(
        (graph.total_items, graph.items.len(), graph.hidden),
        (2351, 20, 2331)
    );
    let mut lasts: Vec<Micros> = events
        .iter()
        .filter(|event| event.item_id.is_some())
        .fold(HashMap::<&str, Micros>::new(), |mut acc, event| {
            let slot = acc
                .entry(event.item_id.as_deref().expect("item"))
                .or_insert(Micros::MIN);
            *slot = (*slot).max(event.at);
            acc
        })
        .into_values()
        .collect();
    lasts.sort_unstable_by(|a, b| b.cmp(a));
    let mut shown: Vec<Micros> = graph.items.iter().map(|item| item.last).collect();
    shown.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(shown, lasts[..20].to_vec());
}
