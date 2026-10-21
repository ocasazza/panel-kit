//! The LANES and COLLAB bodies of the swimlane `EventGraph`, against real SSR
//! output: the lanes split, `+N earlier`, the unattributed toggle, the pair
//! table and the threads.

use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit::widgets::event_graph::swimlane::EventGraph;
use panel_kit_core::widgets::event_graph::query::{GraphState, Mode};
use panel_kit_core::widgets::event_graph::time::{format_iso8601, format_step_time};
use panel_kit_core::widgets::event_graph::{Event, EventKind, Source};

// -- fixtures -----------------------------------------------------------------

/// The clock, epoch seconds.
const NOW: i64 = 1_790_000_000;
const DAY: i64 = 86_400;

/// Microseconds `ago` seconds before [`NOW`].
fn at(ago: i64) -> i64 {
    (NOW - ago) * 1_000_000
}

fn note(id: &str, ago: i64, agent: &str, item: &str) -> Event {
    Event::new(id, at(ago), EventKind::Note)
        .agent(agent)
        .item(item)
        .summary(format!("note {id}"))
}

/// What a graph renders, with the knobs the cases turn.
#[derive(Clone)]
struct Case {
    id: &'static str,
    mode: Mode,
    window: &'static str,
    events: Vec<Event>,
    selected: Option<String>,
    unattributed_open: bool,
}

impl PartialEq for Case {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

fn lanes(events: Vec<Event>) -> Case {
    Case {
        id: "activity",
        mode: Mode::Lanes,
        window: "24h",
        events,
        selected: None,
        unattributed_open: false,
    }
}

fn collab(events: Vec<Event>) -> Case {
    Case {
        mode: Mode::Collab,
        ..lanes(events)
    }
}

/// Handlers need a runtime, so the graph is built inside a component.
#[component]
fn Probe(case: Case) -> Element {
    let state = GraphState {
        mode: case.mode,
        window: case.window.to_string(),
        unattributed_open: case.unattributed_open,
        ..GraphState::default()
    };
    rsx! {
        EventGraph {
            id: case.id,
            state,
            events: Rc::new(case.events.clone()),
            now: NOW,
            selected: case.selected.clone(),
            on_action: |_| {},
            on_select: |_| {},
        }
    }
}

impl Case {
    fn render(self) -> Html {
        let mut dom = VirtualDom::new_with_props(Probe, ProbeProps { case: self });
        dom.rebuild_in_place();
        Html(dioxus_ssr::render(&dom))
    }
}

/// Rendered markup with the lookups the assertions share.
struct Html(String);

impl Html {
    fn has(&self, needle: &str) -> bool {
        self.0.contains(needle)
    }

    fn count(&self, needle: &str) -> usize {
        self.0.matches(needle).count()
    }

    fn has_id(&self, id: &str) -> bool {
        self.0.contains(&format!("id=\"{id}\""))
    }

    /// Byte range of the opening tag of the element with this id.
    fn open(&self, id: &str) -> (usize, usize) {
        let needle = format!("id=\"{id}\"");
        let hit = self
            .0
            .find(&needle)
            .unwrap_or_else(|| panic!("no element #{id}: {}", self.0));
        let start = self.0[..hit].rfind('<').unwrap();
        (start, hit + self.0[hit..].find('>').unwrap() + 1)
    }

    /// The opening tag of the element with this id.
    fn tag(&self, id: &str) -> &str {
        let (start, end) = self.open(id);
        &self.0[start..end]
    }

    /// The text of a leaf element, up to its first child or close tag.
    fn text(&self, id: &str) -> &str {
        let (_, end) = self.open(id);
        &self.0[end..end + self.0[end..].find('<').unwrap()]
    }

    /// Markup from the element's open tag to the next `<{name} id="{until}`
    /// sibling, or the end.
    fn span(&self, id: &str, until: &str) -> &str {
        let (start, end) = self.open(id);
        let stop = self.0[end..]
            .find(until)
            .map_or(self.0.len(), |at| end + at);
        &self.0[start..stop]
    }

    /// Only the collaboration section: the toolbar draws buttons of its own.
    fn body(&self) -> Html {
        let (start, _) = self.open("activity-collaboration");
        let end = self.0[start..].find("</section>").unwrap() + "</section>".len();
        Html(self.0[start..start + end].to_string())
    }

    fn position(&self, needle: &str) -> usize {
        self.0
            .find(needle)
            .unwrap_or_else(|| panic!("no {needle}: {}", self.0))
    }
}

// -- LANES --------------------------------------------------------------------

#[test]
fn one_lane_per_agent_sorted_by_name() {
    let html = lanes(vec![
        note("n1", 300, "zed", "imp-1"),
        note("n2", 200, "amy", "imp-1"),
        note("n3", 100, "amy", "imp-2"),
    ])
    .render();
    let list = html.tag("activity-lanes");
    assert!(list.contains("<ol "), "{list}");
    assert!(list.contains(r#"aria-label="lanes""#), "{list}");
    assert!(list.contains("pk-ag-lane-list"), "{list}");
    assert_eq!(html.count("data-lane="), 2);
    assert!(html.position("id=\"activity-lane-amy\"") < html.position("id=\"activity-lane-zed\""));
    assert!(html.tag("activity-lane-amy").contains(r#"data-lane="amy""#));
    assert!(html.tag("activity-lane-amy").contains("pk-ag-lane"));
}

#[test]
fn lane_head_holds_label_and_node_count() {
    let html = lanes(vec![
        note("n1", 300, "amy", "imp-1"),
        note("n2", 200, "amy", "imp-1"),
    ])
    .render();
    let lane = html.span("activity-lane-amy", "<ol class=\"pk-ag-nodes\"");
    assert!(
        lane.contains(r#"<span class="pk-ag-lane-label">amy</span>"#),
        "{lane}"
    );
    assert!(
        lane.contains(r#"<span class="pk-ag-count">2</span>"#),
        "{lane}"
    );
    assert_eq!(html.count("class=\"pk-ag-nodes\""), 1);
}

#[test]
fn blank_agent_is_not_its_own_lane() {
    let blank = Event::new("n2", at(100), EventKind::Note)
        .agent("")
        .summary("anonymous");
    let html = lanes(vec![note("n1", 200, "worker", "imp-1"), blank]).render();
    assert!(!html.has("id=\"activity-lane-\""), "{}", html.0);
    assert_eq!(html.count("data-lane="), 1);
    assert!(html.has_id("activity-lane-worker"));
    assert!(html.has_id("activity-unattributed"));
    assert_eq!(html.text("activity-unattributed-count"), "1");
}

#[test]
fn unattributed_is_collapsed_by_default_and_counts_events() {
    let events = (0..3)
        .map(|n| Event::new(format!("u{n}"), at(100 + n), EventKind::Note).summary("loose"))
        .chain([Event::new("g1", at(50), EventKind::Commit).source(Source::Git)])
        .collect();
    let html = lanes(events).render();
    let toggle = html.tag("activity-unattributed-toggle");
    assert!(toggle.contains(r#"aria-expanded="false""#), "{toggle}");
    assert!(
        toggle.contains(r#"aria-controls="activity-unattributed-lanes""#),
        "{toggle}"
    );
    assert!(toggle.contains(r#"type="button""#), "{toggle}");
    assert!(!toggle.contains("pk-ag-unattributed-toggle on"), "{toggle}");
    assert!(!html.tag("activity-unattributed").contains(" on\""));
    // Four events in two source lanes: the count is events, not lanes.
    assert_eq!(html.text("activity-unattributed-count"), "4");
    assert!(
        html.has(r#"<span class="pk-ag-caret" aria-hidden="true">▸</span>"#),
        "{}",
        html.0
    );
    assert!(html.has(r#"<span class="pk-ag-tl-label">unattributed</span>"#));
    assert!(!html.has_id("activity-unattributed-lanes"));
    assert!(!html.has_id("activity-lanes"), "no attributed lane exists");
    assert_eq!(html.count("pk-ag-node "), 0);
}

#[test]
fn open_unattributed_lists_source_named_lanes() {
    let events = vec![
        Event::new("u1", at(300), EventKind::Note).summary("one"),
        Event::new("u2", at(200), EventKind::Note).summary("two"),
        Event::new("g1", at(100), EventKind::Commit)
            .source(Source::Git)
            .summary("three"),
    ];
    let html = Case {
        unattributed_open: true,
        ..lanes(events)
    }
    .render();
    let toggle = html.tag("activity-unattributed-toggle");
    assert!(toggle.contains(r#"aria-expanded="true""#), "{toggle}");
    assert!(toggle.contains("pk-ag-unattributed-toggle on"), "{toggle}");
    assert!(html
        .tag("activity-unattributed")
        .contains("pk-ag-unattributed on"));
    assert!(html.has(r#"<span class="pk-ag-caret" aria-hidden="true">▾</span>"#));
    assert!(html
        .tag("activity-unattributed-lanes")
        .contains("pk-ag-lane-list"));
    // Labels are the sources, not "unattributed" twice.
    assert!(
        html.has(r#"<span class="pk-ag-lane-label">ledger</span>"#),
        "{}",
        html.0
    );
    assert!(html.has(r#"<span class="pk-ag-lane-label">git</span>"#));
    assert!(html.has_id("activity-lane-unattributed-ledger"));
    assert!(html.has_id("activity-lane-unattributed-git"));
    // The count equals the nodes drawn.
    assert_eq!(html.text("activity-unattributed-count"), "3");
    assert_eq!(html.count("class=\"pk-ag-node "), 3);
}

#[test]
fn no_unattributed_section_when_every_event_has_an_agent() {
    let html = lanes(vec![note("n1", 100, "amy", "imp-1")]).render();
    assert!(!html.has_id("activity-unattributed"));
    assert!(!html.has_id("activity-unattributed-toggle"));
}

#[test]
fn a_lane_draws_the_newest_hundred_and_counts_the_rest() {
    let events = (0..105)
        .map(|n| note(&format!("n{n}"), 80_000 - n * 60, "amy", "imp-1"))
        .collect();
    let html = lanes(events).render();
    let earlier = html.tag("activity-lane-amy-earlier");
    assert!(earlier.contains("pk-ag-earlier"), "{earlier}");
    assert_eq!(html.text("activity-lane-amy-earlier"), "+5 earlier");
    assert_eq!(html.count("class=\"pk-ag-node "), 100);
    assert!(!html.has_id("activity-node-n4"), "the oldest five collapse");
    assert!(html.has_id("activity-node-n5"));
    assert!(html.has_id("activity-node-n104"));
    // `+N earlier` leads the node list, and the head still counts every node.
    assert!(html.position("activity-lane-amy-earlier") < html.position("activity-node-n5"));
    assert!(html.has(r#"<span class="pk-ag-count">105</span>"#));
}

#[test]
fn a_lane_of_exactly_a_hundred_has_no_earlier_line() {
    let events = (0..100)
        .map(|n| note(&format!("n{n}"), 80_000 - n * 60, "amy", "imp-1"))
        .collect();
    let html = lanes(events).render();
    assert_eq!(html.count("class=\"pk-ag-node "), 100);
    assert!(!html.has_id("activity-lane-amy-earlier"));
}

#[test]
fn branch_head_reads_branch_and_short_sha() {
    let head = Event::new("b1", at(100), EventKind::BranchHead)
        .agent("amy")
        .branch("feat/x")
        .commit("0123456789abcdef");
    let html = lanes(vec![head]).render();
    assert!(
        html.has(r#"<span class="pk-ag-caption">branch feat/x @ 01234567</span>"#),
        "{}",
        html.0
    );
}

#[test]
fn a_long_note_is_one_caption_line() {
    let long = format!("first line of the note\nsecond line {}", "word ".repeat(60));
    let event = Event::new("n1", at(100), EventKind::Note)
        .agent("amy")
        .summary(long);
    let html = lanes(vec![event]).render();
    assert!(!html.has("<br"), "{}", html.0);
    assert_eq!(html.count("class=\"pk-ag-caption\""), 1);
    assert!(html.has(r#"<span class="pk-ag-caption">first line of the note</span>"#));
    assert!(!html.has("second line"));
}

#[test]
fn nodes_carry_kind_tone_and_detail_tooltip() {
    let events = vec![
        Event::new("a", at(500), EventKind::RunFailed).agent("amy"),
        Event::new("b", at(400), EventKind::PrMerged).agent("amy"),
        Event::new("c", at(300), EventKind::Handoff).agent("amy"),
        Event::new("d", at(200), EventKind::RunStarted).agent("amy"),
        Event::new("e", at(100), EventKind::Note)
            .agent("amy")
            .item("imp-1"),
    ];
    let html = lanes(events).render();
    for (id, tone, kind) in [
        ("a", "err", "run_failed"),
        ("b", "ok", "pr_merged"),
        ("c", "warn", "handoff"),
        ("d", "info", "run_started"),
        ("e", "neutral", "note"),
    ] {
        let tag = html.tag(&format!("activity-node-{id}"));
        assert!(tag.contains(&format!("pk-ag-node pk-ag-{tone}")), "{tag}");
        assert!(tag.contains(&format!(r#"data-kind="{kind}""#)), "{tag}");
        assert!(tag.contains("title=\""), "{tag}");
    }
    let tip = html.tag("activity-node-e");
    assert!(tip.contains("note · amy · imp-1"), "{tip}");
}

#[test]
fn an_empty_window_is_one_line() {
    let html = lanes(vec![]).render();
    assert_eq!(
        html.text("activity-lanes-empty"),
        "nothing recorded in this window"
    );
    assert!(html.tag("activity-lanes-empty").contains("pk-ag-empty"));
    assert!(!html.has_id("activity-lanes"));
    assert!(!html.has_id("activity-unattributed"));
    // An event older than the window is nothing recorded in it.
    let old = lanes(vec![note("n1", 3 * DAY, "amy", "imp-1")]).render();
    assert!(old.has_id("activity-lanes-empty"));
}

#[test]
fn every_id_follows_the_graph_id() {
    let html = Case {
        id: "board",
        unattributed_open: true,
        ..lanes(vec![
            note("n1", 100, "amy", "imp-1"),
            Event::new("u1", at(50), EventKind::Note).summary("loose"),
        ])
    }
    .render();
    assert!(html.has_id("board-lanes"));
    assert!(html.has_id("board-lane-amy"));
    assert!(html.has_id("board-node-n1"));
    assert!(html.has_id("board-unattributed-toggle"));
    assert!(html.has_id("board-unattributed-lanes"));
    assert!(!html.has("id=\"activity-lane"), "{}", html.0);
    assert!(!html.has("id=\"activity-unattributed"));
}

// -- COLLAB -------------------------------------------------------------------

/// alice and bob on imp-1: two notes against one, and a handoff from alice.
fn pair_events() -> Vec<Event> {
    vec![
        note("n1", 5000, "alice", "imp-1"),
        note("n2", 4000, "alice", "imp-1"),
        note("n3", 3000, "bob", "imp-1"),
        Event::new("h1", at(2000), EventKind::Handoff)
            .agent("bob")
            .item("imp-1")
            .from("alice")
            .title("Fix the thing")
            .summary("taking over"),
        Event::new("b1", at(1000), EventKind::Commit)
            .agent("bob")
            .item("imp-1")
            .branch("feat/imp-1"),
    ]
}

#[test]
fn pair_row_counts_notes_both_ways() {
    let html = collab(pair_events()).render();
    let table = html.tag("activity-collaboration-pairs");
    assert!(table.contains("<table "), "{table}");
    assert!(table.contains("pk-ag-pairs"), "{table}");
    assert!(html
        .tag("activity-collaboration-pair-alice-bob")
        .contains("<tr "));
    assert!(
        html.has(r#"<span class="pk-ag-tee">↔</span>"#),
        "{}",
        html.0
    );
    assert!(
        html.has(r#"<span class="pk-ag-a">2</span> → <span class="pk-ag-b">1</span>"#),
        "{}",
        html.0
    );
    assert_eq!(html.count(r#"<th scope="col">"#), 7);
    let headers = [
        "pair",
        "shared items",
        "notes",
        "reviews",
        "handoffs",
        "branches",
        "co-commits",
    ];
    let mut last = 0;
    for header in headers {
        let hit = html.position(&format!(">{header}</th>"));
        assert!(hit > last, "{header} out of order");
        last = hit;
    }
    assert!(html.has("feat/imp-1"), "branches column");
}

#[test]
fn a_handoff_names_both_agents_and_titles_its_evidence() {
    let events = pair_events();
    let when = format_iso8601(events[3].at);
    let html = collab(events).render();
    assert!(html.has("alice → bob</span>"), "{}", html.0);
    assert!(
        html.has(&format!(r#"title="alice -> bob on imp-1 at {when}""#)),
        "{}",
        html.0
    );
}

#[test]
fn no_handoffs_reads_a_faint_zero() {
    let html = collab(vec![
        note("n1", 300, "alice", "imp-1"),
        note("n2", 200, "bob", "imp-1"),
    ])
    .render();
    assert!(
        html.has(r#"<span class="pk-ag-faint">0</span>"#),
        "{}",
        html.0
    );
}

#[test]
fn shared_item_is_a_button_that_selects() {
    let html = collab(pair_events()).render().body();
    assert_eq!(
        html.count(r#"<button class="pk-ag-link""#),
        2,
        "pair cell and thread"
    );
    assert_eq!(html.count(r#"type="button""#), 2);
    assert_eq!(html.count(r#"aria-pressed="false""#), 2);
    assert!(!html.has("pk-ag-link on"));
    assert!(!html.has(r#"aria-pressed="true""#));
}

#[test]
fn the_selected_item_is_lit_in_pairs_and_threads() {
    let html = Case {
        selected: Some("imp-1".to_string()),
        ..collab(pair_events())
    }
    .render()
    .body();
    assert_eq!(
        html.count(r#"<button class="pk-ag-link on""#),
        2,
        "{}",
        html.0
    );
    assert_eq!(html.count(r#"aria-pressed="true""#), 2);
    assert!(html
        .tag("activity-collaboration-thread-imp-1")
        .contains("pk-ag-thread on"));
}

#[test]
fn a_selection_elsewhere_lights_nothing() {
    let html = Case {
        selected: Some("imp-9".to_string()),
        ..collab(pair_events())
    }
    .render()
    .body();
    assert!(!html.has("pk-ag-link on"));
    assert!(!html.has("pk-ag-thread on"));
    assert!(!html.has(r#"aria-pressed="true""#));
}

#[test]
fn only_items_with_two_agents_are_threads() {
    let mut events = pair_events();
    events.push(note("n9", 500, "alice", "imp-2"));
    let html = collab(events).render();
    let thread = html.tag("activity-collaboration-thread-imp-1");
    assert!(thread.contains("pk-ag-thread"), "{thread}");
    assert!(thread.contains(r#"title="Fix the thing""#), "{thread}");
    assert!(!html.has_id("activity-collaboration-thread-imp-2"));
    assert!(
        html.has(r#"<h3 class="pk-ag-threads-head">threads</h3>"#),
        "{}",
        html.0
    );
    assert!(html.tag("activity-collaboration-threads").contains("<ol "));
}

#[test]
fn thread_steps_read_time_agent_and_label() {
    let events = pair_events();
    let handoff_at = events[3].at;
    let html = collab(events).render();
    let steps = html.span("activity-collaboration-thread-imp-1", "</ol></li></ol>");
    assert!(steps.contains("pk-ag-steps"), "{steps}");
    assert!(
        steps.contains(&format!(">{}</time>", format_step_time(handoff_at))),
        "{steps}"
    );
    assert!(
        steps.contains(r#"<span class="pk-ag-step-agent">alice</span>"#),
        "{steps}"
    );
    assert!(
        steps.contains(r#"<span class="pk-ag-step-label">handed from alice</span>"#),
        "{steps}"
    );
    assert!(
        steps.contains(r#"<span class="pk-ag-step-label">note n1</span>"#),
        "{steps}"
    );
    // Commits are not steps.
    assert_eq!(steps.matches("<time ").count(), 4);
}

#[test]
fn a_stale_holder_marks_the_thread_and_says_since_when() {
    let held = Event::new("c1", at(4 * DAY + 60), EventKind::ItemClaimed)
        .agent("carol")
        .item("imp-5")
        .status("claimed")
        .title("Fix the thing");
    let status = Event::new("s1", at(4 * DAY), EventKind::ItemStatus)
        .agent("carol")
        .item("imp-5")
        .status("claimed");
    let since = format_iso8601(status.at);
    let html = Case {
        window: "14d",
        ..collab(vec![held, status, note("n1", DAY, "dave", "imp-5")])
    }
    .render();
    let thread = html.tag("activity-collaboration-thread-imp-5");
    assert!(thread.contains("pk-ag-thread pk-ag-stale"), "{thread}");
    assert!(
        html.has(&format!(
            r#"<span class="pk-ag-warn">⚠ stale holder carol since {since}</span>"#
        )),
        "{}",
        html.0
    );
}

#[test]
fn a_fresh_holder_is_not_stale() {
    let html = Case {
        window: "14d",
        ..collab(vec![
            Event::new("c1", at(DAY), EventKind::ItemClaimed)
                .agent("carol")
                .item("imp-5")
                .status("claimed"),
            Event::new("s1", at(DAY - 60), EventKind::ItemStatus)
                .agent("carol")
                .item("imp-5")
                .status("claimed"),
            note("n1", DAY / 2, "dave", "imp-5"),
        ])
    }
    .render();
    assert!(html.has_id("activity-collaboration-thread-imp-5"));
    assert!(!html.has("pk-ag-stale"));
    assert!(!html.has("stale holder"));
}

#[test]
fn two_items_with_one_agent_each_are_no_collaboration() {
    let html = collab(vec![
        note("n1", 300, "alice", "imp-1"),
        note("n2", 200, "bob", "imp-2"),
    ])
    .render();
    let section = html.tag("activity-collaboration");
    assert!(section.contains("pk-ag-collab"), "{section}");
    assert!(
        section.contains(r#"aria-label="collaboration""#),
        "{section}"
    );
    assert_eq!(
        html.text("activity-collaboration-empty"),
        "no two agents worked the same item"
    );
    assert!(html
        .tag("activity-collaboration-empty")
        .contains("pk-ag-empty"));
    assert!(!html.has_id("activity-collaboration-pairs"));
    assert!(!html.has_id("activity-collaboration-threads"));
    assert!(!html.has("pk-ag-threads-head"));
}

#[test]
fn collab_mode_draws_no_lanes() {
    let html = collab(pair_events()).render();
    assert!(html.has_id("activity-collaboration-pairs"));
    assert!(html.has_id("activity-collaboration-threads"));
    assert!(html.has_id("activity-collaboration-thread-imp-1"));
    assert!(!html.has_id("activity-lanes"));
    assert!(!html.has_id("activity-lanes-empty"));
    assert!(!html.has_id("activity-unattributed"));
}
