//! The LANES and COLLAB bodies of the swimlane `EventGraph`, against real SSR
//! output: the sequence diagram's lifelines, arrows, bars and day rules, its
//! lane menu and overflow, and the pair and thread tables, each bounded
//! however many events there are.

use std::collections::BTreeSet;
use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit::widgets::event_graph::swimlane::EventGraph;
use panel_kit_core::widgets::data_table::WINDOW_COUNT;
use panel_kit_core::widgets::event_graph::query::{GraphState, LanePick, Mode};
use panel_kit_core::widgets::event_graph::swimlane::{Node, BUSIEST_LANES, LANE_MIN_W};
use panel_kit_core::widgets::event_graph::{Event, EventKind, Source};

// -- fixtures -----------------------------------------------------------------

/// The clock, epoch seconds.
const NOW: i64 = 1_790_000_000;
const CSS: &str = include_str!("../../assets/panel-kit.css");

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
    mode: Mode,
    window: &'static str,
    events: Vec<Event>,
    open: Option<Node>,
    lanes: LanePick,
    lane: Option<String>,
    lane_filter: String,
}

impl PartialEq for Case {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

fn lanes(events: Vec<Event>) -> Case {
    Case {
        mode: Mode::Lanes,
        window: "24h",
        events,
        open: None,
        lanes: LanePick::Auto,
        lane: None,
        lane_filter: String::new(),
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
        lanes: case.lanes.clone(),
        lane: case.lane.clone(),
        lane_filter: case.lane_filter.clone(),
        ..GraphState::default()
    };
    rsx! {
        EventGraph {
            id: "activity",
            state,
            events: Rc::new(case.events.clone()),
            now: NOW,
            open: case.open.clone(),
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

    /// Every opening tag that starts with `prefix`.
    fn tags(&self, prefix: &str) -> Vec<&str> {
        self.0
            .match_indices(prefix)
            .map(|(at, _)| &self.0[at..at + self.0[at..].find('>').unwrap() + 1])
            .collect()
    }

    fn position(&self, needle: &str) -> usize {
        self.0
            .find(needle)
            .unwrap_or_else(|| panic!("no {needle}: {}", self.0))
    }
}

/// The value of attribute `name` in an opening `tag`.
fn attr<'a>(tag: &'a str, name: &str) -> &'a str {
    let key = format!(" {name}=\"");
    let start = tag
        .find(&key)
        .unwrap_or_else(|| panic!("no {name} in {tag}"))
        + key.len();
    &tag[start..start + tag[start..].find('"').unwrap()]
}

fn num(tag: &str, name: &str) -> f64 {
    attr(tag, name).parse().unwrap()
}

fn handoff(id: &str, ago: i64, to: &str, from: &str, item: &str) -> Event {
    Event {
        kind: EventKind::Handoff,
        ..note(id, ago, to, item).from(from)
    }
}

/// The text of the CSS rule whose selector starts with `selector`.
fn rule(selector: &str) -> &'static str {
    CSS.split(selector)
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .unwrap_or_else(|| panic!("no rule for {selector}"))
}

/// `n` agents (even), each pair meeting once on its own item, so every agent
/// has an interaction.
fn pairs_of(n: usize) -> Vec<Event> {
    (0..n)
        .map(|i| {
            note(
                &format!("p{i}"),
                10_000 - i as i64,
                &format!("agent-{i:03}"),
                &format!("imp-{}", i / 2),
            )
        })
        .collect()
}

/// The lane header buttons of a rendering, in order.
fn heads(html: &Html) -> Vec<&str> {
    html.tags("<button id=\"activity-seq-lane-")
}

// -- LANES: the sequence diagram ------------------------------------------------

#[test]
fn a_handoff_is_an_arrow_from_the_giver_to_the_taker() {
    let html = lanes(vec![
        note("n1", 300, "amy", "imp-1"),
        handoff("h1", 200, "bob", "amy", "imp-1"),
    ])
    .render();

    let arrows = html.tags("<line class=\"pk-ag-seq-arrow\"");
    assert_eq!(arrows.len(), 1, "{}", html.0);
    let arrow = arrows[0];
    assert_eq!(attr(arrow, "data-from"), "amy");
    assert_eq!(attr(arrow, "data-to"), "bob");
    assert_eq!(attr(arrow, "data-edge"), "handoff");
    assert_eq!(attr(arrow, "data-dir"), "right");
    assert!(
        num(arrow, "x1") < num(arrow, "x2"),
        "the line runs from the giver's lifeline to the taker's: {arrow}"
    );
}

#[test]
fn an_arrow_back_points_left_and_a_baton_pass_says_so() {
    let html = lanes(vec![
        note("n1", 400, "amy", "imp-1"),
        note("n2", 300, "bob", "imp-1"),
        note("n3", 200, "amy", "imp-1"),
    ])
    .render();

    let arrows = html.tags("<line class=\"pk-ag-seq-arrow\"");
    assert_eq!(arrows.len(), 2, "{}", html.0);
    assert_eq!(attr(arrows[0], "data-dir"), "right");
    assert_eq!(attr(arrows[1], "data-dir"), "left");
    assert_eq!(attr(arrows[1], "data-from"), "bob");
    assert_eq!(attr(arrows[1], "data-to"), "amy");
    assert_eq!(attr(arrows[1], "data-edge"), "baton");
    assert!(num(arrows[1], "x1") > num(arrows[1], "x2"), "{}", arrows[1]);
}

#[test]
fn a_review_arrow_runs_from_the_reviewer_to_the_holder() {
    let review = Event {
        kind: EventKind::PrReviewed,
        ..note("r1", 200, "bob", "imp-1")
    };
    let html = lanes(vec![note("n1", 300, "amy", "imp-1"), review]).render();

    let arrow = html.tags("<line class=\"pk-ag-seq-arrow\"")[0];
    assert_eq!(attr(arrow, "data-edge"), "review");
    assert_eq!(attr(arrow, "data-from"), "bob");
    assert_eq!(attr(arrow, "data-to"), "amy");
    assert_eq!(attr(arrow, "data-dir"), "left");
}

#[test]
fn every_arrow_ends_in_a_shared_head_of_its_tone_and_starts_on_an_anchor() {
    let status = Event {
        kind: EventKind::ItemStatus,
        ..note("s1", 150, "amy", "imp-1").status("done")
    };
    let html = lanes(vec![
        note("n1", 400, "amy", "imp-1"),
        note("n2", 300, "bob", "imp-1"),
        handoff("h1", 200, "cat", "bob", "imp-1"),
        status,
    ])
    .render();

    let arrows = html.tags("<line class=\"pk-ag-seq-arrow\"");
    assert!(arrows.len() >= 3, "{}", html.0);
    // One marker per tone, defined once however many arrows use them.
    assert_eq!(html.count("<marker "), 5, "{}", html.0);
    for tone in ["info", "ok", "warn", "err", "neutral"] {
        let id = format!("activity-seq-head-{tone}");
        assert_eq!(html.count(&format!("id=\"{id}\"")), 1, "{tone}");
    }
    for arrow in &arrows {
        let end = attr(arrow, "marker-end");
        assert!(end.starts_with("url(#activity-seq-head-"), "{arrow}");
        assert!(
            html.has(&format!("id=\"{}\"", &end[5..end.len() - 1])),
            "{end}"
        );
    }
    let anchors = html.tags("<circle class=\"pk-ag-seq-anchor\"");
    assert_eq!(anchors.len(), arrows.len(), "one source anchor per arrow");
    for (anchor, arrow) in anchors.iter().zip(&arrows) {
        assert_eq!(
            attr(anchor, "cx"),
            attr(arrow, "x1"),
            "the dot sits on the origin lifeline"
        );
        assert_eq!(attr(anchor, "r"), "2");
    }
    let head = html.tags("<marker id=\"activity-seq-head-info\"")[0];
    assert_eq!(attr(head, "markerWidth"), "9");
    assert_eq!(attr(head, "markerHeight"), "10");
    assert_eq!(attr(head, "orient"), "auto");
}

#[test]
fn an_arrow_is_in_the_tone_of_its_event_and_has_a_wide_hit_area() {
    let html = lanes(vec![
        note("n1", 300, "amy", "imp-1"),
        handoff("h1", 200, "bob", "amy", "imp-1"),
    ])
    .render();

    let group = html.tags("<g id=\"activity-seq-h1\"")[0];
    assert!(
        attr(group, "class").starts_with("pk-ag-seq-edge pk-ag-"),
        "{group}"
    );
    let arrow = html.tags("<line class=\"pk-ag-seq-arrow\"")[0];
    let tone = attr(group, "class")
        .split(' ')
        .nth(1)
        .unwrap()
        .trim_start_matches("pk-ag-");
    assert!(
        attr(arrow, "marker-end").ends_with(&format!("-{tone})")),
        "{arrow}"
    );
    let at = html.position("id=\"activity-seq-h1\"");
    let hit = html.0[at..]
        .find("<rect class=\"pk-ag-seq-hit\"")
        .map(|rect| {
            let start = at + rect;
            &html.0[start..start + html.0[start..].find('>').unwrap() + 1]
        });
    assert_eq!(
        num(hit.unwrap(), "height"),
        12.0,
        "a thin line, a generous target"
    );
}

#[test]
fn only_the_open_arrow_carries_the_selected_class() {
    let html = Case {
        open: Some(Node::Event("n3".to_string())),
        ..lanes(vec![
            note("n1", 400, "amy", "imp-1"),
            note("n2", 300, "bob", "imp-1"),
            note("n3", 200, "amy", "imp-1"),
        ])
    }
    .render();

    let selected: Vec<&str> = html
        .tags("<g id=\"activity-seq-")
        .into_iter()
        .filter(|tag| attr(tag, "class").split(' ').any(|c| c == "on"))
        .collect();
    assert_eq!(selected.len(), 1, "{}", html.0);
    assert_eq!(attr(selected[0], "data-event"), "n3");
    assert_eq!(attr(selected[0], "aria-pressed"), "true");
    let others = html
        .tags("<g id=\"activity-seq-")
        .into_iter()
        .filter(|tag| attr(tag, "aria-pressed") == "false")
        .count();
    assert_eq!(others, 2);
}

#[test]
fn the_stylesheet_animates_only_the_hovered_or_selected_arrow_and_respects_reduced_motion() {
    let base = rule(".pk-ag-seq-arrow {");
    assert!(
        !base.contains("animation"),
        "a resting arrow is static: {base}"
    );
    assert!(base.contains("stroke-width: 1.5"), "{base}");
    let flow = CSS
        .match_indices("animation: pk-ag-seq-flow")
        .map(|(at, _)| &CSS[CSS[..at].rfind('}').unwrap_or(0)..at])
        .collect::<Vec<_>>();
    assert!(!flow.is_empty(), "the flow animation exists");
    for before in &flow {
        assert!(
            before.contains(":hover") && before.contains(".on"),
            "the flow runs on hover and selection only: {before}"
        );
    }
    assert!(CSS.contains("@keyframes pk-ag-seq-flow"));
    let motion = CSS
        .split("@media (prefers-reduced-motion: reduce)")
        .skip(1)
        .map(|chunk| chunk.split("\n}\n").next().unwrap_or(""))
        .find(|block| block.contains(".pk-ag-seq-edge"))
        .expect("a reduced-motion block for the arrows");
    assert!(motion.contains("animation: none"), "{motion}");
}

#[test]
fn a_global_action_is_a_bar_across_every_lifeline_captioned_at_the_first() {
    let commit = Event::new("c1", at(250), EventKind::Commit)
        .agent("amy")
        .commit("abcdef0123456789")
        .source(Source::Git);
    let html = lanes(vec![
        note("n1", 300, "amy", "imp-1"),
        note("n2", 200, "bob", "imp-1"),
        commit,
    ])
    .render();

    let group = html.tags("<g id=\"activity-seq-c1\"")[0];
    assert!(
        attr(group, "class").contains("pk-ag-seq-broadcast"),
        "{group}"
    );
    let bar = html.tags("<line class=\"pk-ag-seq-bar\"")[0];
    let lifelines = html.tags("<line class=\"pk-ag-seq-life\"");
    assert_eq!(lifelines.len(), 2);
    let (from, to) = (num(bar, "x1"), num(bar, "x2"));
    for life in lifelines {
        let x = num(life, "x1");
        assert!(from < x && x < to, "{bar} spans {life}");
    }
    assert!(html.has("commit abcdef012345"), "{}", html.0);
}

#[test]
fn lifelines_stand_in_order_of_first_interaction_under_sticky_headers() {
    let html = lanes(vec![
        note("n1", 400, "cat", "imp-2"),
        note("n2", 390, "dan", "imp-2"),
        note("n3", 300, "amy", "imp-1"),
        note("n4", 290, "bob", "imp-1"),
    ])
    .render();

    let chips = heads(&html);
    let names: Vec<&str> = chips.iter().map(|tag| attr(tag, "data-lane")).collect();
    assert_eq!(names, ["cat", "dan", "amy", "bob"]);
    for tag in &chips {
        assert_eq!(attr(tag, "aria-pressed"), "false");
        assert!(attr(tag, "title").contains("interactions"), "{tag}");
        assert!(
            num_style(tag) >= LANE_MIN_W,
            "a lane is never narrower than the minimum: {tag}"
        );
    }
    assert_eq!(html.count("class=\"pk-ag-seq-life\""), 4);
    assert!(
        html.position("pk-ag-seq-head") < html.position("pk-ag-seq-body"),
        "the header precedes the rows"
    );
    let header = rule(".pk-ag-seq-head {");
    assert!(
        header.contains("position: sticky") && header.contains("top: 0"),
        "{header}"
    );
    for sticky in [".pk-ag-seq-corner {", ".pk-ag-seq-gutter {"] {
        let block = rule(sticky);
        assert!(
            block.contains("position: sticky") && block.contains("left: 0"),
            "{block}"
        );
    }
    assert!(rule(".pk-ag-seq-scroll {").contains("overflow: auto"));
}

/// `width: Npx` of an inline style.
fn num_style(tag: &str) -> f64 {
    let style = attr(tag, "style");
    style
        .strip_prefix("width: ")
        .and_then(|rest| rest.strip_suffix("px"))
        .unwrap_or_else(|| panic!("no width in {style}"))
        .parse()
        .unwrap()
}

#[test]
fn a_row_carries_a_stamp_in_the_gutter_and_a_day_rule_precedes_each_day() {
    let html = lanes(vec![
        note("n1", 80_000, "amy", "imp-1"),
        note("n2", 79_000, "bob", "imp-1"),
        note("n3", 20_000, "amy", "imp-1"),
    ])
    .render();

    let days = html.tags("<g class=\"pk-ag-seq-day\"");
    assert!(days.len() >= 2, "{}", html.0);
    for day in &days {
        let label = attr(day, "data-day");
        assert_eq!(label.len(), 5);
        assert_eq!(&label[2..3], "-");
    }
    let gutter = html.position("class=\"pk-ag-seq-gutter\"");
    let lanes_at = html.position("class=\"pk-ag-svg pk-ag-seq-svg\"");
    let stamp = html.position("class=\"pk-ag-seq-stamp\"");
    assert!(
        gutter < stamp && stamp < lanes_at,
        "stamps live in the sticky gutter"
    );
    assert_eq!(html.count("class=\"pk-ag-seq-stamp\""), 3);
    assert_eq!(html.count("class=\"pk-ag-seq-daylabel\""), days.len());
    let text = &html.0[stamp..];
    let text = &text[text.find('>').unwrap() + 1..];
    let text = &text[..text.find('<').unwrap()];
    assert!(
        text.len() == 5 && &text[2..3] == ":",
        "a HH:MM stamp, not {text}"
    );
}

#[test]
fn rows_open_their_event_in_the_inspector_and_say_which() {
    let html = Case {
        open: Some(Node::Event("h1".to_string())),
        ..lanes(vec![
            note("n1", 300, "amy", "imp-1"),
            handoff("h1", 200, "bob", "amy", "imp-1"),
        ])
    }
    .render();

    let row = html.tags("<g id=\"activity-seq-h1\"")[0];
    assert_eq!(attr(row, "role"), "button");
    assert_eq!(attr(row, "tabindex"), "0");
    assert_eq!(attr(row, "data-event"), "h1");
    assert_eq!(attr(row, "aria-pressed"), "true", "the open event is lit");
    assert!(html.has_id("activity-inspector"));
}

#[test]
fn plain_activity_is_a_quiet_toggle_on_for_a_few_events_and_off_for_many() {
    let few = lanes(vec![
        note("n1", 300, "amy", "imp-1"),
        note("n2", 200, "amy", "imp-1"),
    ])
    .render();
    let chip = few.tag("activity-activity");
    assert_eq!(attr(chip, "aria-pressed"), "true");
    assert_eq!(few.count("<circle class=\"pk-ag-dot"), 2, "{}", few.0);

    let many: Vec<Event> = (0..5_001)
        .map(|k| note(&format!("m{k}"), 80_000 - k as i64, "amy", "imp-1"))
        .collect();
    let many = lanes(many).render();
    assert_eq!(attr(many.tag("activity-activity"), "aria-pressed"), "false");
    assert_eq!(many.count("<circle class=\"pk-ag-dot"), 0);
    assert!(many.has("5001"), "the chip counts what it hides");
}

#[test]
fn a_hundred_thousand_events_mount_a_bounded_window_of_rows() {
    let events = crowd(100_000, 40);
    let html = lanes(events).render();

    let rows: usize = attr(html.tag("activity-sequence"), "data-rows")
        .parse()
        .unwrap();
    assert!(rows > 10_000, "the diagram is as tall as its rows: {rows}");
    let mounted = html.count("class=\"pk-ag-seq-edge") + html.count("class=\"pk-ag-seq-day\"");
    assert!(mounted <= 60, "{mounted} rows mounted");
    assert!(html.0.len() < 250_000, "{} bytes of markup", html.0.len());
    assert_eq!(
        heads(&html).len(),
        BUSIEST_LANES,
        "forty agents: the busiest eight"
    );
    assert_eq!(html.count("class=\"pk-ag-seq-lane pk-ag-seq-fold\""), 1);
    let end: usize = attr(html.tag("activity-sequence"), "data-end")
        .parse()
        .unwrap();
    assert!(end <= 64, "the window ends at row {end}");
}

#[test]
fn an_empty_window_and_a_window_with_no_meeting_are_one_line() {
    let none = lanes(vec![]).render();
    assert!(none.has_id("activity-sequence-empty"), "{}", none.0);
    assert_eq!(none.count("class=\"pk-ag-seq-life\""), 0);

    let strangers = lanes(vec![
        note("n1", 100, "amy", "imp-1"),
        note("n2", 90, "bob", "imp-2"),
    ]);
    let quiet = strangers.render();
    assert_eq!(quiet.count("<line class=\"pk-ag-seq-arrow\""), 0);
}

#[test]
fn no_flat_lane_list_or_unattributed_toggle_remains() {
    let html = lanes(vec![
        note("n1", 300, "amy", "imp-1"),
        Event::new("u1", at(250), EventKind::Note).summary("anonymous"),
    ])
    .render();

    for gone in [
        "pk-ag-lane-list",
        "pk-ag-unattributed",
        "pk-ag-node-hit",
        "pk-ag-earlier",
    ] {
        assert!(!html.has(gone), "{gone} in {}", html.0);
    }
    assert!(html.has("1 with no agent not drawn"), "{}", html.0);
}

// -- LANES: overflow, the lane menu, the fold ---------------------------------------

#[test]
fn above_the_cap_the_busiest_eight_stand_and_the_rest_fold_into_one_narrow_lane() {
    let html = lanes(pairs_of(40)).render();

    assert_eq!(heads(&html).len(), BUSIEST_LANES);
    let fold = html.tags("<div id=\"activity-seq-lane-8\"");
    assert_eq!(fold.len(), 1, "{}", html.0);
    assert!(attr(fold[0], "class").contains("pk-ag-seq-fold"));
    assert_eq!(attr(fold[0], "data-lane"), "+32");
    assert_eq!(num_style(fold[0]), 56.0, "narrow");
    let members = attr(fold[0], "title");
    assert!(
        members.contains("agent-"),
        "the title lists them: {members}"
    );
    assert_eq!(html.count("class=\"pk-ag-seq-life\""), BUSIEST_LANES + 1);
    let fold_chip = &html.0[html.position("id=\"activity-seq-lane-8\"")..];
    let fold_chip = &fold_chip[..fold_chip.find("</div>").unwrap()];
    assert!(
        !fold_chip.contains("pk-ag-count"),
        "the count goes before the name: {fold_chip}"
    );
    let first_chip = &html.0[html.position("id=\"activity-seq-lane-0\"")..];
    let first_chip = &first_chip[..first_chip.find("</button>").unwrap()];
    assert!(first_chip.contains("pk-ag-count"), "{first_chip}");
}

#[test]
fn the_body_scrolls_sideways_when_the_minimum_columns_do_not_fit() {
    let crowded = lanes(pairs_of(40)).render();
    assert_eq!(
        attr(crowded.tag("activity-sequence"), "data-scrolls"),
        "true"
    );

    let roomy = lanes(vec![
        note("n1", 300, "amy", "imp-1"),
        note("n2", 200, "bob", "imp-1"),
    ])
    .render();
    assert_eq!(
        attr(roomy.tag("activity-sequence"), "data-scrolls"),
        "false"
    );
}

#[test]
fn the_menu_lists_every_lane_with_its_count_and_checks_the_ones_drawn() {
    let html = lanes(pairs_of(40)).render();

    assert_eq!(
        attr(html.tag("activity-lanes-menu"), "class"),
        "pk-dt-menu pk-ag-lanes-menu"
    );
    assert!(html.has("lanes 40 \u{25be}"), "{}", html.0);
    let rows = html.tags("<div class=\"pk-dt-menu-row pk-ag-lane-row");
    assert_eq!(rows.len(), 40, "every lane, drawn or not");
    let checked = html.0.matches("aria-label=\"draw agent-").count();
    assert_eq!(checked, 40);
    let drawn = html
        .tags("<input type=\"checkbox\"")
        .into_iter()
        .filter(|tag| tag.contains("checked=true"))
        .count();
    assert_eq!(drawn, BUSIEST_LANES, "only the drawn lanes are checked");
    for control in ["busiest 8", ">all<", ">none<"] {
        assert!(html.has(control), "{control}");
    }
    assert!(html.has_id("activity-lanes-filter"));
    // A count beside each name: a pair's first agent has a dot and an arrow
    // end, its second the other arrow end, three interactions a pair.
    assert_eq!(html.count("class=\"pk-ag-lane-name\""), 40);
    let mut total = 0;
    for row in html.0.split("pk-dt-menu-row pk-ag-lane-row").skip(1) {
        let row = &row[..row.find("</div>").unwrap()];
        let at = row.find("<span class=\"pk-ag-count\">").expect(row) + 26;
        let count: usize = row[at..row[at..].find('<').map(|end| at + end).unwrap()]
            .parse()
            .unwrap();
        assert!((1..=2).contains(&count), "{row}");
        total += count;
    }
    assert_eq!(total, 60);
}

#[test]
fn unchecked_lanes_are_absent_from_the_header_and_the_state_decides_who_is_drawn() {
    let only: BTreeSet<String> = ["agent-001", "agent-005"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let html = Case {
        lanes: LanePick::Only(only),
        ..lanes(pairs_of(40))
    }
    .render();

    let names: Vec<&str> = heads(&html)
        .iter()
        .map(|tag| attr(tag, "data-lane"))
        .collect();
    assert_eq!(names, ["agent-001", "agent-005"]);
    assert!(html.has("data-lane=\"+38\""), "{}", html.0);
    assert!(!html.0.contains("<button id=\"activity-seq-lane-2\""));
    assert_eq!(
        html.count("pk-ag-lane-row"),
        40,
        "the menu still lists them all"
    );

    let all = Case {
        lanes: LanePick::All,
        ..lanes(pairs_of(40))
    }
    .render();
    assert_eq!(heads(&all).len(), 40);
    assert!(!all.has("pk-ag-seq-fold"));

    let none = Case {
        lanes: LanePick::Only(BTreeSet::new()),
        ..lanes(pairs_of(40))
    }
    .render();
    assert_eq!(heads(&none).len(), 0);
    assert!(none.has("data-lane=\"+40\""));
}

#[test]
fn the_lane_the_diagram_is_filtered_to_is_framed_in_the_header_and_the_menu() {
    let html = Case {
        lane: Some("agent-002".to_string()),
        ..lanes(pairs_of(8))
    }
    .render();

    let framed: Vec<&str> = heads(&html)
        .into_iter()
        .filter(|tag| attr(tag, "aria-pressed") == "true")
        .collect();
    assert_eq!(framed.len(), 1);
    assert_eq!(attr(framed[0], "data-lane"), "agent-002");
    assert!(attr(framed[0], "class").ends_with(" on"), "{}", framed[0]);
    let rows: Vec<&str> = html
        .tags("<div class=\"pk-dt-menu-row pk-ag-lane-row")
        .into_iter()
        .filter(|tag| attr(tag, "class").ends_with(" on"))
        .collect();
    assert_eq!(rows.len(), 1, "the menu frames the same lane");
    assert_eq!(attr(rows[0], "data-lane"), "agent-002");
    assert_eq!(
        attr(html.tag("activity-sequence"), "data-focus"),
        "agent-002"
    );
    // Only arrows that touch the lane are drawn.
    for arrow in html.tags("<line class=\"pk-ag-seq-arrow\"") {
        assert!(
            attr(arrow, "data-from") == "agent-002" || attr(arrow, "data-to") == "agent-002",
            "{arrow}"
        );
    }
}

#[test]
fn a_focused_lane_that_is_folded_frames_nothing() {
    let only: BTreeSet<String> = ["agent-000", "agent-001"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let html = Case {
        lanes: LanePick::Only(only),
        lane: Some("agent-006".to_string()),
        ..lanes(pairs_of(8))
    }
    .render();

    let pressed = heads(&html)
        .into_iter()
        .filter(|tag| attr(tag, "aria-pressed") == "true")
        .count();
    assert_eq!(pressed, 0, "{}", html.0);
    assert!(!html.has("pk-ag-lane-row on"));
    assert!(!html.has("data-focus="));
}

#[test]
fn the_menu_filter_narrows_the_list_and_the_list_is_bounded() {
    let filtered = Case {
        lane_filter: "agent-01".to_string(),
        ..lanes(pairs_of(40))
    }
    .render();
    assert_eq!(filtered.count("class=\"pk-ag-lane-name\""), 10);
    assert!(filtered.has("value=\"agent-01\""));

    let many = lanes(pairs_of(300)).render();
    assert_eq!(
        many.count("class=\"pk-ag-lane-name\""),
        100,
        "a hundred rows at most"
    );
    assert!(many.has("+200 more: filter to find them"), "{}", many.0);
}

// -- COLLAB: the tables -----------------------------------------------------------

/// amy and bob share three items, bob and cat two, amy and cat one; every
/// item has a note from each of its agents.
fn trio() -> Vec<Event> {
    let mut events = Vec::new();
    let mut n = 0;
    let mut item = |agents: &[&str], name: &str| {
        for agent in agents {
            n += 1;
            events.push(note(&format!("t{n}"), 10_000 + n, agent, name));
        }
    };
    item(&["amy", "bob"], "imp-1");
    item(&["amy", "bob"], "imp-2");
    item(&["amy", "bob"], "imp-3");
    item(&["bob", "cat"], "imp-4");
    item(&["bob", "cat"], "imp-5");
    item(&["amy", "cat"], "imp-6");
    events
}

#[test]
fn pairs_are_one_table_sorted_by_shared_items_descending() {
    let html = collab(trio()).render();

    assert!(html.has_id("activity-pairs"), "{}", html.0);
    assert!(html.has_id("activity-pairs-grid"));
    assert!(html.has("<thead>"), "a header the scroller keeps");
    let amy_bob = html
        .position("amy</b><span class=\"pk-ag-tee\"> \u{2194} </span><b class=\"pk-ag-id\">bob");
    let bob_cat = html
        .position("bob</b><span class=\"pk-ag-tee\"> \u{2194} </span><b class=\"pk-ag-id\">cat");
    let amy_cat = html
        .position("amy</b><span class=\"pk-ag-tee\"> \u{2194} </span><b class=\"pk-ag-id\">cat");
    assert!(
        amy_bob < bob_cat && bob_cat < amy_cat,
        "3, then 2, then 1 shared"
    );
    let sorted = html
        .tags("<th ")
        .into_iter()
        .find(|tag| tag.contains("data-col=\"shared\""));
    assert_eq!(attr(sorted.unwrap(), "aria-sort"), "descending");
}

#[test]
fn shared_items_are_a_count_and_a_few_names_with_the_whole_list_in_the_title() {
    let mut events = Vec::new();
    for n in 1..=5 {
        events.push(note(
            &format!("a{n}"),
            9_000 + n,
            "amy",
            &format!("imp-{n}"),
        ));
        events.push(note(
            &format!("b{n}"),
            8_000 + n,
            "bob",
            &format!("imp-{n}"),
        ));
    }
    let html = collab(events).render();

    let cell = html
        .tags("<td ")
        .into_iter()
        .find(|tag| tag.contains("title=\"imp-1, imp-2, imp-3, imp-4, imp-5\""))
        .unwrap_or_else(|| panic!("no items cell: {}", html.0));
    assert!(cell.contains("pk-dt-td"), "{cell}");
    assert!(
        html.has("imp-1, imp-2, imp-3<span class=\"pk-ag-faint pk-ag-tail\"> +2</span>"),
        "{}",
        html.0
    );
    assert!(
        html.has("<td role=\"gridcell\""),
        "numbers sit in cells of their own"
    );
    assert!(!html.has("pk-ag-link"), "no chip per shared item");
    assert!(!html.has("pk-ag-pair-items"));
}

#[test]
fn a_pair_row_names_its_notes_each_way_and_its_counts() {
    let html = collab(trio()).render();

    assert!(html.has("<span class=\"pk-ag-a\">1</span><span class=\"pk-ag-faint\"> \u{2192} </span><span class=\"pk-ag-b\">1</span>"), "{}", html.0);
    for column in [
        "pair",
        "shared",
        "items",
        "notes",
        "reviews",
        "handoffs",
        "branches",
        "names",
        "co_commits",
    ] {
        assert!(html.has(&format!("data-col=\"{column}\"")), "{column}");
    }
}

#[test]
fn the_open_pair_is_the_selected_row_and_rows_carry_their_index() {
    let html = Case {
        open: Some(Node::Pair("amy".to_string(), "bob".to_string())),
        ..collab(trio())
    }
    .render();

    let rows = html.count("data-id=\"pair-");
    assert!(rows >= 3, "{}", html.0);
    let selected: Vec<&str> = html
        .tags("<tr ")
        .into_iter()
        .filter(|tag| tag.contains("aria-selected=\"true\""))
        .collect();
    assert_eq!(selected.len(), 1, "{}", html.0);
    assert!(html.has_id("activity-inspector"));
}

#[test]
fn threads_are_a_table_with_a_stale_holder_cell() {
    let events = vec![
        Event::new("c1", at(7 * 86_400 + 60), EventKind::ItemClaimed)
            .agent("amy")
            .item("imp-9"),
        Event::new("s1", at(7 * 86_400), EventKind::ItemStatus)
            .agent("amy")
            .item("imp-9")
            .status("claimed"),
        note("n9", 6 * 86_400, "bob", "imp-9"),
    ];
    let html = Case {
        window: "30d",
        ..collab(events)
    }
    .render();

    assert!(html.has_id("activity-threads"), "{}", html.0);
    assert!(html.has("b class=\"pk-ag-id\">imp-9</b>"));
    assert!(
        html.has("pk-ag-ins-dot pk-ag-warn"),
        "a stale holder carries a dot"
    );
    assert!(!html.has("pk-ag-thread-head") && !html.has("pk-ag-steps"));
}

#[test]
fn two_items_with_one_agent_each_are_no_collaboration() {
    let html = collab(vec![
        note("n1", 300, "amy", "imp-1"),
        note("n2", 200, "bob", "imp-2"),
    ])
    .render();

    assert!(html.has_id("activity-collaboration-empty"), "{}", html.0);
    assert!(!html.has_id("activity-pairs"));
    assert!(!html.has_id("activity-threads"));
}

#[test]
fn a_hundred_thousand_events_keep_both_tables_to_a_window_of_rows() {
    let html = collab(crowd(100_000, 40)).render();

    for (table, row) in [("activity-pairs", "pair-"), ("activity-threads", "thread-")] {
        let total: usize = attr(html.tag(table), "data-total").parse().unwrap();
        assert!(total >= 60, "{table} holds {total} rows");
        let mounted = html.count(&format!("data-id=\"{row}"));
        assert!(
            mounted <= WINDOW_COUNT && mounted > 0,
            "{table} mounted {mounted} rows of {total}"
        );
    }
    let threads: usize = attr(html.tag("activity-threads"), "data-total")
        .parse()
        .unwrap();
    assert!(threads > 30_000, "{threads}");
    assert!(html.0.len() < 400_000, "{} bytes of markup", html.0.len());
}

#[test]
fn collab_mode_draws_no_lanes() {
    let html = collab(trio()).render();

    assert!(!html.has_id("activity-sequence"));
    assert!(!html.has("pk-ag-seq-life"));
    assert!(!html.has_id("activity-lanes-menu"));
}

#[test]
fn the_stylesheet_styles_every_class_the_sequence_the_menu_and_the_tables_emit() {
    let pages = [
        lanes(trio()).render(),
        Case {
            open: Some(Node::Event("t1".to_string())),
            lane: Some("amy".to_string()),
            ..lanes(trio())
        }
        .render(),
        lanes(pairs_of(40)).render(),
        collab(trio()).render(),
    ];
    let own = ["pk-ag-id", "pk-ag-tee", "pk-ag-faint", "pk-ag-tail"];
    let mut seen = BTreeSet::new();
    for page in &pages {
        for (at, _) in page.0.match_indices("class=\"") {
            let rest = &page.0[at + 7..];
            for token in rest[..rest.find('"').unwrap()].split_whitespace() {
                let mine = token.starts_with("pk-ag-seq")
                    || token.starts_with("pk-ag-lane")
                    || own.contains(&token);
                if mine {
                    seen.insert(token.to_string());
                }
            }
        }
    }
    assert!(seen.len() >= 20, "{seen:?}");
    for token in &seen {
        assert!(CSS.contains(&format!(".{token}")), "no rule for .{token}");
    }
}

/// `n` notes over the last day: items of three events, each by a different
/// one of `agents`, so every note after an item's first passes a baton.
fn crowd(n: usize, agents: usize) -> Vec<Event> {
    (0..n)
        .map(|k| {
            let item = k / 3;
            let agent = format!("agent-{:02}", (item * 7 + (k % 3) * 13) % agents);
            let ago = 86_000 - (k as i64 * 80_000) / n as i64;
            note(&format!("e{k}"), ago, &agent, &format!("imp-{item}"))
        })
        .collect()
}
