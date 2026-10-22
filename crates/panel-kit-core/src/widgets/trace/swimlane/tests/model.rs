//! The input vocabulary, the helpers the modes share, and the width contract.

use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::json;

use super::super::*;
use super::fixtures::*;
use crate::widgets::trace::model::{
    kinds_of, EdgeKind, Event, EventKind, LaneKind, SessionTree, Source, UnknownKind,
};
use crate::widgets::trace::query::{Filters, Tone};

// -- kinds and sources --------------------------------------------------------

#[test]
fn all_lists_every_kind_once_in_declaration_order() {
    assert_eq!(EventKind::ALL.len(), 20);
    for (index, kind) in EventKind::ALL.iter().enumerate() {
        assert_eq!(*kind as usize, index, "{kind}");
    }
    let names: BTreeSet<&str> = EventKind::ALL.iter().map(|kind| kind.as_str()).collect();
    assert_eq!(names.len(), 20);
}

#[test]
fn by_name_is_sorted_by_name_and_holds_every_kind() {
    let names: Vec<&str> = EventKind::BY_NAME
        .iter()
        .map(|kind| kind.as_str())
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
    let all: BTreeSet<&str> = EventKind::ALL.iter().map(|kind| kind.as_str()).collect();
    assert_eq!(names.into_iter().collect::<BTreeSet<_>>(), all);
}

#[test]
fn a_kind_round_trips_through_every_spelling() {
    for kind in EventKind::ALL {
        let name = kind.as_str();
        assert_eq!(EventKind::from_name(name), Some(kind));
        assert_eq!(name.parse::<EventKind>(), Ok(kind));
        assert_eq!(kind.to_string(), name);
        assert_eq!(serde_json::to_value(kind).unwrap(), json!(name));
        assert_eq!(
            serde_json::from_value::<EventKind>(json!(name)).unwrap(),
            kind
        );
    }
}

#[rstest]
#[case(EventKind::PrMerged, "pr merged")]
#[case(EventKind::BranchHead, "branch head")]
#[case(EventKind::Commit, "commit")]
#[case(EventKind::ItemReleased, "item released")]
fn a_label_is_the_name_with_spaces(#[case] kind: EventKind, #[case] want: &str) {
    assert_eq!(kind.label(), want);
}

#[test]
fn a_name_outside_the_vocabulary_is_an_error() {
    assert_eq!(
        "branch".parse::<EventKind>(),
        Err(UnknownKind("branch".into()))
    );
    assert_eq!(EventKind::from_name("Note"), None);
    assert_eq!(EventKind::from_name(""), None);
    assert!(UnknownKind("x".into()).to_string().contains("\"x\""));
}

#[test]
fn kinds_of_keeps_the_known_names_once_in_order() {
    assert_eq!(
        kinds_of(["note", "bogus", "commit", "note"]),
        vec![EventKind::Note, EventKind::Commit]
    );
    assert_eq!(kinds_of(Vec::<String>::new()), vec![]);
    assert_eq!(
        kinds_of(vec!["pr_merged".to_string()]),
        vec![EventKind::PrMerged]
    );
}

#[test]
fn sources_rank_ledger_run_git_github() {
    let ranks: Vec<(&str, u8)> = Source::ALL.iter().map(|s| (s.as_str(), s.rank())).collect();
    assert_eq!(
        ranks,
        vec![("ledger", 0), ("run", 1), ("git", 2), ("github", 3)]
    );
    assert_eq!(Source::default(), Source::Ledger);
}

#[test]
fn an_events_order_is_time_then_source_rank_then_seq() {
    let at = 5 * SEC;
    let base = Event::new("x", at, EventKind::Note);
    let mut events = [
        base.clone().source(Source::Git).seq(1),
        base.clone().source(Source::Ledger).seq(9),
        Event::new("y", at - 1, EventKind::Note).source(Source::Github),
        base.clone().source(Source::Ledger).seq(2),
    ];
    events.sort_by_key(|event| event.order_key());

    let order: Vec<(i64, u8, u64)> = events.iter().map(|event| event.order_key()).collect();
    assert_eq!(
        order,
        vec![(at - 1, 3, 0), (at, 0, 2), (at, 0, 9), (at, 2, 1)]
    );
}

// -- serde --------------------------------------------------------------------

#[test]
fn an_event_needs_only_id_time_and_kind() {
    let event: Event =
        serde_json::from_value(json!({"id": "ledger:1", "at": 7, "kind": "note"})).unwrap();

    assert_eq!(event, Event::new("ledger:1", 7, EventKind::Note));
}

#[test]
fn an_event_reads_every_field_it_names() {
    let wire = json!({
        "id": "git:abc", "at": 1_000_000, "seq": 4, "source": "git", "kind": "pr_merged",
        "agent": "omp", "item_id": "imp-1", "run_id": "r1", "branch": "feat/x",
        "commit": "abc", "pr": 12, "summary": "merged",
        "detail": {"status": "done", "title": "t", "from": "a", "commits": ["abc", "def"]}
    });
    let event: Event = serde_json::from_value(wire.clone()).unwrap();

    let want = Event::new("git:abc", 1_000_000, EventKind::PrMerged)
        .seq(4)
        .source(Source::Git)
        .agent("omp")
        .item("imp-1")
        .run("r1")
        .branch("feat/x")
        .commit("abc")
        .pr(12)
        .summary("merged")
        .status("done")
        .title("t")
        .from("a")
        .commits(["abc", "def"]);
    assert_eq!(event, want);
    assert_eq!(serde_json::to_value(&event).unwrap(), wire);
}

#[test]
fn unknown_fields_and_kinds_are_refused() {
    assert!(serde_json::from_value::<Event>(
        json!({"id": "a", "at": 1, "kind": "note", "extra": 1})
    )
    .is_err());
    assert!(
        serde_json::from_value::<Event>(json!({"id": "a", "at": 1, "kind": "branch"})).is_err()
    );
    assert!(serde_json::from_value::<Event>(
        json!({"id": "a", "at": 1, "kind": "note", "detail": {"nope": 1}})
    )
    .is_err());
}

#[test]
fn a_fork_tree_reads_lanes_and_edges_and_defaults_to_none() {
    let tree: SessionTree = serde_json::from_value(json!({
        "lanes": [{"id": "imp-1/collab/omp", "item_id": "imp-1", "kind": "collab",
                   "label": "omp", "from": 1, "to": 2}],
        "edges": [{"kind": "fork", "lane": "imp-1/collab/omp", "item_id": "imp-1", "at": 1}]
    }))
    .unwrap();

    assert_eq!(tree.lanes[0].kind, LaneKind::Collab);
    assert_eq!(
        (
            tree.lanes[0].chosen,
            tree.lanes[0].parent.clone(),
            tree.lanes[0].agent.clone()
        ),
        (None, None, None)
    );
    assert_eq!(tree.edges[0].kind, EdgeKind::Fork);
    assert_eq!(
        serde_json::from_value::<SessionTree>(json!({})).unwrap(),
        SessionTree::default()
    );
    assert_eq!(LaneKind::Handoff.as_str(), "handoff");
    assert_eq!(EdgeKind::Merge.as_str(), "merge");
}

#[test]
fn a_range_round_trips_and_refuses_extra_fields() {
    let range = Range::new(1, 2);
    let wire = serde_json::to_value(range).unwrap();

    assert_eq!(wire, json!({"since": 1, "now": 2}));
    assert_eq!(serde_json::from_value::<Range>(wire).unwrap(), range);
    assert!(serde_json::from_value::<Range>(json!({"since": 1, "now": 2, "until": 3})).is_err());
}

#[test]
fn warp_params_default_in_json() {
    let params: WarpParams = serde_json::from_value(json!({"left": 50.0})).unwrap();

    assert_eq!(
        params,
        WarpParams {
            left: 50.0,
            ..WarpParams::default()
        }
    );
}

// -- filters ------------------------------------------------------------------

#[test]
fn an_event_is_filtered_by_the_query_layers_filters() {
    let note = Event::new("a", 0, EventKind::Note)
        .agent("worker")
        .item("imp-1");
    let commit = Event::new("b", 0, EventKind::Commit);

    let by_agent = Filters {
        agent: Some("worker".into()),
        ..Filters::default()
    };
    let by_kind = Filters {
        kind: Some("commit".into()),
        ..Filters::default()
    };
    let by_item = Filters {
        item: Some("imp-1".into()),
        ..Filters::default()
    };
    assert!(by_agent.keep(&note) && !by_agent.keep(&commit));
    assert!(by_kind.keep(&commit) && !by_kind.keep(&note));
    assert!(by_item.keep(&note) && !by_item.keep(&commit));
    assert!(Filters::default().keep(&commit));
}

#[test]
fn laying_out_the_survivors_of_a_filter_is_the_hosts_pipeline() {
    let events = [
        ev("imp-1", 0.0),
        ev("imp-2", 5.0),
        loose_ev(EventKind::Commit, 3.0),
    ];
    let filters = Filters {
        item: Some("imp-2".into()),
        ..Filters::default()
    };

    let graph = Graph::layout(events.iter().filter(|event| filters.keep(*event)), &opts());

    assert_eq!(
        graph
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["imp-2"]
    );
    assert!(graph.strip.is_empty());
}

// -- width --------------------------------------------------------------------

#[rstest]
#[case(1440.0, 8.0, 8.0, 1424)]
#[case(1000.6, 8.0, 8.0, 984)]
#[case(600.0, 8.0, 8.0, 584)]
#[case(1800.0, 8.0, 8.0, 1784)]
#[case(0.0, 8.0, 8.0, 0)]
#[case(10.0, 8.0, 8.0, 0)]
#[case(16.0, 8.0, 8.0, 0)]
#[case(17.0, 8.0, 8.0, 1)]
#[case(733.5, 0.0, 0.5, 733)]
#[case(f64::NAN, 8.0, 8.0, 0)]
fn the_content_width_is_the_floored_box_inside_the_padding(
    #[case] client: f64,
    #[case] left: f64,
    #[case] right: f64,
    #[case] want: u32,
) {
    assert_eq!(content_width(client, left, right), want);
}

#[rstest]
#[case(&[0.0, 5.999], &[2])]
#[case(&[0.0, 6.0], &[1, 1])]
#[case(&[0.0, 6.001], &[1, 1])]
#[case(&[0.0, 5.0, 10.0], &[2, 1])]
#[case(&[0.0, 3.0, 5.9, 6.0], &[3, 1])]
fn events_merge_while_closer_than_six_px_to_the_first_of_their_cluster(
    #[case] xs: &[f64],
    #[case] counts: &[u32],
) {
    let events: Vec<Event> = xs.iter().map(|_| ev("imp-1", 0.0)).collect();

    let dots = cluster(events.iter().zip(xs).map(|(event, x)| (event, *x)));

    assert_eq!(dots.iter().map(|dot| dot.count).collect::<Vec<_>>(), counts);
    // The anchor is the first member's pixel and never slides.
    assert_eq!(dots[0].x, xs[0]);
}

// -- dots ---------------------------------------------------------------------

#[test]
fn a_dot_is_3px_and_a_cluster_5px() {
    let event = ev("imp-1", 0.0);
    let single = Dot::of(&event, 10.0);
    let burst = cluster([(&event, 10.0), (&event, 12.0)]).remove(0);

    assert_eq!(
        (single.radius(), single.clearance(), single.is_cluster()),
        (3.0, 5.75, false)
    );
    assert_eq!(
        (burst.radius(), burst.clearance(), burst.is_cluster()),
        (5.0, 7.75, true)
    );
}

#[rstest]
#[case(
    Some("worker"),
    Some("did a thing"),
    "Oct 1 00:00 · note · worker · did a thing"
)]
#[case(Some(""), Some("did a thing"), "Oct 1 00:00 · note · did a thing")]
#[case(Some("worker"), Some(""), "Oct 1 00:00 · note · worker")]
#[case(None, None, "Oct 1 00:00 · note")]
#[case(Some("  "), None, "Oct 1 00:00 · note ·   ")]
fn a_title_drops_the_parts_an_event_does_not_have(
    #[case] agent: Option<&str>,
    #[case] summary: Option<&str>,
    #[case] want: &str,
) {
    let mut event = Event::new("a", at(0.0), EventKind::Note);
    event.agent = agent.map(str::to_string);
    event.summary = summary.map(str::to_string);

    assert_eq!(title(&event), want);
}

#[test]
fn a_title_names_the_kind_in_words() {
    let event = Event::new("a", at(10.0), EventKind::PrMerged).agent("omp");

    assert_eq!(title(&event), "Oct 1 10:00 · pr merged · omp");
}

#[rstest]
#[case(None, "note")]
#[case(Some(""), "note")]
#[case(Some(" \n\t "), "note")]
#[case(Some("  one\n two\t\tthree  "), "one two three")]
#[case(Some("line one\nline two"), "line one line two")]
#[case(Some("a\u{a0}b"), "a\u{a0}b")]
#[case(Some("\u{a0}x\u{a0}"), "x")]
fn a_caption_is_the_summary_on_one_line(#[case] summary: Option<&str>, #[case] want: &str) {
    let mut event = Event::new("a", 0, EventKind::Note);
    event.summary = summary.map(str::to_string);

    assert_eq!(caption(&event), want);
}

#[test]
fn an_events_tone_follows_its_kind_and_for_item_status_its_status() {
    let tone = |kind: EventKind, status: Option<&str>| {
        let mut event = Event::new("a", 0, kind);
        event.detail.status = status.map(str::to_string);
        event.tone()
    };

    assert_eq!(tone(EventKind::RunFinished, None), Tone::Green);
    assert_eq!(tone(EventKind::RunFinished, Some("blocked")), Tone::Green);
    assert_eq!(tone(EventKind::ItemStatus, Some("blocked")), Tone::Red);
    assert_eq!(tone(EventKind::ItemStatus, Some("review")), Tone::Yellow);
    assert_eq!(tone(EventKind::ItemStatus, None), Tone::Hollow);
    assert_eq!(tone(EventKind::Handoff, None), Tone::Yellow);
    assert_eq!(tone(EventKind::Note, Some("done")), Tone::Hollow);
}

// -- the strip ----------------------------------------------------------------

#[test]
fn the_strip_groups_by_kind_in_name_order_and_expands_only_what_is_open() {
    let events = [
        loose_ev(EventKind::PrMerged, 1.0),
        loose_ev(EventKind::Commit, 1.0),
        loose_ev(EventKind::Commit, 2.0),
        loose_ev(EventKind::BranchHead, 1.0),
    ];
    let refs: Vec<&Event> = events.iter().collect();
    let kinds = [EventKind::PrMerged];

    let (rows, loose) = strip(&refs, |_| 20.0, LooseOpen::Kinds(&kinds));

    let seen: Vec<(EventKind, usize, bool, f64)> = rows
        .iter()
        .map(|row| (row.kind, row.count, row.open, row.y0))
        .collect();
    assert_eq!(
        seen,
        vec![
            (EventKind::BranchHead, 1, false, 0.0),
            (EventKind::Commit, 2, false, 14.0),
            (EventKind::PrMerged, 1, true, 28.0),
        ]
    );
    assert_eq!(loose.len(), 1);
    assert_eq!(
        (loose[0].kind, loose[0].y, loose[0].count),
        (EventKind::PrMerged, 35.0, 1)
    );
}

#[test]
fn a_cell_is_the_four_px_column_its_events_fall_in_and_darkens_with_the_count() {
    let events = [
        loose_ev(EventKind::Commit, 1.0),
        loose_ev(EventKind::Commit, 2.0),
        loose_ev(EventKind::Commit, 3.0),
    ];
    let refs: Vec<&Event> = events.iter().collect();
    let x_of = |event: &Event| match (event.at - since()) / HOUR {
        1 => 10.0,
        2 => 13.9,
        _ => 14.0,
    };

    let (rows, _) = strip(&refs, x_of, LooseOpen::None);

    let cells: Vec<(i64, u32, usize, f64)> = rows[0]
        .cells
        .iter()
        .map(|cell| (cell.x, cell.w, cell.count, cell.alpha))
        .collect();
    // Columns 8 and 12; peak 2: 0.25 + 0.75 / 2 = 0.625 rounds up to 0.63.
    assert_eq!(cells, vec![(8, 4, 1, 0.63), (12, 4, 2, 1.0)]);
}

#[test]
fn loose_open_reads_a_bool() {
    assert_eq!(LooseOpen::from(true), LooseOpen::All);
    assert_eq!(LooseOpen::from(false), LooseOpen::None);
    assert!(LooseOpen::All.is_open(EventKind::Note));
    assert!(!LooseOpen::None.is_open(EventKind::Note));
    assert!(!LooseOpen::Kinds(&[EventKind::Commit]).is_open(EventKind::Note));
}
