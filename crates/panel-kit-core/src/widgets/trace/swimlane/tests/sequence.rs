//! The LANES sequence diagram: who meets whom, in what order, on how many
//! lifelines.

use proptest::prelude::*;
use rstest::rstest;

use super::super::*;
use super::fixtures::{at, ev, loose_ev, since};
use crate::widgets::trace::model::{Event, EventKind, Source};
use crate::widgets::trace::query::LanePick;

const HOUR: i64 = 3_600_000_000;

fn note(agent: &str, item: &str, hour: f64) -> Event {
    Event {
        agent: Some(agent.into()),
        ..ev(item, hour)
    }
}

fn quiet() -> SequenceOptions {
    SequenceOptions::default().activity(Some(false))
}

fn build(events: &[Event]) -> Sequence {
    sequence(events, |_| true, &quiet())
}

/// `(from, to, kind)` of every edge, by agent name.
fn edges(seq: &Sequence) -> Vec<(&str, &str, EdgeKind)> {
    seq.rows
        .iter()
        .filter_map(|row| match *row {
            SeqRow::Edge {
                from_lane,
                to_lane,
                kind,
                ..
            } => Some((
                seq.lanes[from_lane as usize].agent.as_str(),
                seq.lanes[to_lane as usize].agent.as_str(),
                kind,
            )),
            _ => None,
        })
        .collect()
}

fn agents(seq: &Sequence) -> Vec<&str> {
    seq.lanes.iter().map(|lane| lane.agent.as_str()).collect()
}

fn not_a_day(seq: &Sequence) -> Vec<&SeqRow> {
    seq.rows
        .iter()
        .filter(|row| !matches!(row, SeqRow::Day { .. }))
        .collect()
}

// -- edges --------------------------------------------------------------------

#[test]
fn a_handoff_runs_from_the_previous_holder_to_the_new_one() {
    let handoff = Event {
        kind: EventKind::Handoff,
        ..note("bob", "imp-1", 2.0).from("amy")
    };
    let seq = build(&[note("amy", "imp-1", 1.0), handoff]);

    assert_eq!(edges(&seq), [("amy", "bob", EdgeKind::Handoff)]);
    assert_eq!(agents(&seq), ["amy", "bob"]);
}

#[test]
fn a_handoff_from_oneself_or_from_nobody_is_no_handoff() {
    let to_self = Event {
        kind: EventKind::Handoff,
        ..note("amy", "imp-1", 1.0).from("amy")
    };
    let from_nobody = Event {
        kind: EventKind::Handoff,
        ..note("bob", "imp-2", 2.0)
    };

    let seq = build(&[to_self, from_nobody]);

    assert_eq!(edges(&seq), []);
    assert!(seq.is_empty());
}

#[test]
fn a_baton_pass_is_the_next_event_of_an_item_by_someone_else() {
    let seq = build(&[
        note("amy", "imp-1", 1.0),
        note("amy", "imp-1", 2.0),
        note("bob", "imp-1", 3.0),
        note("amy", "imp-1", 4.0),
    ]);

    assert_eq!(
        edges(&seq),
        [
            ("amy", "bob", EdgeKind::Baton),
            ("bob", "amy", EdgeKind::Baton)
        ]
    );
}

#[test]
fn the_batons_of_two_items_do_not_mix() {
    let seq = build(&[
        note("amy", "imp-1", 1.0),
        note("bob", "imp-2", 2.0),
        note("amy", "imp-1", 3.0),
        note("bob", "imp-2", 4.0),
    ]);

    assert_eq!(edges(&seq), []);
}

#[test]
fn a_review_runs_from_the_reviewer_to_the_holder_and_leaves_the_baton_alone() {
    let review = Event {
        kind: EventKind::PrReviewed,
        ..note("bob", "imp-1", 2.0)
    };
    let seq = build(&[note("amy", "imp-1", 1.0), review, note("amy", "imp-1", 3.0)]);

    assert_eq!(edges(&seq), [("bob", "amy", EdgeKind::Review)]);
}

#[test]
fn a_handoff_is_not_also_counted_as_a_baton_pass() {
    let handoff = Event {
        kind: EventKind::Handoff,
        ..note("bob", "imp-1", 2.0).from("amy")
    };
    let seq = build(&[
        note("amy", "imp-1", 1.0),
        handoff,
        note("bob", "imp-1", 3.0),
    ]);

    assert_eq!(edges(&seq), [("amy", "bob", EdgeKind::Handoff)]);
}

#[test]
fn a_blank_agent_is_nobody_and_passes_no_baton() {
    let blank = Event {
        agent: Some("  ".into()),
        ..ev("imp-1", 2.0)
    };
    let seq = build(&[note("amy", "imp-1", 1.0), blank, note("amy", "imp-1", 3.0)]);

    assert_eq!(edges(&seq), []);
    assert_eq!(seq.hidden, 1);
}

// -- broadcasts and dots ------------------------------------------------------

#[test]
fn an_itemless_branch_head_commit_or_merge_is_a_broadcast() {
    let events = [
        loose_ev(EventKind::BranchHead, 1.0),
        loose_ev(EventKind::Commit, 2.0),
        loose_ev(EventKind::PrMerged, 3.0),
    ];

    let seq = build(&events);

    let rows = not_a_day(&seq);
    assert_eq!(rows.len(), 3);
    assert!(rows
        .iter()
        .all(|row| matches!(row, SeqRow::Broadcast { .. })));
    assert!(seq.lanes.is_empty(), "a broadcast opens no lifeline");
}

#[test]
fn a_commit_on_an_item_or_a_loose_note_is_not_a_broadcast() {
    let commit_on_item = Event {
        kind: EventKind::Commit,
        ..note("amy", "imp-1", 1.0)
    };
    let loose_note = loose_ev(EventKind::Note, 2.0);

    let seq = sequence(
        &[commit_on_item, loose_note],
        |_| true,
        &SequenceOptions::default().activity(Some(true)),
    );

    assert!(not_a_day(&seq)
        .iter()
        .all(|row| matches!(row, SeqRow::Dot { .. })));
}

#[test]
fn plain_activity_is_a_dot_only_while_activity_is_on() {
    let events = [note("amy", "imp-1", 1.0), note("amy", "imp-1", 2.0)];

    let off = sequence(&events, |_| true, &quiet());
    let on = sequence(
        &events,
        |_| true,
        &SequenceOptions::default().activity(Some(true)),
    );

    assert!(off.is_empty() && off.lanes.is_empty());
    assert_eq!(
        off.quiet, 2,
        "the quiet events are counted even when hidden"
    );
    assert_eq!(agents(&on), ["amy"]);
    assert_eq!(on.drawn, 2);
    assert_eq!((on.lanes[0].count, on.lanes[0].first), (2, at(1.0)));
}

#[test]
fn activity_defaults_on_for_a_few_events_and_off_for_many() {
    let few: Vec<Event> = (0..ACTIVITY_AUTO_MAX)
        .map(|n| note("amy", "imp-1", n as f64 / 1000.0))
        .collect();
    let many: Vec<Event> = (0..=ACTIVITY_AUTO_MAX)
        .map(|n| note("amy", "imp-1", n as f64 / 1000.0))
        .collect();

    let by_default = |events: &[Event]| sequence(events, |_| true, &SequenceOptions::default());

    assert!(by_default(&few).activity);
    assert!(!by_default(&many).activity);
    let forced = sequence(
        &many,
        |_| true,
        &SequenceOptions::default().activity(Some(true)),
    );
    assert!(forced.activity);
}

// -- lifelines ----------------------------------------------------------------

#[test]
fn lifelines_stand_in_order_of_first_interaction_with_their_counts() {
    let seq = build(&[
        note("cat", "imp-2", 1.0),
        note("dan", "imp-2", 2.0),
        note("amy", "imp-1", 3.0),
        note("bob", "imp-1", 4.0),
        note("amy", "imp-1", 5.0),
    ]);

    assert_eq!(agents(&seq), ["cat", "dan", "amy", "bob"]);
    let at_index: Vec<usize> = seq.lanes.iter().map(|lane| lane.x_index).collect();
    assert_eq!(at_index, [0, 1, 2, 3]);
    let amy = &seq.lanes[2];
    assert_eq!((amy.count, amy.first, amy.last), (2, at(4.0), at(5.0)));
}

/// amy and bob meet three times, amy and cat twice, bob and dan and bob and
/// eve once: counts amy 4, bob 4, cat 2, dan 1, eve 1.
fn crowd() -> Vec<Event> {
    vec![
        note("amy", "imp-1", 1.0),
        note("bob", "imp-1", 2.0),
        note("amy", "imp-1", 3.0),
        note("amy", "imp-2", 4.0),
        note("cat", "imp-2", 5.0),
        note("amy", "imp-2", 6.0),
        note("bob", "imp-3", 7.0),
        note("dan", "imp-3", 8.0),
        note("bob", "imp-4", 9.0),
        note("eve", "imp-4", 10.0),
        note("cat", "imp-5", 11.0),
        note("dan", "imp-5", 12.0),
    ]
}

fn only(names: &[&str]) -> LanePick {
    LanePick::Only(names.iter().map(|name| name.to_string()).collect())
}

/// `n` agents (even), each pair meeting once on its own item: every agent
/// has one interaction.
fn pairs_of(n: usize) -> Sequence {
    let events: Vec<Event> = (0..n)
        .map(|i| note(&format!("a{i:02}"), &format!("imp-{}", i / 2), i as f64))
        .collect();
    build(&events)
}

#[test]
fn every_agent_that_interacted_has_a_lane_before_any_is_folded() {
    let seq = build(&crowd());

    assert_eq!(agents(&seq), ["amy", "bob", "cat", "dan", "eve"]);
    assert!(!seq.folds());
    let roster: Vec<(&str, usize, bool)> = seq
        .roster
        .iter()
        .map(|r| (r.agent.as_str(), r.count, r.shown))
        .collect();
    assert_eq!(
        roster,
        [
            ("amy", 4, true),
            ("bob", 4, true),
            ("cat", 3, true),
            ("dan", 2, true),
            ("eve", 1, true)
        ],
        "busiest first, then earliest, then by name"
    );
}

#[test]
fn the_lanes_a_pick_names_stand_and_the_rest_fold_into_one_narrow_lane() {
    let seq = build(&crowd()).show(&only(&["amy", "bob"]));

    assert_eq!(agents(&seq), ["amy", "bob", "+3"]);
    assert!(seq.folds());
    let fold = &seq.lanes[2];
    assert_eq!(fold.folded, ["cat", "dan", "eve"]);
    assert_eq!(fold.x_index, 2);
    assert_eq!(fold.count, 3 + 2 + 1);
    assert_eq!((fold.first, fold.last), (at(5.0), at(12.0)));
    assert!(seq.lanes[..2].iter().all(|lane| lane.folded.is_empty()));
    let shown: Vec<(&str, bool)> = seq
        .roster
        .iter()
        .map(|r| (r.agent.as_str(), r.shown))
        .collect();
    assert_eq!(
        shown,
        [
            ("amy", true),
            ("bob", true),
            ("cat", false),
            ("dan", false),
            ("eve", false)
        ],
        "the menu still lists every agent"
    );
}

#[test]
fn an_edge_to_a_folded_agent_ends_on_the_folded_lane() {
    let seq = build(&crowd()).show(&only(&["amy", "bob"]));

    let reaches: Vec<(&str, &str)> = edges(&seq).iter().map(|(a, b, _)| (*a, *b)).collect();
    assert_eq!(
        reaches,
        [
            ("amy", "bob"),
            ("bob", "amy"),
            ("amy", "+3"),
            ("+3", "amy"),
            ("bob", "+3"),
            ("bob", "+3"),
            ("+3", "+3"),
        ],
        "no interaction disappears; two folded agents meet on the fold"
    );

    // Folded agents on both ends, and lanes whose index moves when others fold.
    let apart = build(&crowd()).show(&only(&["amy", "dan"]));
    let reaches: Vec<(&str, &str)> = edges(&apart).iter().map(|(a, b, _)| (*a, *b)).collect();
    assert_eq!(
        reaches,
        [
            ("amy", "+3"),
            ("+3", "amy"),
            ("amy", "+3"),
            ("+3", "amy"),
            ("+3", "dan"),
            ("+3", "+3"),
            ("+3", "dan"),
        ]
    );
}

#[test]
fn folding_moves_no_row_and_changes_no_count() {
    let base = build(&crowd());
    let shown = base.show(&only(&["bob"]));

    assert_eq!(shown.rows.len(), base.rows.len());
    let at_of = |seq: &Sequence| {
        seq.rows
            .iter()
            .map(|row| (row.at(), row.event()))
            .collect::<Vec<_>>()
    };
    assert_eq!(at_of(&shown), at_of(&base));
    assert_eq!(
        (shown.read, shown.drawn, shown.quiet, shown.hidden),
        (base.read, base.drawn, base.quiet, base.hidden)
    );
}

#[test]
fn the_default_draws_every_lane_up_to_the_cap_and_the_busiest_eight_above_it() {
    let at_cap = pairs_of(LANE_CAP).show(&LanePick::Auto);
    assert_eq!(at_cap.lanes.len(), LANE_CAP);
    assert!(!at_cap.folds());

    let mut events: Vec<Event> = (0..LANE_CAP)
        .map(|i| note(&format!("a{i:02}"), &format!("imp-{}", i / 2), i as f64))
        .collect();
    events.push(note("a16", "imp-7", 16.0));
    let over = build(&events).show(&LanePick::Auto);
    assert_eq!(over.lanes.len(), BUSIEST_LANES + 1, "eight and the fold");
    assert_eq!(
        over.lanes[BUSIEST_LANES].agent,
        format!("+{}", LANE_CAP + 1 - BUSIEST_LANES)
    );
    assert_eq!(
        over.roster.iter().filter(|r| r.shown).count(),
        BUSIEST_LANES
    );
}

#[test]
fn the_busiest_eight_are_the_ones_with_the_most_interactions() {
    // a14 and zed end with three interactions, a15 with two, the rest with one.
    let mut events: Vec<Event> = (0..16)
        .map(|i| note(&format!("a{i:02}"), &format!("imp-{}", i / 2), i as f64))
        .collect();
    events.push(note("zed", "imp-7", 16.0));
    events.push(note("a14", "imp-7", 17.0));
    events.push(note("zed", "imp-7", 18.0));

    let over = build(&events).show(&LanePick::Auto);

    let drawn: Vec<&str> = over.lanes.iter().map(|lane| lane.agent.as_str()).collect();
    assert!(
        drawn.contains(&"zed") && drawn.contains(&"a14") && drawn.contains(&"a15"),
        "{drawn:?}"
    );
    assert_eq!(
        over.roster[0].agent, "a14",
        "a tie goes to the earlier lane"
    );
}

#[test]
fn all_draws_every_lane_above_the_cap_and_an_empty_pick_folds_them_all() {
    let base = pairs_of(40);

    let all = base.show(&LanePick::All);
    assert_eq!(all.lanes.len(), 40);
    assert!(!all.folds());

    let none = base.show(&LanePick::Only(Default::default()));
    assert_eq!(agents(&none), ["+40"]);
    assert_eq!(none.lanes[0].folded.len(), 40);
    assert!(none.rows.iter().all(|row| match row {
        SeqRow::Edge {
            from_lane, to_lane, ..
        } => (*from_lane, *to_lane) == (0, 0),
        _ => true,
    }));
    assert_eq!(
        base.show(&only(&["nobody"])).lanes.len(),
        1,
        "unknown names pick nothing"
    );
}

// -- columns ------------------------------------------------------------------

#[test]
fn columns_share_the_room_and_never_shrink_below_the_minimum() {
    let three = pairs_of(4).show(&only(&["a00", "a01", "a02"]));
    let room = three.columns(1_000.0);
    // Three lanes and the folded one: (1000 - gutter - fold) / 3, floored.
    assert_eq!(room.lanes, 3);
    assert_eq!(room.lane_w, ((1_000.0 - GUTTER_W - FOLD_W) / 3.0).floor());
    assert_eq!(room.fold_w, FOLD_W);
    assert!(!room.scrolls);

    let crowded = pairs_of(10).columns(600.0);
    assert_eq!(crowded.lane_w, LANE_MIN_W);
    assert_eq!(crowded.width, 10.0 * LANE_MIN_W);
    assert!(crowded.scrolls, "ten minimum columns overflow 600px");
}

#[test]
fn the_body_scrolls_exactly_when_the_minimums_do_not_fit() {
    let four = pairs_of(4);

    assert!(!four.columns(GUTTER_W + 4.0 * LANE_MIN_W).scrolls);
    assert!(four.columns(GUTTER_W + 4.0 * LANE_MIN_W - 1.0).scrolls);
    assert_eq!(four.columns(GUTTER_W + 4.0 * LANE_MIN_W).lane_w, LANE_MIN_W);
}

#[test]
fn a_lifeline_sits_mid_column_and_the_folded_one_in_its_narrow_column() {
    let seq = pairs_of(4).show(&only(&["a00", "a01"]));
    let cols = seq.columns(600.0);

    assert_eq!((cols.lanes, cols.lane_w, cols.fold_w), (2, 244.0, FOLD_W));
    assert_eq!(cols.width, 2.0 * 244.0 + FOLD_W);
    assert!(!cols.scrolls, "the columns fill the room exactly");
    assert_eq!(
        (cols.x(0), cols.x(1), cols.x(2)),
        (122.0, 366.0, 488.0 + FOLD_W / 2.0)
    );
    assert_eq!((cols.span(0), cols.span(2)), (244.0, FOLD_W));
}

#[test]
fn the_count_goes_before_the_name_does() {
    assert!(shows_count(LANE_MIN_W));
    assert!(shows_count(COUNT_MIN_W));
    assert!(!shows_count(COUNT_MIN_W - 1.0));
    assert!(
        !shows_count(FOLD_W),
        "the narrow fold lane keeps only its name"
    );
}

// -- rows ---------------------------------------------------------------------

#[test]
fn rows_run_in_time_order_whatever_order_the_events_arrive_in() {
    let events = [
        note("amy", "imp-1", 5.0),
        note("bob", "imp-1", 3.0),
        note("amy", "imp-1", 1.0),
    ];

    let seq = build(&events);

    let times: Vec<i64> = not_a_day(&seq).iter().map(|row| row.at()).collect();
    assert_eq!(times, [at(3.0), at(5.0)]);
    assert_eq!(
        edges(&seq),
        [
            ("amy", "bob", EdgeKind::Baton),
            ("bob", "amy", EdgeKind::Baton)
        ]
    );
}

#[test]
fn rows_index_the_events_that_were_read_not_the_events_that_were_kept() {
    let events = [
        note("amy", "imp-1", 1.0),
        note("zed", "imp-9", 1.5),
        note("bob", "imp-1", 2.0),
    ];

    let seq = sequence(
        &events,
        |event| event.agent.as_deref() != Some("zed"),
        &quiet(),
    );

    let SeqRow::Edge { event, .. } = *not_a_day(&seq)[0] else {
        panic!("an edge");
    };
    assert_eq!(events[event as usize].id, events[2].id);
    assert_eq!(seq.read, 2);
}

#[test]
fn the_pitch_is_fixed_so_a_row_sits_at_its_index_times_the_pitch() {
    let seq = build(&crowd());

    assert_eq!(ROW_PITCH, 22.0);
    assert_eq!(seq.height(), seq.rows.len() as f64 * 22.0);
    assert_eq!(seq.rows.len(), seq.drawn + 1, "one day separator");
}

#[test]
fn a_day_separator_opens_each_utc_day_and_is_a_row_of_its_own() {
    let seq = build(&[
        note("amy", "imp-1", 1.0),
        note("bob", "imp-1", 2.0),
        note("amy", "imp-1", 26.0),
    ]);

    let labels: Vec<String> = seq
        .rows
        .iter()
        .filter_map(|row| match row {
            SeqRow::Day { at } => Some(day_label(*at)),
            _ => None,
        })
        .collect();
    assert_eq!(labels, ["10-01", "10-02"]);
    assert!(matches!(seq.rows[0], SeqRow::Day { .. }));
    assert!(matches!(seq.rows[2], SeqRow::Day { at } if at == since() + 24 * HOUR));
    assert_eq!(seq.rows.len(), 4);
}

#[test]
fn the_last_second_of_a_day_and_the_first_of_the_next_are_two_days() {
    let a = Event::new("a", since() + 24 * HOUR - 1_000_000, EventKind::Commit);
    let b = Event::new("b", since() + 24 * HOUR, EventKind::Commit);

    let seq = build(&[a, b]);

    assert_eq!(
        seq.rows
            .iter()
            .filter(|row| matches!(row, SeqRow::Day { .. }))
            .count(),
        2
    );
}

#[test]
fn stamps_read_hours_and_minutes_in_utc() {
    assert_eq!(clock_label(at(2.0)), "02:00");
    assert_eq!(clock_label(since() + 13 * HOUR + 7 * 60_000_000), "13:07");
    assert_eq!(day_label(at(0.0)), "10-01");
}

// -- one lifeline's view ------------------------------------------------------

#[test]
fn touching_a_lifeline_keeps_its_edges_and_dots_and_redraws_the_days() {
    let events = [
        note("amy", "imp-1", 1.0),
        note("bob", "imp-1", 2.0),
        note("cat", "imp-2", 3.0),
        note("dan", "imp-2", 4.0),
        note("cat", "imp-2", 27.0),
        loose_ev(EventKind::Commit, 28.0),
    ];
    let seq = sequence(
        &events,
        |_| true,
        &SequenceOptions::default().activity(Some(true)),
    );
    let cat = seq
        .lanes
        .iter()
        .position(|lane| lane.agent == "cat")
        .unwrap();

    let only = seq.touching(cat);

    assert_eq!(only.lanes, seq.lanes, "every lifeline is still drawn");
    assert!(only.rows.iter().all(|row| matches!(
        row,
        SeqRow::Day { .. } | SeqRow::Edge { .. } | SeqRow::Dot { .. }
    )));
    assert!(only
        .rows
        .iter()
        .filter(|row| !matches!(row, SeqRow::Day { .. }))
        .all(|row| row.touches(cat as u32)));
    assert_eq!(only.drawn, 3, "cat's dot, the edge to dan, the edge back");
    assert_eq!(
        only.rows
            .iter()
            .filter(|row| matches!(row, SeqRow::Day { .. }))
            .count(),
        2
    );
    assert_eq!(only.read, seq.read);
}

// -- captions -----------------------------------------------------------------

#[test]
fn a_caption_names_the_kind_the_target_and_the_first_line() {
    let merged = Event {
        kind: EventKind::PrMerged,
        pr: Some(12),
        summary: Some("ship it\nsecond line".into()),
        ..loose_ev(EventKind::PrMerged, 1.0)
    };
    let head = Event {
        branch: Some("main".into()),
        commit: Some("0123456789abcdef".into()),
        ..loose_ev(EventKind::BranchHead, 1.0)
    };

    assert_eq!(label(&merged), "pr merged #12 ship it");
    assert_eq!(label(&head), "branch main @ 01234567");
    assert_eq!(label(&note("amy", "imp-7", 1.0)), "note imp-7");
}

#[test]
fn a_branch_head_is_named_by_its_branch_with_a_short_sha() {
    let head = |branch: Option<&str>, commit: Option<&str>| Event {
        branch: branch.map(String::from),
        commit: commit.map(String::from),
        ..Event::new("h", at(0.0), EventKind::BranchHead)
    };

    assert_eq!(
        label(&head(Some("fix/imp-7-thing"), Some("0123456789abcdef"))),
        "branch fix/imp-7-thing @ 01234567"
    );
    assert_eq!(label(&head(Some("x"), None)), "branch x @ ");
    assert_eq!(label(&head(Some("x"), Some("abc"))), "branch x @ abc");
    assert_eq!(
        label(&head(None, Some("0123456789abcdef"))),
        "branch head 0123456789ab",
        "no branch: it reads like any event"
    );
}

#[rstest]
#[case::item_only(Some("imp-1"), None, None, "imp-1")]
#[case::commit_is_cut_to_twelve(None, Some("0123456789abcdef0123"), None, "0123456789ab")]
#[case::short_commit_whole(None, Some("abc"), None, "abc")]
#[case::pr(None, None, Some(7), "#7")]
#[case::pr_wins_over_commit(None, Some("0123456789abcdef0123"), Some(7), "#7")]
#[case::pr_wins_over_item(Some("imp-1"), None, Some(7), "#7")]
#[case::commit_wins_over_item(Some("imp-1"), Some("0123456789abcdef"), None, "0123456789ab")]
#[case::nothing_no_link_word(None, None, None, "")]
fn a_caption_points_at_a_pull_request_then_a_commit_then_an_item(
    #[case] item: Option<&str>,
    #[case] commit: Option<&str>,
    #[case] pr: Option<u64>,
    #[case] link: &str,
) {
    let bare = Event {
        item_id: item.map(String::from),
        commit: commit.map(String::from),
        pr,
        ..Event::new("e", at(0.0), EventKind::Note)
    };
    let wanted = |tail: &str| {
        ["note", link, tail]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };

    assert_eq!(label(&bare), wanted(""));
    let said = Event {
        summary: Some("said".into()),
        ..bare
    };
    assert_eq!(label(&said), wanted("said"));
}

#[test]
fn a_caption_keeps_the_first_line_of_a_summary_trimmed_and_adds_nothing_for_an_empty_one() {
    let said = |text: &str| Event {
        summary: Some(text.into()),
        ..note("worker", "imp-1", 0.0)
    };

    assert_eq!(
        label(&said("  first line  \nsecond line\nthird")),
        "note imp-1 first line"
    );
    assert_eq!(label(&said("\n second")), "note imp-1");
    let long = "Imp.Predict.ReActV2.StepError: the model refused. ".repeat(6);
    assert_eq!(label(&said(&long)), format!("note imp-1 {}", long.trim()));
}

#[rstest]
#[case::kind_and_time(None, None, None, "note · 2026-10-01T00:00:00Z")]
#[case::agent(Some("worker"), None, None, "note · worker · 2026-10-01T00:00:00Z")]
#[case::agent_and_item(
    Some("worker"),
    Some("imp-7"),
    None,
    "note · worker · imp-7 · 2026-10-01T00:00:00Z"
)]
#[case::item_only(None, Some("imp-7"), None, "note · imp-7 · 2026-10-01T00:00:00Z")]
#[case::branch_only(None, None, Some("feat/x"), "note · feat/x · 2026-10-01T00:00:00Z")]
#[case::everything(
    Some("worker"),
    Some("imp-7"),
    Some("feat/x"),
    "note · worker · imp-7 · feat/x · 2026-10-01T00:00:00Z"
)]
fn the_tooltip_drops_the_parts_an_event_does_not_have(
    #[case] agent: Option<&str>,
    #[case] item: Option<&str>,
    #[case] branch: Option<&str>,
    #[case] expected: &str,
) {
    let event = Event {
        agent: agent.map(String::from),
        item_id: item.map(String::from),
        branch: branch.map(String::from),
        ..Event::new("e", at(0.0), EventKind::Note)
    };

    assert_eq!(detail(&event), expected);
    let merged = Event::new("m", at(24.0), EventKind::PrMerged);
    assert_eq!(detail(&merged), "pr_merged · 2026-10-02T00:00:00Z");
}

#[rstest]
#[case("a b/c", "a-b-c")]
#[case("a  b", "a-b")]
#[case("a /  / b", "a-b")]
#[case("a_b-c", "a_b-c")]
#[case("a--b", "a--b")]
#[case("ledger:12", "ledger-12")]
#[case("héllo", "h-llo")]
#[case("", "")]
#[case("/x/", "-x-")]
fn a_slug_collapses_runs_of_disallowed_characters(#[case] text: &str, #[case] wanted: &str) {
    assert_eq!(slug(text), wanted);
}

// -- properties ---------------------------------------------------------------

fn arb_event() -> impl Strategy<Value = Event> {
    let text = || {
        prop::collection::vec(prop::char::range('a', 'd'), 0..2)
            .prop_map(|c| c.into_iter().collect::<String>())
    };
    (
        text(),
        prop::option::of(text()),
        prop::option::of(text()),
        0..Source::ALL.len(),
        0..EventKind::ALL.len(),
        0i64..5_000_000_000,
    )
        .prop_map(|(id, agent, item, source, kind, at)| Event {
            agent,
            item_id: item,
            source: Source::ALL[source],
            ..Event::new(id, since() + at, EventKind::ALL[kind])
        })
}

proptest! {
    #[test]
    fn every_kept_event_is_drawn_counted_or_hidden_and_the_lanes_add_up(
        events in prop::collection::vec(arb_event(), 0..40),
    ) {
        let seq = build(&events);

        let (mut edge_ends, mut bars) = (0usize, 0usize);
        for row in &seq.rows {
            match row {
                SeqRow::Edge { from_lane, to_lane, event, .. } => {
                    edge_ends += 2;
                    prop_assert!((*from_lane as usize) < seq.lanes.len());
                    prop_assert!((*to_lane as usize) < seq.lanes.len());
                    prop_assert!((*event as usize) < events.len());
                }
                SeqRow::Broadcast { event, .. } => {
                    bars += 1;
                    prop_assert!((*event as usize) < events.len());
                }
                SeqRow::Dot { .. } | SeqRow::Day { .. } => {}
            }
        }
        prop_assert_eq!(seq.read, events.len());
        prop_assert_eq!(seq.drawn, edge_ends / 2 + bars);
        prop_assert_eq!(seq.drawn + seq.quiet + seq.hidden, seq.read);
        let counted: usize = seq.lanes.iter().map(|lane| lane.count).sum();
        prop_assert_eq!(counted, edge_ends);
        prop_assert!(seq.rows.windows(2).all(|pair| pair[0].at() <= pair[1].at()));
        for (at, lane) in seq.lanes.iter().enumerate() {
            prop_assert_eq!(lane.x_index, at);
            prop_assert!(!lane.agent.trim().is_empty(), "a blank agent is never a lane");
        }
    }

    #[test]
    fn folding_any_set_of_lanes_keeps_every_row_and_every_agent_in_the_roster(
        events in prop::collection::vec(arb_event(), 0..40),
        keep in prop::collection::btree_set(prop::char::range('a', 'd').prop_map(String::from), 0..4),
    ) {
        let base = build(&events);
        let shown = base.show(&LanePick::Only(keep.clone()));

        prop_assert_eq!(shown.rows.len(), base.rows.len());
        prop_assert_eq!(shown.roster.len(), base.lanes.len());
        let drawn = shown.roster.iter().filter(|r| r.shown).count();
        prop_assert_eq!(drawn, base.lanes.iter().filter(|l| keep.contains(&l.agent)).count());
        let folded = base.lanes.len() - drawn;
        prop_assert_eq!(shown.folds(), folded > 0);
        prop_assert_eq!(shown.lanes.len(), drawn + usize::from(folded > 0));
        for row in &shown.rows {
            if let SeqRow::Edge { from_lane, to_lane, .. } = row {
                prop_assert!((*from_lane as usize) < shown.lanes.len());
                prop_assert!((*to_lane as usize) < shown.lanes.len());
            }
        }
    }

    #[test]
    fn slug_is_idempotent_and_never_leaves_a_disallowed_character(
        chars in prop::collection::vec(any::<char>(), 0..16),
    ) {
        let text: String = chars.into_iter().collect();
        let once = slug(&text);

        prop_assert!(once.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'));
        prop_assert_eq!(slug(&once), once);
    }
}
