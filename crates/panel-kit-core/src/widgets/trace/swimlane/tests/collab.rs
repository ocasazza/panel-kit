//! `collaboration_test.exs` ported, plus the open-question answers of
//! `collab.rs`: thread sort fallback, chronological staleness, order
//! independence, `Step` normalisation, `current_items` and `Supplied`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use rstest::rstest;

use super::super::*;
use super::fixtures::{uid, SEC};
use crate::widgets::trace::model::{Event, EventKind, Micros, Source};
use crate::widgets::trace::time;

// -- fixtures -----------------------------------------------------------------

const MINUTE: i64 = 60 * SEC;
const DAY: i64 = 86_400 * SEC;

/// `@t0`: 2026-10-03T14:00:00Z.
fn t0() -> Micros {
    time::from_utc(2026, 10, 3, 14, 0, 0)
}

/// `@now`: 2026-10-05T18:00:00Z.
fn now() -> Micros {
    time::from_utc(2026, 10, 5, 18, 0, 0)
}

/// `at(minutes)`: `minutes` after `t0`.
fn at(minutes: i64) -> Micros {
    t0() + minutes * MINUTE
}

/// An instant `days` before `now`.
fn days_ago(days: i64) -> Micros {
    now() - days * DAY
}

fn seq() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// A ledger event with a unique id and seq and nothing else.
fn event(kind: EventKind, at: Micros) -> Event {
    Event::new(uid(), at, kind).seq(seq())
}

fn note(item: &str, agent: &str, min: i64) -> Event {
    note_text(item, agent, min, "a note")
}

fn note_text(item: &str, agent: &str, min: i64, text: &str) -> Event {
    event(EventKind::Note, at(min))
        .item(item)
        .agent(agent)
        .summary(text)
}

fn note_at(stamp: Micros, agent: &str, text: &str, item: &str) -> Event {
    event(EventKind::Note, stamp)
        .item(item)
        .agent(agent)
        .summary(text)
}

fn created(item: &str, agent: &str, min: i64) -> Event {
    event(EventKind::ItemCreated, at(min))
        .item(item)
        .agent(agent)
        .title(format!("t {item}"))
        .status("open")
}

fn claimed(item: &str, agent: &str, min: i64) -> Event {
    claimed_at(item, agent, at(min))
}

fn claimed_at(item: &str, agent: &str, stamp: Micros) -> Event {
    event(EventKind::ItemClaimed, stamp)
        .item(item)
        .agent(agent)
        .status("claimed")
}

fn handoff(item: &str, from: &str, to: &str, min: i64) -> Event {
    event(EventKind::Handoff, at(min))
        .item(item)
        .agent(to)
        .from(from)
        .status("claimed")
}

fn merged(item: &str, pr: u64, branch: &str, commits: &[&str], min: i64) -> Event {
    event(EventKind::PrMerged, at(min))
        .item(item)
        .source(Source::Git)
        .pr(pr)
        .branch(branch)
        .commits(commits.iter().copied())
}

fn review(item: &str, agent: &str, pr: u64, min: i64) -> Event {
    event(EventKind::PrReviewed, at(min))
        .item(item)
        .agent(agent)
        .pr(pr)
        .source(Source::Github)
}

/// A git commit on `item` at minute 10, naming no agent.
fn commit(item: &str) -> Event {
    event(EventKind::Commit, at(10))
        .item(item)
        .source(Source::Git)
}

/// A commit that names no item.
fn loose_commit(sha: &str, min: i64) -> Event {
    event(EventKind::Commit, at(min)).commit(sha)
}

fn build(events: &[Event]) -> Collaboration {
    Collaboration::build(events, &HashMap::new(), now())
}

fn names(pairs: &[Pair]) -> Vec<(&str, &str)> {
    pairs
        .iter()
        .map(|pair| (pair.a.as_str(), pair.b.as_str()))
        .collect()
}

fn labels(thread: &Thread) -> Vec<&str> {
    thread
        .steps
        .iter()
        .map(|step| step.label.as_str())
        .collect()
}

fn held(name: &str) -> Supplied<String> {
    Supplied::Present(Some(name.to_string()))
}

/// The host's view of one item: its status and assignee.
fn items(item: &str, status: &str, assignee: Supplied<String>) -> HashMap<String, ItemInfo> {
    HashMap::from([(
        item.to_string(),
        ItemInfo {
            title: None,
            status: Supplied::Present(Some(status.into())),
            assignee,
        },
    )])
}

/// `a` claims the item five days ago and is never heard from again; `b` speaks later.
fn silent_holder_events(item: &str) -> Vec<Event> {
    vec![
        claimed_at(item, "a", days_ago(5)),
        note_at(days_ago(5), "a", "a note", item),
        note_at(days_ago(4), "b", "a note", item),
    ]
}

// -- a pair -------------------------------------------------------------------

#[test]
fn two_agents_on_one_item_are_one_edge() {
    let graph = build(&[
        created("imp-26", "imp-design", 0),
        note("imp-26", "imp-design", 1),
        note("imp-26", "ThemeLand", 5),
    ]);

    assert_eq!(graph.pairs.len(), 1);
    assert_eq!(graph.pairs[0].a, "ThemeLand");
    assert_eq!(graph.pairs[0].b, "imp-design");
    assert_eq!(graph.pairs[0].shared_items, ["imp-26"]);
}

#[test]
fn notes_are_counted_each_way_never_summed() {
    let graph = build(&[
        note("imp-26", "a", 5),
        note("imp-26", "a", 6),
        note("imp-26", "a", 7),
        note("imp-26", "b", 8),
    ]);

    assert_eq!(graph.pairs.len(), 1);
    assert_eq!((graph.pairs[0].notes_ab, graph.pairs[0].notes_ba), (3, 1));
}

#[test]
fn three_agents_who_all_spoke_are_three_edges_not_one() {
    let graph = build(&[
        note("imp-26", "a", 1),
        note("imp-26", "b", 2),
        note("imp-26", "c", 3),
    ]);

    assert_eq!(names(&graph.pairs), [("a", "b"), ("a", "c"), ("b", "c")]);
}

#[test]
fn an_edge_is_one_row_however_many_items_it_spans() {
    let graph = build(&[
        note("imp-26", "a", 1),
        note("imp-26", "b", 2),
        note("imp-27", "a", 3),
        note("imp-27", "b", 4),
    ]);

    assert_eq!(graph.pairs.len(), 1);
    assert_eq!(graph.pairs[0].shared_items, ["imp-26", "imp-27"]);
    assert_eq!((graph.pairs[0].notes_ab, graph.pairs[0].notes_ba), (2, 2));
}

#[test]
fn no_shared_item_is_no_edge_however_many_branches_they_share() {
    let events = [
        merged("imp-26", 1, "feat/x", &[], 5),
        merged("imp-27", 2, "feat/x", &[], 6),
    ];

    assert_eq!(build(&events).pairs, []);
}

#[test]
fn two_names_on_one_item_with_nothing_between_them_is_not_an_edge() {
    // The item's creator and its assignee are two names on one line.
    let events = [created("imp-32", "worker", 0), claimed("imp-32", "omp", 5)];

    assert_eq!(build(&events).pairs, []);
}

#[test]
fn a_co_commit_is_evidence_enough_on_its_own() {
    let events = [
        created("imp-16", "a", 0),
        created("imp-16", "b", 0),
        merged("imp-16", 155, "feat/imp-16", &["abc1234"], 30),
        loose_commit("abc1234", 10),
    ];

    let graph = build(&events);
    assert_eq!(graph.pairs.len(), 1);
    assert_eq!(graph.pairs[0].co_commits, 1);
}

#[test]
fn a_commit_a_pull_request_on_the_item_brought_in_is_a_co_commit() {
    let events = [
        created("imp-16", "a", 0),
        note("imp-16", "a", 1),
        created("imp-16", "b", 0),
        merged("imp-16", 155, "feat/imp-16-children", &["deadbee"], 30),
        commit("imp-16").commit("deadbee"),
    ];

    let graph = build(&events);
    assert_eq!(graph.pairs.len(), 1);
    assert_eq!(graph.pairs[0].co_commits, 1);
    assert_eq!(graph.pairs[0].branches, ["feat/imp-16-children"]);
}

#[test]
fn agents_who_share_no_item_are_not_an_edge() {
    let graph = build(&[note("imp-1", "a", 1), note("imp-2", "b", 2)]);

    assert_eq!(graph.pairs, []);
}

// -- handoffs -----------------------------------------------------------------

#[test]
fn handoffs_are_directional_and_name_who_took_the_item_over() {
    let events = [
        claimed("imp-29", "claude-imp-29", 2),
        handoff("imp-29", "claude-imp-29", "claude-29", 20),
        note("imp-29", "claude-29", 21),
    ];

    let graph = build(&events);
    assert_eq!(graph.pairs.len(), 1);
    assert_eq!(graph.pairs[0].a, "claude-29");
    assert_eq!(graph.pairs[0].b, "claude-imp-29");
    assert_eq!(
        graph.pairs[0].handoffs,
        [Handoff {
            from: "claude-imp-29".into(),
            to: "claude-29".into(),
            item_id: "imp-29".into(),
            at: at(20),
        }]
    );
}

#[test]
fn the_reverse_edge_is_the_same_fact_read_the_other_way_not_two() {
    let events = [claimed("imp-29", "a", 2), handoff("imp-29", "a", "b", 20)];

    let graph = build(&events);
    assert_eq!(names(&graph.pairs), [("a", "b")]);
    assert_eq!(graph.pairs[0].handoffs.len(), 1);
}

// -- reviews, branches and co-commits -----------------------------------------

#[test]
fn a_review_on_a_shared_item_is_an_edge() {
    let events = [
        note("imp-26", "a", 1),
        note("imp-26", "b", 2),
        review("imp-26", "b", 3, 9),
    ];

    let graph = build(&events);
    assert_eq!(graph.pairs.len(), 1);
    assert_eq!(graph.pairs[0].reviews, 1);
}

#[test]
fn a_review_does_not_make_an_edge_on_its_own() {
    // `b` only reviewed: a review is evidence about a pair that exists, and
    // `a`'s note is what puts `a` and `b` on one item with weight behind it.
    let events = [note("imp-26", "a", 1), review("imp-26", "b", 3, 9)];

    let graph = build(&events);
    assert_eq!(names(&graph.pairs), [("a", "b")]);
    assert_eq!(graph.pairs[0].reviews, 1);
}

#[test]
fn a_branch_on_the_item_belongs_to_both_agents_who_worked_it() {
    let events = [
        note("imp-26", "a", 1),
        note("imp-26", "b", 2),
        merged("imp-26", 3, "feat/imp-26", &[], 5),
        merged("imp-27", 4, "feat/imp-27", &[], 6),
    ];

    let graph = build(&events);
    assert_eq!(graph.pairs.len(), 1);
    assert_eq!(graph.pairs[0].branches, ["feat/imp-26"]);
    assert_eq!(graph.pairs[0].shared_items, ["imp-26"]);
}

#[test]
fn a_commit_the_index_has_not_read_yet_is_not_counted() {
    // The merge knows the sha; no commit event carries it yet.
    let events = [
        note("imp-16", "a", 1),
        note("imp-16", "b", 2),
        merged("imp-16", 155, "feat/imp-16", &["deadbee"], 30),
    ];

    let graph = build(&events);
    assert_eq!(graph.pairs.len(), 1);
    assert_eq!(graph.pairs[0].co_commits, 0);
}

#[test]
fn a_commit_naming_an_agent_is_that_agents_alone_not_a_co_commit() {
    let events = [
        note("imp-16", "a", 1),
        note("imp-16", "b", 2),
        merged("imp-16", 155, "feat/imp-16", &["deadbee"], 30),
        loose_commit("deadbee", 20).agent("a"),
    ];

    let graph = build(&events);
    assert_eq!(graph.pairs.len(), 1);
    assert_eq!(graph.pairs[0].co_commits, 0);
}

// -- order --------------------------------------------------------------------

#[test]
fn the_busiest_edge_is_first_and_the_order_never_depends_on_a_hash() {
    let mut events = vec![
        note("imp-1", "a", 1),
        note("imp-1", "b", 2),
        note("imp-2", "c", 3),
        note("imp-2", "d", 4),
        note("imp-2", "d", 5),
        note("imp-2", "c", 6),
    ];

    let first = build(&events).pairs;
    events.reverse();

    assert_eq!(first, build(&events).pairs);
    assert_eq!(names(&first), [("c", "d"), ("a", "b")]);
    assert_eq!((first[0].notes_ab, first[0].notes_ba), (2, 2));
    assert_eq!((first[1].notes_ab, first[1].notes_ba), (1, 1));
}

// -- threads ------------------------------------------------------------------

#[test]
fn an_item_with_two_agents_is_a_thread_an_item_with_one_is_not() {
    let graph = build(&[
        note("imp-26", "a", 1),
        note("imp-26", "b", 2),
        note("imp-27", "c", 3),
        note("imp-27", "d", 4),
    ]);

    let ids: Vec<&str> = graph
        .threads
        .iter()
        .map(|thread| thread.item_id.as_str())
        .collect();
    assert_eq!(ids, ["imp-26", "imp-27"]);

    let lone = build(&[note("imp-26", "a", 1), note("imp-27", "c", 3)]);
    assert_eq!(lone.threads, []);
}

#[test]
fn a_blank_agent_is_nobody_so_it_is_neither_half_of_a_pair_nor_an_edge() {
    let events = [note("imp-32", "  ", 1), note("imp-32", "omp", 2)];

    assert_eq!(build(&events).pairs, []);
    assert_eq!(build(&events).threads, []);
}

#[test]
fn an_item_with_one_real_agent_and_one_blank_is_a_thread_of_one() {
    let events = [
        note("imp-32", "worker", 1),
        Event {
            agent: None,
            ..note("imp-32", "x", 2)
        },
        note("imp-32", "  ", 3),
    ];

    assert_eq!(build(&events), Collaboration::default());
}

#[test]
fn the_steps_are_in_order_and_carry_the_notes_own_words() {
    let events = [
        note_text("imp-29", "b", 20, "second"),
        claimed("imp-29", "a", 2),
        note_text("imp-29", "a", 5, "first"),
    ];

    let graph = build(&events);
    assert_eq!(graph.threads.len(), 1);
    assert_eq!(labels(&graph.threads[0]), ["claimed", "first", "second"]);
}

#[test]
fn steps_arrive_shuffled_and_are_drawn_in_time_order() {
    let base = days_ago(4);
    let shuffled = [
        note_at(base + 360 * SEC, "b", "late", "imp-18"),
        note_at(base, "a", "first", "imp-18"),
        note_at(base + 120 * SEC, "a", "second", "imp-18"),
        note_at(base + 60 * SEC, "b", "middle", "imp-18"),
    ];
    let input: Vec<&str> = shuffled
        .iter()
        .filter_map(|e| e.summary.as_deref())
        .collect();
    assert_ne!(input, ["first", "middle", "second", "late"]);

    let graph = build(&shuffled);
    assert_eq!(graph.threads.len(), 1);
    let thread = &graph.threads[0];
    assert_eq!(labels(thread), ["first", "middle", "second", "late"]);
    assert!(thread.steps.windows(2).all(|pair| pair[0].at <= pair[1].at));
}

#[test]
fn an_item_still_claimed_by_an_agent_nobody_has_heard_from_is_stale() {
    let events = [
        claimed_at("imp-29", "claude-imp-29", days_ago(5)),
        note_at(days_ago(5) + 60 * SEC, "claude-imp-29", "a note", "imp-29"),
        note_at(days_ago(5) + 120 * SEC, "claude-29", "a note", "imp-29"),
    ];

    let graph = build(&events);
    assert_eq!(graph.threads.len(), 1);
    let thread = &graph.threads[0];
    assert!(thread.stale);
    assert_eq!(thread.stale_agent.as_deref(), Some("claude-imp-29"));
    assert_eq!(thread.stale_since, Some(days_ago(5) + 60 * SEC));
}

#[test]
fn an_item_still_claimed_but_heard_from_in_the_last_three_days_is_not() {
    let events = [
        claimed_at("imp-11", "omp", days_ago(1)),
        note_at(days_ago(2), "omp", "a note", "imp-29"),
        note_at(days_ago(1), "someone", "a note", "imp-29"),
    ];

    // The claim is on imp-11, the notes on imp-29: one thread, not stale.
    let graph = build(&events);
    assert_eq!(graph.threads.len(), 1);
    assert!(!graph.threads[0].stale);
}

#[test]
fn an_item_that_moved_on_is_not_stale_whoever_holds_it_now() {
    let events = silent_holder_events("imp-29");
    let info = items("imp-29", "done", held("a"));

    let graph = Collaboration::build(&events, &info, now());
    assert_eq!(graph.threads.len(), 1);
    assert!(!graph.threads[0].stale);
}

#[test]
fn an_item_with_no_assignee_at_all_is_not_stale_it_is_unheld() {
    let events = silent_holder_events("imp-29");
    let info = items("imp-29", "claimed", Supplied::Present(None));

    let graph = Collaboration::build(&events, &info, now());
    assert_eq!(graph.threads.len(), 1);
    assert!(!graph.threads[0].stale);
    assert_eq!(graph.threads[0].stale_agent, None);
}

#[test]
fn a_stale_thread_sorts_above_a_live_one() {
    let events = [
        note("imp-1", "a", 10),
        note("imp-1", "b", 10 * 60),
        claimed_at("imp-2", "a", days_ago(5)),
        note_at(days_ago(5), "a", "a note", "imp-2"),
        note_at(days_ago(4), "b", "a note", "imp-2"),
    ];

    let graph = build(&events);
    assert_eq!(graph.threads[0].item_id, "imp-2");
    assert_eq!(graph.threads[1].item_id, "imp-1");
}

// -- events with no item ------------------------------------------------------

#[test]
fn events_with_no_item_are_not_a_thread_about_every_item_at_once() {
    let events = [
        Event {
            item_id: None,
            ..note("x", "a", 1)
        },
        Event {
            item_id: None,
            ..note("x", "b", 2)
        },
    ];

    assert_eq!(build(&events), Collaboration::default());
}

// -- nothing at all -----------------------------------------------------------

#[test]
fn nothing_at_all_is_an_empty_graph_not_a_crash() {
    assert_eq!(build(&[]), Collaboration::default());
    assert_eq!(Collaboration::default().size(), 0);
}

#[test]
fn size_counts_pairs_plus_threads() {
    let graph = build(&[note("imp-1", "a", 1), note("imp-1", "b", 2)]);

    assert_eq!((graph.pairs.len(), graph.threads.len()), (1, 1));
    assert_eq!(graph.size(), 2);
}

// -- open questions: thread order ---------------------------------------------

fn two_agent_item(item: &str) -> [Event; 2] {
    [note(item, "a", 1), note(item, "b", 2)]
}

#[test]
fn threads_sort_by_item_number_then_name() {
    let events: Vec<Event> = ["imp-10", "imp-2", "zeta", "alpha"]
        .into_iter()
        .flat_map(two_agent_item)
        .collect();

    let ids: Vec<String> = build(&events)
        .threads
        .into_iter()
        .map(|t| t.item_id)
        .collect();
    assert_eq!(ids, ["imp-2", "imp-10", "alpha", "zeta"]);
}

#[test]
fn a_stale_thread_beats_the_item_number_order() {
    let mut events: Vec<Event> = ["imp-10", "imp-2", "alpha"]
        .into_iter()
        .flat_map(two_agent_item)
        .collect();
    events.extend(silent_holder_events("zeta"));

    let graph = build(&events);
    let ids: Vec<&str> = graph.threads.iter().map(|t| t.item_id.as_str()).collect();
    assert_eq!(ids, ["zeta", "imp-2", "imp-10", "alpha"]);
    assert!(graph.threads[0].stale);
}

// -- open questions: staleness is chronological -------------------------------

#[test]
fn stale_since_is_the_holders_chronologically_last_event_under_any_input_order() {
    let events = vec![
        claimed_at("imp-5", "a", days_ago(6)),
        note_at(days_ago(5), "a", "earlier", "imp-5"),
        note_at(days_ago(4), "a", "last", "imp-5"),
        note_at(days_ago(4) + SEC, "b", "other", "imp-5"),
    ];
    let mut reversed = events.clone();
    reversed.reverse();
    let rotated: Vec<Event> = events[2..].iter().chain(&events[..2]).cloned().collect();

    for input in [&events, &reversed, &rotated] {
        let graph = build(input);
        assert_eq!(graph.threads.len(), 1);
        let thread = &graph.threads[0];
        assert!(thread.stale);
        assert_eq!(thread.stale_agent.as_deref(), Some("a"));
        assert_eq!(thread.stale_since, Some(days_ago(4)));
    }
}

#[test]
fn a_holder_heard_from_recently_is_not_stale_whatever_the_input_order() {
    // The newest note of `a` is listed first; the claim listed last is older.
    let events = [
        note_at(days_ago(1), "a", "recent", "imp-5"),
        note_at(days_ago(2), "b", "other", "imp-5"),
        claimed_at("imp-5", "a", days_ago(6)),
    ];

    let graph = build(&events);
    assert_eq!(graph.threads.len(), 1);
    assert!(!graph.threads[0].stale);
}

// -- open questions: pair evidence is order independent ------------------------

/// A fixed multi-item event set: notes, a review, handoffs both ways,
/// merges with branches and commits, a loose commit and no-item events.
fn fixed_events() -> Vec<Event> {
    vec![
        note("imp-1", "a", 1),
        note("imp-1", "b", 2),
        note("imp-1", "b", 3),
        review("imp-1", "c", 9, 4),
        created("imp-2", "a", 0),
        claimed("imp-2", "a", 2),
        handoff("imp-2", "a", "b", 20),
        note("imp-2", "b", 21),
        note("imp-2", "c", 22),
        merged("imp-3", 5, "feat/x", &["s1", "s2", "s3"], 30),
        note("imp-3", "a", 1),
        note("imp-3", "c", 2),
        commit("imp-3").commit("s1"),
        commit("imp-3").commit("s2").agent("a"),
        loose_commit("s3", 11),
        merged("imp-4", 6, "feat/y", &[], 6),
        note("imp-4", "b", 1),
        note("imp-4", "c", 3),
        Event {
            item_id: None,
            ..note("x", "a", 4)
        },
        handoff("imp-5", "c", "a", 7),
        note("imp-5", "c", 2),
        claimed_at("imp-6", "a", days_ago(6)),
        note_at(days_ago(5), "b", "a note", "imp-6"),
    ]
}

type ThreadView = (
    String,
    Vec<String>,
    bool,
    Option<String>,
    Option<Micros>,
    Vec<Step>,
);

/// Everything of a thread but its title, which reads the first title in input order.
fn thread_views(graph: &Collaboration) -> Vec<ThreadView> {
    graph
        .threads
        .iter()
        .map(|t| {
            (
                t.item_id.clone(),
                t.agents.clone(),
                t.stale,
                t.stale_agent.clone(),
                t.stale_since,
                t.steps.clone(),
            )
        })
        .collect()
}

proptest! {
    #[test]
    fn pairs_and_threads_do_not_depend_on_input_order(
        keys in prop::collection::vec(any::<u32>(), 32),
    ) {
        let events = fixed_events();
        prop_assert!(events.len() <= keys.len());
        let mut order: Vec<usize> = (0..events.len()).collect();
        order.sort_by_key(|&i| (keys[i], i));
        let shuffled: Vec<Event> = order.iter().map(|&i| events[i].clone()).collect();

        let expected = build(&events);
        let got = build(&shuffled);
        prop_assert_eq!(&got.pairs, &expected.pairs);
        prop_assert_eq!(thread_views(&got), thread_views(&expected));
    }
}

#[test]
fn the_fixed_event_set_draws_pairs_with_every_kind_of_evidence() {
    let graph = build(&fixed_events());

    assert!(graph.pairs.iter().any(|p| p.reviews > 0));
    assert!(graph.pairs.iter().any(|p| !p.handoffs.is_empty()));
    assert!(graph.pairs.iter().any(|p| p.co_commits > 0));
    assert!(graph.pairs.iter().any(|p| !p.branches.is_empty()));
    assert!(graph.threads.iter().any(|t| t.stale));
}

// -- open questions: handoff order --------------------------------------------

#[test]
fn handoffs_across_items_sort_by_time_then_item() {
    let events = vec![
        note("imp-5", "a", 1),
        handoff("imp-5", "a", "b", 30),
        note("imp-3", "a", 1),
        handoff("imp-3", "a", "b", 30),
        note("imp-9", "a", 1),
        handoff("imp-9", "a", "b", 10),
        note("imp-7", "b", 1),
        handoff("imp-7", "b", "a", 20),
    ];
    let mut reversed = events.clone();
    reversed.reverse();

    for input in [&events, &reversed] {
        let graph = build(input);
        assert_eq!(graph.pairs.len(), 1);
        let order: Vec<(&str, Micros)> = graph.pairs[0]
            .handoffs
            .iter()
            .map(|h| (h.item_id.as_str(), h.at))
            .collect();
        assert_eq!(
            order,
            [
                ("imp-9", at(10)),
                ("imp-7", at(20)),
                ("imp-3", at(30)),
                ("imp-5", at(30))
            ]
        );
    }
}

// -- open questions: step and thread normalisation -----------------------------

#[test]
fn step_at_is_truncated_to_whole_seconds() {
    let events = [
        Event {
            at: t0() + 5 * SEC + 700_000,
            ..note("imp-1", "a", 0)
        },
        Event {
            at: t0() + SEC + 999_999,
            ..note("imp-1", "b", 0)
        },
        Event {
            at: t0() + 7 * SEC,
            ..note("imp-1", "a", 0)
        },
    ];

    let graph = build(&events);
    let at: Vec<Micros> = graph.threads[0].steps.iter().map(|s| s.at).collect();
    assert_eq!(at, [t0() + SEC, t0() + 5 * SEC, t0() + 7 * SEC]);
}

#[test]
fn thread_agents_are_sorted_and_a_blank_step_agent_is_none() {
    let events = [
        note("imp-1", "zed", 1),
        note("imp-1", "alpha", 2),
        note("imp-1", "mid", 3),
        note("imp-1", "  ", 4),
        Event {
            agent: None,
            ..note("imp-1", "x", 5)
        },
    ];

    let graph = build(&events);
    let thread = &graph.threads[0];
    assert_eq!(thread.agents, ["alpha", "mid", "zed"]);
    let step_agents: Vec<Option<&str>> = thread.steps.iter().map(|s| s.agent.as_deref()).collect();
    assert_eq!(
        step_agents,
        [Some("zed"), Some("alpha"), Some("mid"), None, None]
    );
}

#[test]
fn only_created_claimed_released_handed_noted_and_merged_events_are_steps() {
    let events = [
        note("imp-1", "a", 1),
        note("imp-1", "b", 2),
        review("imp-1", "b", 3, 3),
        event(EventKind::ItemStatus, at(4))
            .item("imp-1")
            .agent("a")
            .status("done"),
        event(EventKind::Heartbeat, at(5)).item("imp-1").agent("a"),
        commit("imp-1").commit("abc"),
        event(EventKind::ItemReleased, at(6))
            .item("imp-1")
            .agent("a"),
    ];

    let graph = build(&events);
    let kinds: Vec<EventKind> = graph.threads[0].steps.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        [EventKind::Note, EventKind::Note, EventKind::ItemReleased]
    );
    assert!(graph.threads[0].steps.iter().all(|s| s.item_id == "imp-1"));
}

#[test]
fn step_labels_name_the_event() {
    let events = [
        Event {
            summary: None,
            ..note("imp-12", "a", 1)
        },
        merged("imp-12", 12, "feat/x", &[], 2),
        handoff("imp-12", "a", "b", 3),
        created("imp-12", "a", 0),
        claimed("imp-12", "a", 0),
    ];

    let graph = build(&events);
    assert_eq!(graph.threads.len(), 1);
    let thread = &graph.threads[0];
    assert_eq!(labels(thread).len(), 5);
    for wanted in [
        "note",
        "PR #12 merged",
        "handed from a",
        "created",
        "claimed",
    ] {
        assert!(
            labels(thread).contains(&wanted),
            "{wanted} missing in {:?}",
            labels(thread)
        );
    }
}

#[test]
fn a_thread_reads_the_hosts_title_and_status_else_the_first_event_title() {
    let events = [created("imp-1", "a", 0), note("imp-1", "b", 1)];

    let from_events = build(&events);
    assert_eq!(from_events.threads[0].title.as_deref(), Some("t imp-1"));
    assert_eq!(from_events.threads[0].status, None);

    let info = HashMap::from([(
        "imp-1".to_string(),
        ItemInfo {
            title: Some("host title".into()),
            status: Supplied::Present(Some("done".into())),
            assignee: Supplied::Absent,
        },
    )]);
    let from_host = Collaboration::build(&events, &info, now());
    assert_eq!(from_host.threads[0].title.as_deref(), Some("host title"));
    assert_eq!(from_host.threads[0].status.as_deref(), Some("done"));
}

// -- open questions: current_items --------------------------------------------

#[test]
fn current_items_status_is_the_last_item_status_event() {
    let events = [
        created("imp-1", "a", 0),
        event(EventKind::ItemStatus, at(1))
            .item("imp-1")
            .status("in_progress"),
        event(EventKind::ItemStatus, at(2))
            .item("imp-1")
            .status("done"),
        note("imp-1", "a", 3),
        note("imp-2", "a", 3),
        Event {
            item_id: None,
            ..note("x", "a", 4)
        },
    ];

    let info = current_items(&events);
    assert_eq!(info.len(), 2);
    assert_eq!(info["imp-1"].status, Supplied::Present(Some("done".into())));
    assert_eq!(info["imp-2"].status, Supplied::Present(None));
    assert_eq!(info["imp-1"].assignee, Supplied::Absent);
    assert_eq!(info["imp-2"].assignee, Supplied::Absent);
}

#[test]
fn current_items_title_is_the_first_present_one_and_an_empty_title_counts() {
    let events = [
        note("imp-1", "a", 0),
        note("imp-1", "a", 1).title(""),
        note("imp-1", "a", 2).title("real"),
        note("imp-2", "a", 0),
        note("imp-2", "a", 1).title("later"),
        note("imp-2", "a", 2).title("latest"),
        note("imp-3", "a", 0),
    ];

    let info = current_items(&events);
    assert_eq!(info["imp-1"].title.as_deref(), Some(""));
    assert_eq!(info["imp-2"].title.as_deref(), Some("later"));
    assert_eq!(info["imp-3"].title, None);
}

#[test]
fn current_items_makes_a_silent_in_progress_claim_stale_and_a_done_one_not() {
    let mut events = Vec::new();
    for (item, last_status) in [("imp-1", "in_progress"), ("imp-2", "done")] {
        events.push(claimed_at(item, "a", days_ago(5)));
        events.push(note_at(days_ago(5), "a", "a note", item));
        events.push(note_at(days_ago(4), "b", "a note", item));
        events.push(
            event(EventKind::ItemStatus, days_ago(4))
                .item(item)
                .agent("b")
                .status(last_status),
        );
    }

    let graph = Collaboration::build(&events, &current_items(&events), now());
    assert_eq!(graph.threads.len(), 2);
    let stale: Vec<(&str, bool)> = graph
        .threads
        .iter()
        .map(|t| (t.item_id.as_str(), t.stale))
        .collect();
    // The stale thread sorts first.
    assert_eq!(stale, [("imp-1", true), ("imp-2", false)]);
    assert_eq!(graph.threads[0].stale_agent.as_deref(), Some("a"));
    assert_eq!(graph.threads[0].status.as_deref(), Some("in_progress"));
    assert_eq!(graph.threads[1].status.as_deref(), Some("done"));
}

// -- open questions: Supplied assignee ----------------------------------------

#[rstest]
#[case::explicit_nobody(Supplied::Present(None), None)]
#[case::explicit_holder(held("a"), Some("a"))]
#[case::absent_falls_back_to_the_claim_event(Supplied::Absent, Some("a"))]
fn supplied_assignee_beats_the_claim_event(
    #[case] assignee: Supplied<String>,
    #[case] stale_agent: Option<&str>,
) {
    let events = silent_holder_events("imp-29");
    let info = items("imp-29", "claimed", assignee);

    let graph = Collaboration::build(&events, &info, now());
    assert_eq!(graph.threads.len(), 1);
    assert_eq!(graph.threads[0].stale_agent.as_deref(), stale_agent);
    assert_eq!(graph.threads[0].stale, stale_agent.is_some());
}

#[test]
fn an_absent_assignee_falls_back_to_the_latest_claim_or_handoff_agent() {
    // `b` takes the item over from `a` and then goes quiet too.
    let events = [
        claimed_at("imp-29", "a", days_ago(6)),
        note_at(days_ago(6), "a", "a note", "imp-29"),
        Event {
            at: days_ago(5),
            ..handoff("imp-29", "a", "b", 0)
        },
    ];
    let info = HashMap::from([(
        "imp-29".to_string(),
        ItemInfo {
            status: Supplied::Present(Some("in_progress".into())),
            ..ItemInfo::default()
        },
    )]);

    let graph = Collaboration::build(&events, &info, now());
    assert_eq!(graph.threads[0].stale_agent.as_deref(), Some("b"));
}

// -- open questions: a commit listed twice -------------------------------------

#[rstest]
#[case::later_names_an_agent(None, Some("a"), 0)]
#[case::later_names_nobody(Some("a"), None, 1)]
fn a_commit_listed_twice_with_one_sha_the_later_wins(
    #[case] first: Option<&str>,
    #[case] second: Option<&str>,
    #[case] co_commits: usize,
) {
    let with = |agent: Option<&str>, min: i64| Event {
        agent: agent.map(String::from),
        ..loose_commit("deadbee", min)
    };
    let events = [
        note("imp-16", "a", 1),
        note("imp-16", "b", 2),
        merged("imp-16", 155, "feat/imp-16", &["deadbee"], 30),
        with(first, 10),
        with(second, 20),
    ];

    let graph = build(&events);
    assert_eq!(graph.pairs.len(), 1);
    assert_eq!(graph.pairs[0].co_commits, co_commits);
}
