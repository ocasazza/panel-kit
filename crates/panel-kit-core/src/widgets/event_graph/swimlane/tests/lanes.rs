//! The model facts behind `activity_live_test.exs` ("lanes", "filters",
//! "labels", "the filter row"): the LANES-mode `split`, `node` and `slug`.

use proptest::prelude::*;
use rstest::rstest;

use super::super::*;
use super::fixtures::{at, uid};
use crate::widgets::event_graph::model::{Event, EventKind, Source};
use crate::widgets::event_graph::query::Filters;

// -- fixtures -----------------------------------------------------------------

/// The fixture event of the live test: a ledger note by `agent`, `text` as summary.
fn event(agent: Option<&str>, text: &str) -> Event {
    let base = Event::new(uid(), at(0.0), EventKind::Note).summary(text);
    Event {
        agent: agent.map(String::from),
        ..base
    }
}

fn on(item: &str, agent: &str, text: &str) -> Event {
    Event {
        item_id: Some(item.to_string()),
        ..event(Some(agent), text)
    }
}

fn from_source(source: Source, agent: Option<&str>) -> Event {
    Event {
        source,
        ..event(agent, "x")
    }
}

fn lane_ids(lanes: &[ActivityLane]) -> Vec<&str> {
    lanes.iter().map(|lane| lane.id.as_str()).collect()
}

fn lane_labels(lanes: &[ActivityLane]) -> Vec<&str> {
    lanes.iter().map(|lane| lane.label.as_str()).collect()
}

fn node_labels(lane: &ActivityLane) -> Vec<&str> {
    lane.nodes.iter().map(|node| node.label.as_str()).collect()
}

fn filtered(events: &[Event], filters: &Filters) -> Split {
    split(events.iter().filter(|event| filters.keep(*event)))
}

// -- lanes --------------------------------------------------------------------

#[test]
fn one_lane_per_agent_each_labelled_with_its_name() {
    let events = [
        event(Some("worker"), "one"),
        event(Some("worker"), "two"),
        event(Some("omp"), "three"),
    ];

    let lanes = split(&events);
    // Two lanes for two agents, not one lane per event.
    assert_eq!(lane_ids(&lanes.lanes), ["omp", "worker"]);
    assert_eq!(lane_labels(&lanes.lanes), ["omp", "worker"]);
    assert_eq!(lanes.lanes[0].nodes.len(), 1);
    assert_eq!(lanes.lanes[1].nodes.len(), 2);
    assert_eq!(lanes.unattributed, []);
}

#[test]
fn an_event_with_no_agent_goes_to_the_unattributed_lanes() {
    let events = [event(Some("worker"), "x"), event(None, "a commit")];

    let lanes = split(&events);
    assert_eq!(lane_ids(&lanes.lanes), ["worker"]);
    assert_eq!(lanes.unattributed.len(), 1);
    assert_eq!(lanes.unattributed_count(), 1);
}

#[test]
fn an_unattributed_lane_carries_the_event_when_opened() {
    let lanes = split(&[event(None, "a commit")]);

    assert_eq!(lanes.lanes, []);
    assert_eq!(node_labels(&lanes.unattributed[0]), ["a commit"]);
}

#[test]
fn the_unattributed_count_is_events_not_lanes() {
    let events = [event(None, "one"), event(None, "two"), event(None, "three")];

    let lanes = split(&events);
    assert_eq!(lanes.unattributed.len(), 1);
    assert_eq!(lanes.unattributed_count(), 3);
    assert_eq!(
        lanes.unattributed_count(),
        lanes.unattributed[0].nodes.len()
    );
}

#[test]
fn the_expanded_section_is_split_by_source_not_a_second_unattributed() {
    let lanes = split(&[event(None, "a commit")]);

    assert_eq!(lane_labels(&lanes.unattributed), ["ledger"]);
    assert_eq!(lane_ids(&lanes.unattributed), ["unattributed-ledger"]);
    assert!(lanes
        .unattributed
        .iter()
        .all(|lane| lane.label != "unattributed"));
}

#[test]
fn a_branch_head_is_labelled_by_its_branch_not_a_bare_sha() {
    let head = Event {
        source: Source::Git,
        ..Event::new(uid(), at(0.0), EventKind::BranchHead)
            .branch("fix/imp-7-thing")
            .commit("0123456789abcdef0123456789abcdef01234567")
    };

    let lanes = split(&[head]);
    assert_eq!(lane_labels(&lanes.unattributed), ["git"]);
    assert_eq!(lane_ids(&lanes.unattributed), ["unattributed-git"]);
    assert_eq!(
        lanes.unattributed[0].nodes[0].label,
        "branch fix/imp-7-thing @ 01234567"
    );
}

#[test]
fn a_branch_head_with_no_sha_ends_in_an_empty_sha() {
    let head = Event::new(uid(), at(0.0), EventKind::BranchHead).branch("x");

    assert_eq!(node(&head).label, "branch x @ ");
}

#[test]
fn a_short_sha_is_not_padded() {
    let head = Event::new(uid(), at(0.0), EventKind::BranchHead)
        .branch("x")
        .commit("abc");

    assert_eq!(node(&head).label, "branch x @ abc");
}

#[test]
fn a_branch_head_without_a_branch_reads_like_any_event() {
    let head = Event::new(uid(), at(0.0), EventKind::BranchHead).commit("0123456789abcdef");

    assert_eq!(node(&head).label, "0123456789ab");
}

// -- filters ------------------------------------------------------------------

fn filter_events() -> Vec<Event> {
    vec![
        on("imp-1", "worker", "worker one"),
        on("imp-2", "worker", "worker two"),
        on("imp-3", "omp", "omp one"),
    ]
}

#[test]
fn by_agent_narrows_the_lanes_to_that_agent() {
    let filters = Filters {
        agent: Some("omp".into()),
        ..Filters::default()
    };

    let lanes = filtered(&filter_events(), &filters);
    assert_eq!(lane_ids(&lanes.lanes), ["omp"]);
    assert_eq!(node_labels(&lanes.lanes[0]), ["imp-3 omp one"]);
}

#[test]
fn by_item_narrows_the_lanes_to_that_item() {
    let filters = Filters {
        item: Some("imp-2".into()),
        ..Filters::default()
    };

    let lanes = filtered(&filter_events(), &filters);
    assert_eq!(lane_ids(&lanes.lanes), ["worker"]);
    assert_eq!(node_labels(&lanes.lanes[0]), ["imp-2 worker two"]);
}

#[test]
fn by_kind_narrows_the_lanes_to_that_kind() {
    let mut events = filter_events();
    events.push(Event {
        kind: EventKind::Commit,
        ..on("imp-4", "omp", "a commit")
    });
    let filters = Filters {
        kind: Some("commit".into()),
        ..Filters::default()
    };

    let lanes = filtered(&events, &filters);
    assert_eq!(lane_ids(&lanes.lanes), ["omp"]);
    assert_eq!(lanes.lanes[0].nodes[0].kind, EventKind::Commit);

    let notes = Filters {
        kind: Some("note".into()),
        ..Filters::default()
    };
    assert_eq!(filtered(&events, &notes).lanes.len(), 2);
}

#[test]
fn an_unknown_agent_matches_nothing_rather_than_everything() {
    let filters = Filters {
        agent: Some("nobody".into()),
        ..Filters::default()
    };

    let lanes = filtered(&filter_events(), &filters);
    assert!(lanes.is_empty());
    assert_eq!(lanes, Split::default());
}

#[test]
fn no_events_is_an_empty_split() {
    assert!(split(&[] as &[Event]).is_empty());
    assert!(!split(&[event(None, "x")]).is_empty());
    assert!(!split(&[event(Some("a"), "x")]).is_empty());
}

// -- labels -------------------------------------------------------------------

#[test]
fn an_event_names_the_item_it_is_about_and_what_was_said() {
    let lanes = split(&[on("imp-7", "worker", "terminal elapsed reads 0.0s")]);

    assert_eq!(
        lanes.lanes[0].nodes[0].label,
        "imp-7 terminal elapsed reads 0.0s"
    );
}

#[test]
fn the_nodes_title_names_the_kind_the_agent_and_the_time() {
    let lanes = split(&[event(Some("worker"), "did a thing")]);

    assert_eq!(
        lanes.lanes[0].nodes[0].detail,
        "note · worker · 2026-10-01T00:00:00Z"
    );
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
fn the_nodes_detail_drops_the_parts_an_event_does_not_have(
    #[case] agent: Option<&str>,
    #[case] item: Option<&str>,
    #[case] branch: Option<&str>,
    #[case] detail: &str,
) {
    let event = Event {
        agent: agent.map(String::from),
        item_id: item.map(String::from),
        branch: branch.map(String::from),
        ..Event::new(uid(), at(0.0), EventKind::Note)
    };

    assert_eq!(node(&event).detail, detail);
}

#[test]
fn the_detail_names_the_kind_by_its_snake_case_name() {
    let event = Event::new(uid(), at(24.0), EventKind::PrMerged);

    assert_eq!(node(&event).detail, "pr_merged · 2026-10-02T00:00:00Z");
}

#[test]
fn a_long_note_is_one_line() {
    let long = "Imp.Predict.ReActV2.StepError: the model refused. ".repeat(6);

    let label = node(&event(Some("worker"), &long)).label;
    assert_eq!(label, long.trim());
    assert!(!label.contains('\n'));
}

#[test]
fn a_multi_line_summary_keeps_its_first_line_only_trimmed() {
    let text = "  first line  \nsecond line\nthird";

    let label = node(&on("imp-1", "worker", text)).label;
    assert_eq!(label, "imp-1 first line");
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
fn link_label_of_a_node(
    #[case] item: Option<&str>,
    #[case] commit: Option<&str>,
    #[case] pr: Option<u64>,
    #[case] link: &str,
) {
    let bare = Event {
        item_id: item.map(String::from),
        commit: commit.map(String::from),
        pr,
        ..Event::new(uid(), at(0.0), EventKind::Note)
    };
    assert_eq!(node(&bare).label, link);

    let said = Event {
        summary: Some("said".into()),
        ..bare
    };
    let wanted = if link.is_empty() {
        "said".to_string()
    } else {
        format!("{link} said")
    };
    assert_eq!(node(&said).label, wanted);
}

#[test]
fn an_empty_summary_adds_nothing_after_the_link() {
    let event = Event {
        summary: Some("\n second".into()),
        ..on("imp-1", "worker", "")
    };

    assert_eq!(node(&event).label, "imp-1");
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
fn slug_collapses_runs_of_disallowed_characters(#[case] text: &str, #[case] wanted: &str) {
    assert_eq!(slug(text), wanted);
}

#[test]
fn a_node_id_is_the_event_ids_slug() {
    let event = Event::new("ledger:12", at(0.0), EventKind::Note);

    assert_eq!(node(&event).id, "ledger-12");
}

// -- the filter row -----------------------------------------------------------

#[rstest]
#[case::empty("")]
#[case::blanks("   ")]
#[case::tab_and_newline("\t\n")]
fn a_blank_agent_is_unattributed_not_its_own_unlabelled_lane(#[case] blank: &str) {
    let events = [
        event(Some("worker"), "x"),
        event(Some(blank), "blank agent"),
    ];

    let lanes = split(&events);
    assert_eq!(lane_ids(&lanes.lanes), ["worker"]);
    assert!(lanes
        .lanes
        .iter()
        .all(|lane| !lane.label.is_empty() && !lane.id.is_empty()));
    assert_eq!(lanes.unattributed_count(), 1);
    assert_eq!(lane_labels(&lanes.unattributed), ["ledger"]);
}

// -- order --------------------------------------------------------------------

#[test]
fn unattributed_lanes_are_sorted_by_source_name() {
    let events = [
        from_source(Source::Run, None),
        from_source(Source::Ledger, None),
        from_source(Source::Github, None),
        from_source(Source::Git, None),
    ];

    let lanes = split(&events);
    assert_eq!(
        lane_labels(&lanes.unattributed),
        ["git", "github", "ledger", "run"]
    );
    assert_eq!(
        lane_ids(&lanes.unattributed),
        [
            "unattributed-git",
            "unattributed-github",
            "unattributed-ledger",
            "unattributed-run"
        ]
    );
    assert_eq!(lanes.unattributed_count(), 4);
}

#[test]
fn agent_lanes_are_sorted_by_name_bytewise() {
    let events = [
        event(Some("worker"), "x"),
        event(Some("Zed"), "x"),
        event(Some("omp"), "x"),
        event(Some("Alpha"), "x"),
    ];

    assert_eq!(
        lane_labels(&split(&events).lanes),
        ["Alpha", "Zed", "omp", "worker"]
    );
}

#[test]
fn agent_lane_ids_are_slugs_of_the_name() {
    let lanes = split(&[event(Some("claude imp/29"), "x")]);

    assert_eq!(lane_ids(&lanes.lanes), ["claude-imp-29"]);
    assert_eq!(lane_labels(&lanes.lanes), ["claude imp/29"]);
}

#[test]
fn nodes_keep_the_input_order() {
    let ids = ["e:9", "e:1", "e:5", "e:3"];
    let events: Vec<Event> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| Event {
            agent: Some("worker".into()),
            ..Event::new(*id, at(10.0 - i as f64), EventKind::Note)
        })
        .collect();

    let lanes = split(&events);
    let got: Vec<&str> = lanes.lanes[0].nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(got, ["e-9", "e-1", "e-5", "e-3"]);
}

// -- properties ---------------------------------------------------------------

fn arb_event() -> impl Strategy<Value = Event> {
    let text = || {
        prop::collection::vec(any::<char>(), 0..8).prop_map(|c| c.into_iter().collect::<String>())
    };
    (
        text(),
        prop::option::of(text()),
        prop::option::of(text()),
        0..Source::ALL.len(),
        0..EventKind::ALL.len(),
    )
        .prop_map(|(id, agent, item, source, kind)| Event {
            agent,
            item_id: item,
            source: Source::ALL[source],
            ..Event::new(id, at(0.0), EventKind::ALL[kind])
        })
}

fn is_dom_safe(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

proptest! {
    #[test]
    fn split_conserves_events_and_every_id_is_dom_safe(
        events in prop::collection::vec(arb_event(), 0..24),
    ) {
        let lanes = split(&events);
        let in_lanes: usize = lanes.lanes.iter().map(|lane| lane.nodes.len()).sum();

        prop_assert_eq!(in_lanes + lanes.unattributed_count(), events.len());
        for lane in lanes.lanes.iter().chain(&lanes.unattributed) {
            prop_assert!(is_dom_safe(&lane.id), "lane id {:?}", lane.id);
            prop_assert!(!lane.nodes.is_empty());
            for node in &lane.nodes {
                prop_assert!(is_dom_safe(&node.id), "node id {:?}", node.id);
            }
        }
        // A blank agent is never a lane.
        prop_assert!(lanes.lanes.iter().all(|lane| !lane.label.trim().is_empty()));
    }

    #[test]
    fn slug_is_idempotent_and_never_leaves_a_disallowed_character(
        chars in prop::collection::vec(any::<char>(), 0..16),
    ) {
        let text: String = chars.into_iter().collect();
        let once = slug(&text);

        prop_assert!(is_dom_safe(&once));
        prop_assert_eq!(slug(&once), once);
    }
}
