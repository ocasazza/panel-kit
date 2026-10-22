//! The inspector of the swimlane `EventGraph`, against real SSR output: what
//! each kind of node opens, that nothing is open by default, and which
//! elements carry the handlers that open and close it.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use dioxus::dioxus_core::Mutation;
use dioxus::prelude::*;
use panel_kit::widgets::event_graph::swimlane::EventGraph;
use panel_kit_core::widgets::event_graph::query::{GraphState, Mode};
use panel_kit_core::widgets::event_graph::swimlane::{view, Node};
use panel_kit_core::widgets::event_graph::{
    Edge, EdgeKind, Event, EventKind, Lane, LaneKind, SessionTree,
};

// -- fixtures -----------------------------------------------------------------

/// 2026-10-01T00:00:00Z, the start of the seven-day window ending at [`NOW`].
const SINCE: i64 = 1_790_812_800;
/// 2026-10-08T00:00:00Z.
const NOW: i64 = 1_791_417_600;

fn hour(h: f64) -> i64 {
    SINCE * 1_000_000 + (h * 3_600_000_000.0).round() as i64
}

fn ev(id: &str, h: f64, kind: EventKind, item: Option<&str>) -> Event {
    let event = Event::new(id, hour(h), kind).agent("worker");
    match item {
        Some(item) => event.item(item),
        None => event,
    }
}

/// `imp-1`: three events (one with a title and a status, one multi-line);
/// `imp-2`: fourteen events in one burst; and a commit that belongs to no item.
fn events() -> Vec<Event> {
    let mut all = vec![
        ev("a1", 0.0, EventKind::ItemCreated, Some("imp-1")).title("Retry ingest"),
        ev("a2", 45.0, EventKind::Note, Some("imp-1"))
            .agent("omp")
            .summary("first line\nsecond line"),
        ev("a3", 100.0, EventKind::ItemStatus, Some("imp-1")).status("review"),
        ev("loose:1", 10.0, EventKind::Commit, None)
            .summary("a loose commit")
            .commit("abc1234"),
    ];
    for n in 0..14 {
        all.push(ev(&format!("b{n}"), 20.0, EventKind::Note, Some("imp-2")));
    }
    all
}

/// [`forked`], plus a second lane under `imp-1` when `extra`.
fn tree_of(extra: bool) -> SessionTree {
    let mut tree = forked();
    if extra {
        let mut lane = tree.lanes[0].clone();
        lane.id = "imp-1/path/other".to_string();
        lane.label = "other".to_string();
        lane.kind = LaneKind::Path;
        let mut edge = tree.edges[0].clone();
        edge.lane = lane.id.clone();
        tree.lanes.push(lane);
        tree.edges.push(edge);
    }
    tree
}

fn forked() -> SessionTree {
    let lane = |id: &str, label: &str, item: &str| Lane {
        id: format!("{item}/{id}"),
        item_id: item.to_string(),
        kind: LaneKind::Branch,
        label: label.to_string(),
        from: hour(40.0),
        to: hour(80.0),
        chosen: Some(false),
        parent: None,
        agent: Some("omp".to_string()),
    };
    let edge = |lane: &str, item: &str| Edge {
        kind: EdgeKind::Fork,
        lane: format!("{item}/{lane}"),
        item_id: item.to_string(),
        at: hour(40.0),
    };
    SessionTree {
        agent: None,
        lanes: vec![lane("branch/fix", "fix/imp-1", "imp-1")],
        edges: vec![edge("branch/fix", "imp-1")],
    }
}

type Log = Rc<RefCell<Vec<Option<Node>>>>;

/// What a graph renders, with the knobs the cases turn.
#[derive(Clone)]
struct Case {
    mode: Mode,
    open: Option<Node>,
    events: Vec<Event>,
    tree: bool,
    extra_fork: bool,
    log: Log,
}

impl PartialEq for Case {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

fn case() -> Case {
    Case {
        mode: Mode::Graph,
        open: None,
        events: events(),
        tree: true,
        extra_fork: false,
        log: Log::default(),
    }
}

impl Case {
    fn open(mut self, node: Node) -> Self {
        self.open = Some(node);
        self
    }

    fn state(&self) -> GraphState {
        GraphState {
            mode: self.mode,
            window: "7d".to_string(),
            graph_width: 1400,
            tree: self.tree,
            ..GraphState::default()
        }
    }

    fn dom(&self) -> VirtualDom {
        VirtualDom::new_with_props(Probe, ProbeProps { case: self.clone() })
    }

    fn render(self) -> Html {
        let mut dom = self.dom();
        dom.rebuild_in_place();
        Html(dioxus_ssr::render(&dom))
    }
}

#[component]
fn Probe(case: Case) -> Element {
    let log = case.log.clone();
    rsx! {
        EventGraph {
            state: case.state(),
            events: Rc::new(case.events.clone()),
            tree: Rc::new(tree_of(case.extra_fork)),
            now: NOW,
            open: case.open.clone(),
            on_action: |_| {},
            on_open: move |node| log.borrow_mut().push(node),
        }
    }
}

struct Html(String);

impl Html {
    fn has(&self, needle: &str) -> bool {
        self.0.contains(needle)
    }

    fn count(&self, needle: &str) -> usize {
        self.0.matches(needle).count()
    }

    /// The inspector region, to the end of the document.
    fn inspector(&self) -> &str {
        let at = self
            .0
            .find("<section id=\"activity-inspector\"")
            .unwrap_or_else(|| panic!("no inspector: {}", self.0));
        &self.0[at..]
    }

    /// `data-node` of the inspector, if one is open.
    fn kind(&self) -> Option<String> {
        let at = self.0.find("<section id=\"activity-inspector\"")?;
        let rest = &self.0[at..];
        let key = "data-node=\"";
        let start = rest.find(key)? + key.len();
        Some(rest[start..start + rest[start..].find('"')?].to_string())
    }
}

/// The ids of the events the dot-level fixtures draw for `item`'s first dot.
fn first_dot(item: &str) -> panel_kit_core::widgets::event_graph::swimlane::Dot {
    let case = case();
    let graph = view::graph(&events(), &case.state(), Some(&forked()), NOW);
    graph
        .items
        .into_iter()
        .find(|i| i.id == item)
        .unwrap()
        .dots
        .remove(0)
}

// -- closed by default ----------------------------------------------------------

#[test]
fn nothing_is_open_until_a_node_is_clicked() {
    let html = case().render();
    assert!(!html.has("pk-ag-ins"), "{}", html.0);
    assert!(!html.has("activity-inspector"));
    assert!(!html.has("pk-rail"), "no nested rail without an open node");
    assert!(html.has("pk-ag-body"));
}

#[test]
fn an_unresolvable_node_draws_no_region() {
    let html = case().open(Node::Item("nope".to_string())).render();
    assert!(!html.has("pk-ag-ins"), "{}", html.0);
}

#[test]
fn the_inspector_opens_in_lanes_over_an_event_and_in_collab_over_a_pair() {
    let lanes = Case {
        mode: Mode::Lanes,
        ..case()
    }
    .open(Node::Event("loose:1".to_string()))
    .render();
    assert_eq!(lanes.kind().as_deref(), Some("event"));

    let collab = Case {
        mode: Mode::Collab,
        ..case()
    }
    .open(Node::Pair("omp".to_string(), "worker".to_string()))
    .render();
    assert_eq!(collab.kind().as_deref(), Some("pair"));
    let region = collab.inspector();
    for want in [
        "omp \u{2194} worker",
        "imp-1",
        "agent a",
        "shared",
        "co-commits",
    ] {
        assert!(region.contains(want), "{want} in {region}");
    }

    let stranger = Case {
        mode: Mode::Collab,
        ..case()
    }
    .open(Node::Pair("a".to_string(), "b".to_string()))
    .render();
    assert!(
        !stranger.has("pk-ag-ins"),
        "an unknown pair draws no region"
    );
}

// -- an item --------------------------------------------------------------------

#[test]
fn an_item_opens_its_header_over_its_events_as_closed_rail_rows() {
    let html = case().open(Node::Item("imp-1".to_string())).render();
    assert_eq!(html.kind().as_deref(), Some("item"));
    let region = html.inspector();
    assert!(region
        .starts_with("<section id=\"activity-inspector\" class=\"pk-ag-ins\" role=\"region\""));
    for want in [
        ">item<",
        ">imp-1<",
        ">title<",
        ">Retry ingest<",
        ">status<",
        ">review<",
        "pk-ag-ins-dot pk-ag-warn",
        ">events<",
        ">3<",
        ">first<",
        ">last<",
        ">agents<",
        ">omp, worker<",
        ">lanes<",
        ">1<",
    ] {
        assert!(region.contains(want), "{want} in {region}");
    }
    // The nested rail: one disclosure row per event, all closed.
    assert_eq!(
        region.matches("class=\"pk-rail-hit\"").count(),
        3,
        "{region}"
    );
    assert_eq!(region.matches("aria-expanded=\"true\"").count(), 0);
    assert!(!region.contains("pk-rail-detail"));
}

#[test]
fn the_region_sits_under_the_body_and_closes_with_a_text_button() {
    let html = case().open(Node::Item("imp-1".to_string())).render();
    assert!(html.0.find("pk-ag-body").unwrap() < html.0.find("pk-ag-ins\"").unwrap());
    let region = html.inspector();
    let close = &region[region
        .find("<button id=\"activity-inspector-close\"")
        .unwrap()..];
    let tag = &close[..close.find('>').unwrap()];
    assert!(tag.contains("pk-dt-btn"), "{tag}");
    assert!(tag.contains("aria-keyshortcuts=\"Escape\""), "{tag}");
    assert!(close[tag.len()..].starts_with(">close</button>"), "{close}");
}

// -- an event, a burst, a lane ----------------------------------------------------

#[test]
fn a_dot_opens_its_item_with_that_row_open() {
    let html = case().open(Node::Event("a2".to_string())).render();
    assert_eq!(html.kind().as_deref(), Some("event"));
    let region = html.inspector();
    assert_eq!(
        region.matches("aria-expanded=\"true\"").count(),
        1,
        "{region}"
    );
    assert_eq!(region.matches("class=\"pk-rail-detail\"").count(), 1);
    // The open row's facts: both lines of its multi-line text.
    assert!(
        region.contains(">first line<") && region.contains(">second line<"),
        "{region}"
    );
    assert!(region.contains(">imp-1<"), "its item's header");
}

#[test]
fn a_burst_opens_its_members_up_to_twelve_then_counts_the_rest() {
    let dot = first_dot("imp-2");
    assert_eq!(dot.members.len(), 14, "the fixture burst is one glyph");
    let html = case().open(Node::Cluster(dot.members)).render();
    assert_eq!(html.kind().as_deref(), Some("events"));
    let region = html.inspector();
    assert_eq!(
        region.matches("aria-expanded=\"true\"").count(),
        12,
        "{region}"
    );
    assert_eq!(region.matches("class=\"pk-rail-detail\"").count(), 12);
    assert!(region.contains(">+2 more<"), "{region}");
}

#[test]
fn a_fork_leaf_opens_the_lanes_facts_and_no_rail() {
    let html = case()
        .open(Node::Lane("imp-1/branch/fix".to_string()))
        .render();
    assert_eq!(html.kind().as_deref(), Some("lane"));
    let region = html.inspector();
    for want in [
        ">lane<",
        ">fix/imp-1<",
        ">outcome<",
        ">dropped<",
        ">agent<",
        ">omp<",
    ] {
        assert!(region.contains(want), "{want} in {region}");
    }
    assert!(!region.contains("pk-rail"), "{region}");
}

#[test]
fn an_event_without_an_item_shows_its_facts_not_a_rail() {
    let html = case().open(Node::Event("loose:1".to_string())).render();
    let region = html.inspector();
    assert!(
        region.contains(">abc1234<") && region.contains(">a loose commit<"),
        "{region}"
    );
    assert!(!region.contains("pk-rail"));
}

#[test]
fn values_are_one_ellipsised_line_each_under_a_title() {
    let html = case()
        .open(Node::Lane("imp-1/branch/fix".to_string()))
        .render();
    let region = html.inspector();
    assert!(
        region.contains("<span class=\"pk-ag-fact-line\" title=\"fix/imp-1\">fix/imp-1</span>"),
        "{region}"
    );
    assert!(region.contains("--pk-ag-labels: 7ch"), "{region}");
}

// -- handlers ---------------------------------------------------------------------

/// How many listeners of `name` a fresh mount of `case` registers. Events
/// cannot be dispatched here (the DOM event conversion needs dioxus-html's
/// `serialize` feature), so wiring is checked by what carries a listener.
fn listeners(case: &Case, name: &str) -> usize {
    let mut dom = case.dom();
    dom.rebuild_to_vec()
        .edits
        .iter()
        .filter(|edit| matches!(edit, Mutation::NewEventListener { name: n, .. } if *n == name))
        .count()
}

#[test]
fn a_dot_and_a_fork_leaf_each_carry_a_click_listener_and_no_key_stop() {
    let base = case();
    let mut more = events();
    more.push(ev("a4", 70.0, EventKind::Note, Some("imp-1")));
    let dot = Case {
        events: more,
        ..case()
    };
    assert_eq!(listeners(&dot, "click"), listeners(&base, "click") + 1);
    assert_eq!(listeners(&dot, "keydown"), listeners(&base, "keydown"));
    let forked = Case {
        extra_fork: true,
        ..case()
    };
    assert_eq!(listeners(&forked, "click"), listeners(&base, "click") + 1);
}

#[test]
fn an_open_inspector_adds_the_close_button_the_bar_and_one_per_rail_row() {
    let closed = case();
    let open = case().open(Node::Item("imp-1".to_string()));
    let rows = open.clone().render().count("class=\"pk-rail-hit\"");
    assert_eq!(rows, 3, "one row per event");
    assert_eq!(
        listeners(&open, "click"),
        listeners(&closed, "click") + 1 + rows
    );
    assert_eq!(
        listeners(&open, "keydown"),
        listeners(&closed, "keydown") + 1 + rows
    );
}

// -- the stylesheet ---------------------------------------------------------------

#[test]
fn the_stylesheet_styles_every_class_the_inspector_emits() {
    let css = include_str!("../../assets/panel-kit.css");
    let pages = [
        case().open(Node::Item("imp-1".to_string())).render(),
        case().open(Node::Event("loose:1".to_string())).render(),
        case()
            .open(Node::Lane("imp-1/branch/fix".to_string()))
            .render(),
        Case {
            mode: Mode::Collab,
            ..case()
        }
        .open(Node::Pair("omp".to_string(), "worker".to_string()))
        .render(),
    ];
    let mut seen = BTreeSet::new();
    for page in &pages {
        for (at, _) in page.0.match_indices("class=\"") {
            let rest = &page.0[at + 7..];
            for token in rest[..rest.find('"').unwrap()].split_whitespace() {
                if ["pk-ag-ins", "pk-ag-fact"]
                    .iter()
                    .any(|p| token.starts_with(p))
                {
                    seen.insert(token.to_string());
                }
            }
        }
    }
    assert!(seen.len() >= 12, "{seen:?}");
    for token in &seen {
        assert!(css.contains(&format!(".{token}")), "no rule for .{token}");
    }
    assert!(css.contains("--pk-ag-row:"), "--pk-ag-row is never defined");
}
