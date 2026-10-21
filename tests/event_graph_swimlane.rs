//! The swimlane painter's markup against real SSR output: the toolbar, the
//! no-item head, the canvas of the GRAPH mode and the sample window at full
//! size. The LANES and COLLAB bodies have their own module.
//!
//! Layout numbers come from the core model, so these cases check that the
//! painter draws what the model says and wires the controls the state names.

#[path = "event_graph_swimlane/bodies.rs"]
mod bodies;

use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit::widgets::event_graph::swimlane::EventGraph;
use panel_kit_core::widgets::event_graph::query::{GraphState, Mode, Tone};
use panel_kit_core::widgets::event_graph::sample;
use panel_kit_core::widgets::event_graph::swimlane::{fmt1, view, Graph, TickKind};
use panel_kit_core::widgets::event_graph::{
    Edge, EdgeKind, Event, EventKind, Lane, LaneKind, SessionTree,
};
use rstest::rstest;

// -- fixtures -----------------------------------------------------------------

/// 2026-10-01T00:00:00Z, the start of the seven-day window ending at [`NOW`].
const SINCE: i64 = 1_790_812_800;
/// 2026-10-08T00:00:00Z.
const NOW: i64 = 1_791_417_600;

/// The instant `h` hours after the window opens, in microseconds.
fn hour(h: f64) -> i64 {
    SINCE * 1_000_000 + (h * 3_600_000_000.0).round() as i64
}

/// A note by `worker` on `item` (or on none) `h` hours into the window.
fn at(item: Option<&str>, h: f64, kind: EventKind) -> Event {
    let id = format!("{}@{h}/{}", item.unwrap_or("-"), kind.as_str());
    let event = Event::new(id, hour(h), kind).agent("worker");
    match item {
        Some(item) => event.item(item),
        None => event,
    }
}

fn note(item: &str, h: f64) -> Event {
    at(Some(item), h, EventKind::Note)
}

fn status(item: &str, h: f64, status: &str) -> Event {
    at(Some(item), h, EventKind::ItemStatus).status(status)
}

/// What a graph renders, with the knobs the cases turn.
#[derive(Clone)]
struct Case {
    id: &'static str,
    state: GraphState,
    events: Vec<Event>,
    tree: Option<SessionTree>,
    selected: Option<String>,
}

impl PartialEq for Case {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

/// A seven-day graph at 1400px over `events`.
fn graph(events: Vec<Event>) -> Case {
    Case {
        id: "activity",
        state: GraphState {
            window: "7d".to_string(),
            graph_width: 1400,
            ..GraphState::default()
        },
        events,
        tree: None,
        selected: None,
    }
}

impl Case {
    fn state(mut self, edit: impl FnOnce(&mut GraphState)) -> Self {
        edit(&mut self.state);
        self
    }

    fn mode(self, mode: Mode) -> Self {
        self.state(|state| state.mode = mode)
    }

    fn picked(mut self, item: &str) -> Self {
        self.selected = Some(item.to_string());
        self
    }

    /// The graph the model lays out for this case.
    fn layout(&self) -> Graph {
        view::graph(&self.events, &self.state, self.tree.as_ref(), NOW)
    }

    fn render(self) -> Html {
        let mut dom = VirtualDom::new_with_props(Probe, ProbeProps { case: self });
        dom.rebuild_in_place();
        Html(dioxus_ssr::render(&dom))
    }
}

/// Handlers need a runtime, so the graph is built inside a component.
#[component]
fn Probe(case: Case) -> Element {
    rsx! {
        EventGraph {
            id: case.id,
            state: case.state,
            events: Rc::new(case.events),
            tree: case.tree.map(Rc::new),
            now: NOW,
            selected: case.selected,
            on_action: |_| {},
            on_select: |_| {},
        }
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

    fn position(&self, needle: &str) -> usize {
        self.0
            .find(needle)
            .unwrap_or_else(|| panic!("no {needle}: {}", self.0))
    }

    /// Opening tags `<name …>` in order.
    fn tags(&self, name: &str) -> Vec<&str> {
        let open = format!("<{name} ");
        self.0
            .match_indices(&open)
            .map(|(at, _)| &self.0[at..at + self.0[at..].find('>').unwrap() + 1])
            .collect()
    }

    /// The first opening tag `<name …>` carrying `needle`.
    fn find(&self, name: &str, needle: &str) -> &str {
        self.tags(name)
            .into_iter()
            .find(|tag| tag.contains(needle))
            .unwrap_or_else(|| panic!("no <{name}> with {needle}: {}", self.0))
    }

    /// The opening tag of the element with this id.
    fn tag(&self, id: &str) -> &str {
        let needle = format!("id=\"{id}\"");
        let hit = self
            .0
            .find(&needle)
            .unwrap_or_else(|| panic!("no element #{id}: {}", self.0));
        let start = self.0[..hit].rfind('<').unwrap();
        &self.0[start..hit + self.0[hit..].find('>').unwrap() + 1]
    }

    /// The text of a leaf element, up to its first child or close tag.
    fn text(&self, id: &str) -> &str {
        let tag = self.tag(id);
        let end = self.position(tag) + tag.len();
        &self.0[end..end + self.0[end..].find('<').unwrap()]
    }

    /// Markup from the first `from` to the first `until` after it, or the end.
    fn between(&self, from: &str, until: &str) -> &str {
        let start = self.position(from);
        let stop = self.0[start + from.len()..]
            .find(until)
            .map_or(self.0.len(), |at| start + from.len() + at);
        &self.0[start..stop]
    }

    /// One item's row: its open tag to the next item or the axis.
    fn item(&self, id: &str) -> &str {
        let open = format!("<g id=\"activity-graph-{id}\"");
        let start = self.position(&open);
        let rest = &self.0[start + open.len()..];
        let stop = ["<g id=\"activity-graph-", "<g class=\"pk-ag-axis\""]
            .iter()
            .filter_map(|next| rest.find(next))
            .min()
            .unwrap_or(rest.len());
        &self.0[start..start + open.len() + stop]
    }

    /// The legend list.
    fn legend(&self) -> &str {
        self.between("<ul id=\"activity-graph-legend\"", "</ul>")
    }

    /// Classes of every `<li data-tone=…>` legend entry in order.
    fn tones(&self) -> Vec<String> {
        self.legend()
            .match_indices("data-tone=\"")
            .map(|(at, key)| {
                let rest = &self.legend()[at + key.len()..];
                rest[..rest.find('"').unwrap()].to_string()
            })
            .collect()
    }

    /// The legend toggle of one tone.
    fn chip(&self, tone: Tone) -> &str {
        let entry = format!("data-tone=\"{}\"", tone.name());
        let legend = self.legend();
        let at = legend.find(&entry).unwrap();
        let button = legend[at..].find("<button").unwrap() + at;
        &legend[button..button + legend[button..].find('>').unwrap() + 1]
    }
}

/// The attribute `name="…"` of an opening tag.
fn attr<'a>(tag: &'a str, name: &str) -> &'a str {
    let key = format!(" {name}=\"");
    let at = tag
        .find(&key)
        .unwrap_or_else(|| panic!("no {name} in {tag}"));
    let rest = &tag[at + key.len()..];
    &rest[..rest.find('"').unwrap()]
}

/// A session tree with one collab lane under `imp-1`.
fn forked() -> SessionTree {
    let lane = |id: &str, label: &str, kind: LaneKind, from: f64, to: f64, chosen| Lane {
        id: format!("imp-1/{id}"),
        item_id: "imp-1".to_string(),
        kind,
        label: label.to_string(),
        from: hour(from),
        to: hour(to),
        chosen,
        parent: None,
        agent: Some(label.to_string()),
    };
    let edge = |kind, lane: &str, h: f64| Edge {
        kind,
        lane: format!("imp-1/{lane}"),
        item_id: "imp-1".to_string(),
        at: hour(h),
    };
    SessionTree {
        agent: None,
        lanes: vec![
            lane("collab/omp", "omp", LaneKind::Collab, 40.0, 60.0, None),
            lane(
                "branch/fix",
                "fix/imp-1",
                LaneKind::Branch,
                50.0,
                80.0,
                Some(true),
            ),
            lane(
                "path/dead",
                "drop-queue",
                LaneKind::Path,
                55.0,
                70.0,
                Some(false),
            ),
        ],
        edges: vec![
            edge(EdgeKind::Fork, "collab/omp", 40.0),
            edge(EdgeKind::Merge, "collab/omp", 60.0),
            edge(EdgeKind::Fork, "branch/fix", 50.0),
            edge(EdgeKind::Fork, "path/dead", 55.0),
        ],
    }
}

/// `imp-1` over 100 hours, forked.
fn forked_case() -> Case {
    Case {
        tree: Some(forked()),
        ..graph(vec![
            note("imp-1", 0.0),
            note("imp-1", 45.0),
            note("imp-1", 100.0),
        ])
    }
    .state(|state| state.tree = true)
}

// -- the shell ----------------------------------------------------------------

#[test]
fn the_root_is_a_group_whose_ids_follow_the_id_prop() {
    let html = Case {
        id: "dock",
        ..graph(vec![note("imp-1", 5.0), at(None, 6.0, EventKind::Commit)])
    }
    .render();
    let root = html.find("div", "id=\"dock\"");
    for attr in [
        "class=\"pk-widget pk-ag\"",
        "role=\"group\"",
        "aria-label=\"event graph\"",
        "data-mode=\"graph\"",
    ] {
        assert!(root.contains(attr), "{attr} missing: {root}");
    }
    for id in [
        "dock-modes-graph",
        "dock-filters",
        "dock-filters-input",
        "dock-filters-options",
        "dock-filters-tree",
        "dock-graph",
        "dock-graph-head",
        "dock-graph-legend",
        "dock-strip-commit",
        "dock-strip-count-commit",
        "dock-strip-row-commit",
        "dock-graph-imp-1",
        "dock-hatch",
    ] {
        assert!(html.has_id(id), "{id} missing: {}", html.0);
    }
    assert!(!html.has("id=\"activity"), "no id escapes the prefix");
}

#[rstest]
#[case(Mode::Graph, "activity-modes-graph")]
#[case(Mode::Lanes, "activity-modes-lanes")]
#[case(Mode::Collab, "activity-modes-collaboration")]
fn the_current_mode_chip_is_pressed_and_the_others_are_not(#[case] mode: Mode, #[case] id: &str) {
    let html = graph(vec![note("imp-1", 5.0)]).mode(mode).render();
    let chips = html.tags("button");
    let chips: Vec<&&str> = chips.iter().filter(|tag| tag.contains("-modes-")).collect();
    assert_eq!(chips.len(), 3);
    for tag in chips {
        let pressed = tag.contains(&format!("id=\"{id}\""));
        assert_eq!(attr(tag, "aria-pressed"), pressed.to_string(), "{tag}");
        assert_eq!(tag.contains("pk-ag-chip on"), pressed, "{tag}");
        assert!(tag.contains("type=\"button\""), "{tag}");
    }
    assert!(html.has(&format!("data-mode=\"{}\"", mode.token())));
}

#[test]
fn chips_read_graph_lanes_collab_and_say_what_they_show() {
    let html = graph(vec![note("imp-1", 5.0)]).render();
    for (id, label) in [
        ("graph", "graph"),
        ("lanes", "lanes"),
        ("collaboration", "collab"),
    ] {
        let tag = html.tag(&format!("activity-modes-{id}"));
        assert_eq!(attr(tag, "aria-label"), format!("show {label}"));
        let body = html.between(tag, "</button>");
        assert!(body.ends_with(label), "{body}");
        assert!(body.contains("pk-ag-chip-dot"), "{body}");
    }
}

#[rstest]
#[case(Mode::Graph, true, true, true)]
#[case(Mode::Lanes, true, false, false)]
#[case(Mode::Collab, false, false, false)]
fn each_mode_draws_only_the_pieces_it_uses(
    #[case] mode: Mode,
    #[case] query: bool,
    #[case] legend: bool,
    #[case] figure: bool,
) {
    let html = graph(vec![note("imp-1", 5.0)]).mode(mode).render();
    assert_eq!(html.has_id("activity-filters"), query, "query line");
    assert_eq!(html.has_id("activity-filters-tree"), query, "TREE toggle");
    assert_eq!(html.has_id("activity-graph-legend"), legend, "legend");
    assert_eq!(html.has_id("activity-graph"), figure, "graph figure");
}

#[test]
fn compact_puts_chips_query_and_legend_in_one_bar_in_that_order() {
    let html = graph(vec![note("imp-1", 5.0)])
        .state(|state| state.compact = true)
        .render();
    assert!(html
        .find("div", "id=\"activity\"")
        .contains("pk-ag-compact"));
    let bar = html.tag("activity-bar");
    assert!(bar.contains("role=\"toolbar\""), "{bar}");
    let inside = html.between(bar, "<div class=\"pk-ag-body\"");
    let chips = inside.find("activity-modes-graph").unwrap();
    let query = inside.find("activity-filters-input").unwrap();
    let legend = inside.find("activity-graph-legend").unwrap();
    assert!(chips < query && query < legend, "{inside}");
    // The legend is compact there: colour words only, no meanings.
    assert!(
        html.legend().contains("pk-ag-legend-compact"),
        "{}",
        html.legend()
    );
    assert!(!html.has("pk-ag-legend-means"));
    assert!(!html.has("pk-ag-stack"));
}

#[test]
fn the_full_register_stacks_the_toolbar_and_puts_the_legend_in_the_head() {
    let html = graph(vec![note("imp-1", 5.0)]).render();
    assert!(!html.has_id("activity-bar"));
    assert!(html.has("pk-ag-stack"));
    assert!(!html
        .find("div", "id=\"activity\"")
        .contains("pk-ag-compact"));
    let head = html.between(
        "<div id=\"activity-graph-head\"",
        "<div class=\"pk-ag-canvas\"",
    );
    assert!(head.contains("id=\"activity-graph-legend\""), "{head}");
    // The meanings ride beside the colour words, three at most and an ellipsis.
    assert!(
        head.contains("claimed · in progress · branch head …"),
        "{head}"
    );
    assert!(
        head.contains("<span class=\"pk-ag-legend-means\">done · pr merged · run finished</span>")
    );
}

// -- the query line -----------------------------------------------------------

#[test]
fn the_box_shows_the_canonical_line_and_offers_whole_line_completions() {
    let html = graph(vec![
        note("imp-1", 5.0),
        at(Some("imp-2"), 6.0, EventKind::Commit).agent("omp"),
    ])
    .state(|state| {
        state.window = "30d".to_string();
        state.tree = true;
    })
    .render();
    let input = html.tag("activity-filters-input");
    assert_eq!(attr(input, "value"), "range:30d tree:on");
    assert_eq!(attr(input, "list"), "activity-filters-options");
    assert_eq!(attr(input, "aria-label"), "filter activity");
    assert_eq!(attr(input, "autocomplete"), "off");
    assert_eq!(
        attr(input, "placeholder"),
        "agent:worker item:imp-16 kind:commit range:7d"
    );
    assert!(input.contains("type=\"text\""), "{input}");
    let form = html.tag("activity-filters");
    assert!(form.contains("role=\"search\""), "{form}");

    // Each option is the whole line with one key changed.
    for (value, label) in [
        ("agent:omp range:30d tree:on", "agent:omp"),
        ("agent:worker range:30d tree:on", "agent:worker"),
        ("item:imp-1 range:30d tree:on", "item:imp-1"),
        ("kind:commit range:30d tree:on", "kind:commit"),
        ("tree:on", "range:24h"),
        ("range:7d tree:on", "range:7d"),
        ("range:90d tree:on", "range:90d"),
    ] {
        let option = format!("<option value=\"{value}\">{label}</option>");
        assert!(html.has(&option), "{option} missing: {}", html.0);
    }
}

#[test]
fn completions_ignore_the_filters_and_the_events_outside_the_window() {
    let html = graph(vec![
        note("imp-1", 5.0),
        at(Some("imp-2"), 6.0, EventKind::Note).agent("omp"),
        // Eight days before the window opens.
        Event::new("old", hour(-8.0 * 24.0), EventKind::Note)
            .agent("ghost")
            .item("imp-0"),
    ])
    .state(|state| state.filters.agent = Some("omp".to_string()))
    .render();
    // Filtered to omp, the line still offers worker: a pick changes one key.
    assert!(html.has("<option value=\"agent:worker range:7d\">agent:worker</option>"));
    assert!(html.has("<option value=\"agent:omp range:7d\">agent:omp</option>"));
    assert!(html.has("<option value=\"agent:omp item:imp-1 range:7d\">item:imp-1</option>"));
    assert!(
        !html.has("ghost"),
        "an event outside the window is not offered"
    );
    assert!(!html.has("item:imp-0"));
}

#[rstest]
#[case(false)]
#[case(true)]
fn tree_toggle_mirrors_the_state(#[case] tree: bool) {
    let html = graph(vec![note("imp-1", 5.0)])
        .state(|state| state.tree = tree)
        .render();
    let button = html.tag("activity-filters-tree");
    assert_eq!(attr(button, "aria-pressed"), tree.to_string(), "{button}");
    assert_eq!(button.contains("pk-ag-tree on"), tree, "{button}");
    assert_eq!(attr(button, "title"), "draw forks as child lanes");
    assert!(button.contains("type=\"button\""), "{button}");
    assert_eq!(html.text("activity-filters-tree"), "tree");
}

#[test]
fn tokens_the_last_line_ignored_are_named() {
    let quiet = graph(vec![note("imp-1", 5.0)]).render();
    assert!(!quiet.has_id("activity-filters-errors"));
    let html = graph(vec![note("imp-1", 5.0)])
        .state(|state| state.query_errors = vec!["bogus:1".to_string(), "range:5y".to_string()])
        .render();
    assert_eq!(
        html.text("activity-filters-errors"),
        "ignored: bogus:1 range:5y"
    );
    assert!(html
        .tag("activity-filters-errors")
        .contains("role=\"status\""));
}

#[test]
fn active_tokens_are_ringed_chips_that_remove_themselves() {
    let none = graph(vec![note("imp-1", 5.0)])
        .state(|state| state.window = "24h".to_string())
        .render();
    assert!(!none.has_id("activity-filters-tokens"), "no token, no row");

    let html = graph(vec![note("imp-1", 5.0)])
        .state(|state| {
            state.filters.agent = Some("omp".to_string());
            state.window = "30d".to_string();
            state.tree = true;
        })
        .render();
    let row = html.between("<ul id=\"activity-filters-tokens\"", "</ul>");
    for label in ["agent:omp", "range:30d", "tree:on"] {
        let chip = html.find("button", &format!("aria-label=\"remove {label}\""));
        assert!(chip.contains("pk-ag-token on"), "{chip}");
        assert!(
            chip.contains(&format!("title=\"remove {label}\"")),
            "{chip}"
        );
        assert!(row.contains(&format!("<span>{label}</span>")), "{row}");
    }
    assert_eq!(html.count("pk-ag-token on"), 3);
    // Collab ignores the line, so it shows no tokens either.
    let collab = graph(vec![note("imp-1", 5.0)])
        .mode(Mode::Collab)
        .state(|state| state.tree = true)
        .render();
    assert!(!collab.has_id("activity-filters-tokens"));
}

// -- the legend ---------------------------------------------------------------

#[test]
fn the_legend_lists_the_five_tones_in_order_with_their_meanings_as_tooltips() {
    let html = graph(vec![note("imp-1", 5.0)]).render();
    assert_eq!(html.tones(), ["info", "ok", "warn", "err", "neutral"]);
    let legend = html.tag("activity-graph-legend");
    assert!(legend.contains("<ul "), "{legend}");
    assert_eq!(attr(legend, "aria-label"), "legend");
    for (tone, colour, means) in [
        ("info", "blue", "claimed · in progress · branch head"),
        ("ok", "green", "done · pr merged · run finished"),
        ("warn", "yellow", "review · handoff"),
        ("err", "red", "blocked"),
        (
            "neutral",
            "hollow",
            "item created · item released · item status · note · commit",
        ),
    ] {
        let entry = html.find("li", &format!("data-tone=\"{tone}\""));
        assert!(attr(entry, "title").starts_with(means), "{entry}");
        assert!(attr(entry, "class").ends_with(tone), "{entry}");
        assert!(html.has(&format!(
            "<span class=\"pk-ag-legend-name\">{colour}</span>"
        )));
    }
    // Every colour the canvas can draw is named by a legend entry.
    for tone in Tone::ALL {
        assert!(
            html.has(&format!(
                "class=\"pk-ag-dot pk-ag-{}\" cx=\"5\"",
                tone.name()
            )),
            "{tone:?}"
        );
    }
}

#[test]
fn with_no_tone_picked_every_chip_is_plain_and_the_root_is_not_filtering() {
    let html = graph(vec![note("imp-1", 5.0)]).render();
    for tone in Tone::ALL {
        let chip = html.chip(tone);
        assert!(chip.contains("class=\"pk-ag-lg\""), "{chip}");
        assert_eq!(attr(chip, "aria-pressed"), "false", "{chip}");
    }
    assert!(!html.has("pk-ag-filtering"));
    assert!(!html.has("pk-ag-f-"));
    assert!(!html.has("pk-ag-lg-clear"));
}

#[test]
fn picked_tones_are_pressed_the_rest_fade_and_the_root_carries_the_filter() {
    let html = graph(vec![note("imp-1", 5.0)])
        .state(|state| {
            state.tones.insert(Tone::Red);
            state.tones.insert(Tone::Yellow);
        })
        .render();
    for (tone, on) in [
        (Tone::Blue, false),
        (Tone::Green, false),
        (Tone::Yellow, true),
        (Tone::Red, true),
        (Tone::Hollow, false),
    ] {
        let chip = html.chip(tone);
        assert_eq!(
            attr(chip, "aria-pressed"),
            on.to_string(),
            "{tone:?}: {chip}"
        );
        let class = attr(chip, "class");
        if on {
            assert_eq!(class, "pk-ag-lg on", "{tone:?}");
        } else {
            assert_eq!(class, "pk-ag-lg dim", "{tone:?}");
        }
    }
    let root = html.find("div", "id=\"activity\"");
    assert!(root.contains("pk-ag-filtering"), "{root}");
    assert!(
        root.contains("pk-ag-f-warn") && root.contains("pk-ag-f-err"),
        "{root}"
    );
    for off in ["info", "ok", "neutral"] {
        assert!(!root.contains(&format!("pk-ag-f-{off}")), "{root}");
    }
    let clear = html.find("button", "pk-ag-lg-clear");
    assert_eq!(attr(clear, "aria-label"), "clear colour filter");
}

#[test]
fn the_tone_filter_is_a_graph_thing_and_lanes_never_dim() {
    let html = graph(vec![note("imp-1", 5.0)])
        .mode(Mode::Lanes)
        .state(|state| {
            state.tones.insert(Tone::Red);
        })
        .render();
    assert!(!html.has("pk-ag-filtering"));
    assert!(!html.has_id("activity-graph-legend"));
}

// -- the head: no-item rows and show all ---------------------------------------

/// Ten commits with no item (five at 20h, five at 21h) and one item at 21h.
fn commits() -> Vec<Event> {
    let mut events: Vec<Event> = (0..10)
        .map(|n| {
            let h = if n < 5 { 20.0 } else { 21.0 };
            let mut event = at(None, h, EventKind::Commit);
            event.id = format!("c{n}");
            event
        })
        .collect();
    events.push(note("imp-1", 21.0));
    events
}

#[test]
fn a_no_item_row_is_a_toggle_with_its_count_and_a_density_bar() {
    let html = graph(commits()).render();
    let button = html.tag("activity-strip-commit");
    assert!(button.contains("<button "), "{button}");
    assert_eq!(attr(button, "aria-expanded"), "false");
    assert_eq!(attr(button, "title"), "10 commit events with no work item");
    assert!(button.contains("class=\"pk-ag-strip-toggle\""), "{button}");
    assert_eq!(html.text("activity-strip-count-commit"), "10");
    assert!(html.has("<span class=\"pk-ag-tl-label\">no item: commit</span>"));

    // Closed: the density bar is drawn, no dotted row.
    let row = html.between("<g id=\"activity-strip-row-commit\"", "</g>");
    assert!(row.contains("class=\"pk-ag-strip\""), "{row}");
    assert!(
        row.contains("<title>10 commit events with no work item</title>"),
        "{row}"
    );
    let cells = row.matches("fill-opacity=").count();
    assert!(cells >= 1, "{row}");
    let densest = row
        .match_indices("fill-opacity=\"")
        .map(|(at, key)| {
            let rest = &row[at + key.len()..];
            rest[..rest.find('"').unwrap()].parse::<f64>().unwrap()
        })
        .fold(0.0_f64, f64::max);
    assert_eq!(densest, 1.0, "the densest cell is fully opaque");
    assert!(!html.has("pk-ag-loose"));
    assert_eq!(html.count("class=\"pk-ag-item"), 1, "commits are not rows");
}

#[test]
fn an_open_no_item_row_becomes_a_dashed_line_of_dots() {
    let html = graph(commits())
        .state(|state| state.loose_kinds = vec!["commit".to_string()])
        .render();
    let button = html.tag("activity-strip-commit");
    assert_eq!(attr(button, "aria-expanded"), "true");
    assert!(button.contains("pk-ag-strip-toggle on"), "{button}");
    assert!(html.has("class=\"pk-ag-loose\""));
    assert!(html.has("class=\"pk-ag-loose-line\""));
    assert!(
        !html.has_id("activity-strip-row-commit"),
        "an open row is not also a bar"
    );
    // The same ten events, merged into bubbles by proximity.
    let row = html.between("<g class=\"pk-ag-loose\"", "<g id=\"activity-graph-imp-1\"");
    assert!(row.contains("pk-ag-cluster"), "{row}");
    let total: usize = row
        .match_indices("data-count=\"")
        .map(|(at, key)| {
            let rest = &row[at + key.len()..];
            rest[..rest.find('"').unwrap()].parse::<usize>().unwrap()
        })
        .sum();
    assert_eq!(total, 10);
}

#[test]
fn every_kind_gets_its_own_row_in_name_order_and_only_the_named_one_opens() {
    let mut events = commits();
    events.push(at(None, 22.0, EventKind::BranchHead));
    let html = graph(events)
        .state(|state| state.loose_kinds = vec!["commit".to_string()])
        .render();
    let branch = html.tag("activity-strip-branch_head");
    let commit = html.tag("activity-strip-commit");
    assert_eq!(attr(branch, "aria-expanded"), "false");
    assert_eq!(attr(commit, "aria-expanded"), "true");
    assert!(html.position("activity-strip-branch_head") < html.position("activity-strip-commit"));
    assert!(html.has("<span class=\"pk-ag-tl-label\">no item: branch head</span>"));
    assert!(html.has_id("activity-strip-row-branch_head"));
}

#[test]
fn show_all_appears_only_when_rows_are_hidden() {
    let many: Vec<Event> = (1..=30)
        .map(|n| note(&format!("imp-{n}"), n as f64 * 4.0))
        .collect();
    let html = graph(many.clone()).render();
    let button = html.tag("activity-show-all");
    assert_eq!(attr(button, "aria-pressed"), "false");
    assert!(button.contains("class=\"pk-ag-strip-toggle\""), "{button}");
    assert_eq!(html.text("activity-show-all"), "show all (30)");
    assert_eq!(html.count("class=\"pk-ag-item"), 20, "the newest twenty");
    assert!(html.has_id("activity-graph-imp-30"));
    assert!(!html.has_id("activity-graph-imp-1"));

    let all = graph(many).state(|state| state.show_all = true).render();
    let button = all.tag("activity-show-all");
    assert_eq!(attr(button, "aria-pressed"), "true");
    assert!(button.contains("pk-ag-strip-toggle on"), "{button}");
    assert_eq!(all.text("activity-show-all"), "show newest 20");
    assert_eq!(all.count("class=\"pk-ag-item"), 30);

    let few = graph(
        (1..=5)
            .map(|n| note(&format!("imp-{n}"), n as f64))
            .collect(),
    )
    .render();
    assert!(!few.has_id("activity-show-all"));
}

#[test]
fn an_empty_window_is_one_line_with_no_canvas() {
    let html = graph(vec![]).render();
    assert_eq!(
        html.text("activity-graph-empty"),
        "nothing happened in this window"
    );
    assert!(html.tag("activity-graph-empty").contains("pk-ag-empty"));
    assert!(!html.has("<svg class=\"pk-ag-svg\""));
    assert!(!html.has_id("activity-graph-head"));
    // The toolbar is still there to widen the window with.
    assert!(html.has_id("activity-filters-input"));
}

// -- the canvas ---------------------------------------------------------------

#[test]
fn the_svg_is_the_layout_sized_and_named() {
    let case = graph(vec![note("imp-1", 5.0), note("imp-2", 50.0)]);
    let layout = case.layout();
    let html = case.render();
    let svg = html.find("svg", "class=\"pk-ag-svg\"");
    assert_eq!(attr(svg, "width"), "1400");
    assert_eq!(attr(svg, "height"), layout.height.to_string());
    assert_eq!(attr(svg, "viewBox"), format!("0 0 1400 {}", layout.height));
    assert_eq!(attr(svg, "aria-label"), layout.aria_label());
    assert_eq!(attr(svg, "role"), "group");
    let narrow = graph(vec![note("imp-1", 5.0)])
        .state(|state| state.graph_width = 640)
        .render();
    assert_eq!(
        attr(narrow.find("svg", "class=\"pk-ag-svg\""), "width"),
        "640"
    );
}

#[test]
fn an_item_is_one_button_with_a_line_its_dots_and_its_id_at_the_tip() {
    let case = graph(vec![
        note("imp-7", 10.0).summary("first"),
        note("imp-7", 12.0).summary("second"),
        status("imp-7", 14.0, "review").title("Seven"),
    ]);
    let layout = case.layout();
    let item = &layout.items[0];
    let html = case.render();
    let row = html.item("imp-7");
    let open = html.tag("activity-graph-imp-7");
    for attr in [
        "role=\"button\"",
        "tabindex=\"0\"",
        "aria-pressed=\"false\"",
        "data-id=\"imp-7\"",
        "aria-label=\"imp-7 · Seven · review\"",
        "class=\"pk-ag-item pk-ag-warn\"",
    ] {
        assert!(open.contains(attr), "{attr} missing: {open}");
    }
    assert!(
        row.contains("<title>imp-7 · Seven · review</title>"),
        "{row}"
    );
    // The hit rectangle and the line are the model's, to the decimal.
    let hit = item.hit_rect();
    assert!(
        row.contains(&format!(
            "<rect class=\"pk-ag-hit\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\">",
            fmt1(hit.x),
            fmt1(hit.y),
            fmt1(hit.w),
            fmt1(hit.h)
        )),
        "{row}"
    );
    assert!(
        row.contains(&format!(
            "<line class=\"pk-ag-line\" x1=\"{}\" x2=\"{}\" y1=\"{y}\" y2=\"{y}\">",
            fmt1(item.x0),
            fmt1(item.x1),
            y = fmt1(item.y)
        )),
        "{row}"
    );
    assert!(
        row.contains(&format!(
            "<text class=\"pk-ag-label\" x=\"{}\" y=\"{}\" dominant-baseline=\"central\">imp-7</text>",
            fmt1(item.label_x),
            fmt1(item.y)
        )),
        "{row}"
    );
}

#[test]
fn a_line_of_a_hundred_and_twenty_pixels_is_labelled_at_both_ends() {
    let case = graph(vec![
        note("imp-9", 10.0),
        note("imp-9", 30.0),
        note("imp-3", 50.0),
    ]);
    let layout = case.layout();
    let nine = layout.items.iter().find(|item| item.id == "imp-9").unwrap();
    let three = layout.items.iter().find(|item| item.id == "imp-3").unwrap();
    assert!(nine.start_label_x.is_some() && three.start_label_x.is_none());
    let html = case.render();
    assert_eq!(html.item("imp-9").matches("pk-ag-label-start").count(), 1);
    let start = html.find("text", "class=\"pk-ag-label pk-ag-label-start\"");
    assert_eq!(attr(start, "text-anchor"), "end");
    assert_eq!(attr(start, "x"), fmt1(nine.start_label_x.unwrap()));
    assert_eq!(html.item("imp-9").matches(">imp-9</text>").count(), 2);
    assert!(!html.item("imp-3").contains("pk-ag-label-start"));
    assert_eq!(html.item("imp-3").matches(">imp-3</text>").count(), 1);
}

#[rstest]
#[case("done", "pk-ag-item pk-ag-ok pk-ag-done")]
#[case("review", "pk-ag-item pk-ag-warn")]
#[case("blocked", "pk-ag-item pk-ag-err")]
#[case("in_progress", "pk-ag-item pk-ag-info")]
#[case("open", "pk-ag-item pk-ag-neutral")]
fn the_newest_status_tints_the_line_and_done_dims_it(
    #[case] status_name: &str,
    #[case] class: &str,
) {
    let html = graph(vec![
        status("imp-1", 5.0, "claimed"),
        status("imp-1", 9.0, status_name),
    ])
    .render();
    assert_eq!(attr(html.tag("activity-graph-imp-1"), "class"), class);
}

#[test]
fn a_merged_pull_request_marks_the_item_done_whatever_its_status() {
    let html = graph(vec![
        status("imp-1", 5.0, "review"),
        at(Some("imp-1"), 9.0, EventKind::PrMerged),
    ])
    .render();
    assert_eq!(
        attr(html.tag("activity-graph-imp-1"), "class"),
        "pk-ag-item pk-ag-warn pk-ag-done"
    );
}

#[test]
fn a_burst_is_one_numbered_bubble_with_the_severest_tone_and_every_title() {
    let case = graph(vec![
        status("imp-1", 0.0, "blocked").summary("failed"),
        note("imp-1", 0.0).summary("retrying"),
        note("imp-1", 90.0),
    ]);
    let html = case.render();
    let row = html.item("imp-1");
    assert_eq!(row.matches("pk-ag-cluster ").count(), 1, "{row}");
    let cluster = html.find("g", "class=\"pk-ag-cluster ");
    assert_eq!(
        attr(cluster, "class"),
        "pk-ag-cluster pk-ag-glyph pk-ag-err"
    );
    assert_eq!(attr(cluster, "data-count"), "2");
    assert!(row.contains("<text class=\"pk-ag-cluster-count\""), "{row}");
    assert!(row.contains(">2</text>"), "{row}");
    let tip = html.between("<g class=\"pk-ag-cluster", "</g>");
    assert!(
        tip.contains("<title>2 events\nOct 1 00:00 · item status · worker · failed"),
        "{tip}"
    );
    assert!(
        tip.contains("Oct 1 00:00 · note · worker · retrying"),
        "{tip}"
    );
    // The bubble is r=5 and the lone dot after it r=3.
    assert!(tip.contains("r=\"5\""), "{tip}");
    assert_eq!(row.matches("r=\"3\"").count(), 1, "{row}");
}

#[test]
fn five_coincident_events_are_one_glyph() {
    let mut events = vec![note("imp-1", 29.0)];
    events.extend((0..5).map(|n| {
        let mut event = note("imp-1", 30.0);
        event.id = format!("burst{n}");
        event
    }));
    events.push(note("imp-1", 120.0));
    let html = graph(events).render();
    assert_eq!(html.count("class=\"pk-ag-cluster "), 1);
    assert_eq!(html.count("class=\"pk-ag-cluster-count\""), 1);
    assert_eq!(attr(html.find("g", "pk-ag-cluster "), "data-count"), "5");
    assert!(html.has(">5</text>"));
    // Two lone dots and the bubble: three glyphs for seven events.
    assert_eq!(html.item("imp-1").matches("pk-ag-glyph").count(), 3);
}

#[test]
fn dot_tones_are_the_layouts_and_hollow_is_neutral() {
    let case = graph(vec![
        at(Some("imp-1"), 1.0, EventKind::Note),
        at(Some("imp-1"), 10.0, EventKind::RunStarted),
        at(Some("imp-1"), 20.0, EventKind::PrMerged),
        at(Some("imp-1"), 30.0, EventKind::Handoff),
        status("imp-1", 40.0, "blocked"),
    ]);
    let html = case.render();
    let row = html.item("imp-1");
    for tone in ["neutral", "info", "ok", "warn", "err"] {
        assert!(
            row.contains(&format!("class=\"pk-ag-dot pk-ag-glyph pk-ag-{tone}\"")),
            "{tone}: {row}"
        );
    }
    assert_eq!(row.matches("pk-ag-glyph").count(), 5);
}

#[test]
fn every_dot_the_model_lays_out_is_drawn_once_at_its_pixel() {
    let case = forked_case().state(|state| state.show_all = true);
    let layout = case.layout();
    let html = case.render();
    for item in &layout.items {
        let row = html.item(&item.id);
        for dot in &item.dots {
            let at = format!("cx=\"{}\" cy=\"{}\"", fmt1(dot.x), fmt1(item.y));
            assert!(row.contains(&at), "{at}: {row}");
        }
        let glyphs: usize = item.dots.len();
        assert_eq!(row.matches("pk-ag-glyph").count(), glyphs, "{}", item.id);
    }
}

#[test]
fn titles_are_real_svg_titles_and_user_text_cannot_break_out() {
    let html = graph(vec![
        note("imp-1", 5.0).summary("<script>alert(1)</script> & </title><b>"),
        note("imp-1", 40.0),
    ])
    .render();
    assert!(!html.has("<script"), "{}", html.0);
    assert!(!html.has("<b>"), "{}", html.0);
    assert!(html.has("&lt;script&gt;alert(1)&lt;/script&gt; &amp; &lt;/title&gt;&lt;b&gt;"));
    // The title is the element's own markup, a child of the dot it describes.
    let dot = html.between("<circle class=\"pk-ag-dot pk-ag-glyph", "</circle>");
    assert!(
        dot.contains("><title>Oct 1 05:00 · note · worker · "),
        "{dot}"
    );
}

#[test]
fn idle_stretches_are_hatched_gaps_with_a_tooltip() {
    let case = graph(vec![note("imp-1", 144.0), note("imp-1", 150.0)]);
    let layout = case.layout();
    assert!(!layout.gaps.is_empty());
    let html = case.render();
    assert!(html.has("<pattern id=\"activity-hatch\" width=\"4\" height=\"4\" patternUnits=\"userSpaceOnUse\" patternTransform=\"rotate(45)\">"));
    assert!(html.has("<line class=\"pk-ag-hatch-line\""));
    assert_eq!(html.count("class=\"pk-ag-gap\""), layout.gaps.len());
    for gap in &layout.gaps {
        let rect = format!(
            "<rect class=\"pk-ag-gap\" x=\"{}\" y=\"0\" width=\"24.0\" height=\"{}\" fill=\"url(#activity-hatch)\"><title>{} with nothing happening</title></rect>",
            fmt1(gap.x0),
            fmt1(layout.axis_y),
            gap.idle_label()
        );
        assert!(html.has(&rect), "{rect}\n{}", html.0);
    }
    assert!(html.has("6d with nothing happening"));
}

#[test]
fn the_grid_has_a_line_per_tick_and_the_axis_a_label() {
    let case = graph(vec![
        note("imp-1", 0.5),
        note("imp-1", 1.5),
        note("imp-1", 50.0),
    ]);
    let layout = case.layout();
    assert!(!layout.ticks.is_empty());
    let html = case.render();
    let lines = html.tags("line");
    let ticks: Vec<&&str> = lines
        .iter()
        .filter(|tag| tag.contains("pk-ag-tick "))
        .collect();
    assert_eq!(ticks.len(), layout.ticks.len());
    for (tag, tick) in ticks.iter().zip(&layout.ticks) {
        assert_eq!(attr(tag, "x1"), fmt1(tick.x));
        assert_eq!(attr(tag, "x2"), fmt1(tick.x));
        assert!(
            tag.contains(&format!("pk-ag-tick-{}", tick.kind.as_str())),
            "{tag}"
        );
        let end = if tick.kind == TickKind::Day {
            layout.height.to_string()
        } else {
            fmt1(layout.axis_y)
        };
        assert_eq!(
            attr(tag, "y2"),
            end,
            "day lines run the full height, time lines stop at the axis"
        );
    }
    let axis = html.between("<g class=\"pk-ag-axis\"", "</g>");
    for tick in &layout.ticks {
        assert!(
            axis.contains(&format!(">{}</text>", tick.label)),
            "{} in {axis}",
            tick.label
        );
    }
    assert_eq!(axis.matches("<text ").count(), layout.ticks.len());
    assert!(axis.contains("aria-hidden=\"true\""));
}

#[test]
fn only_the_picked_item_is_marked_and_pressed() {
    let events = vec![note("imp-1", 5.0), note("imp-2", 9.0), note("imp-3", 13.0)];
    let html = graph(events.clone()).picked("imp-2").render();
    for (id, on) in [("imp-1", false), ("imp-2", true), ("imp-3", false)] {
        let tag = html.tag(&format!("activity-graph-{id}"));
        assert_eq!(tag.contains(" on\""), on, "{id}: {tag}");
        assert_eq!(attr(tag, "aria-pressed"), on.to_string(), "{id}: {tag}");
    }
    assert_eq!(html.count("pk-ag-item pk-ag-neutral on"), 1);
    let none = graph(events).render();
    assert_eq!(
        none.count("aria-pressed=\"true\""),
        1,
        "only the graph chip: {}",
        none.0
    );
    assert!(!none.has("pk-ag-neutral on"));
}

// -- forks ---------------------------------------------------------------------

#[test]
fn forks_draw_their_line_tip_square_caption_and_elbows() {
    let case = forked_case();
    let layout = case.layout();
    let forks = &layout.items[0].forks;
    assert_eq!(forks.len(), 3);
    let html = case.render();
    let row = html.item("imp-1");
    assert_eq!(row.matches("<g class=\"pk-ag-fork").count(), 3);
    let collab = &forks[0];
    let g = html.find("g", "data-kind=\"collab\"");
    assert!(g.contains("class=\"pk-ag-fork\""), "{g}");
    let block = html.between(g, "</g>");
    assert!(block.contains("<title>collab omp</title>"), "{block}");
    assert!(
        block.contains(&format!(
            "<line class=\"pk-ag-line pk-ag-fork-line\" x1=\"{}\" x2=\"{}\" y1=\"{y}\" y2=\"{y}\">",
            fmt1(collab.x0),
            fmt1(collab.x1),
            y = fmt1(collab.y)
        )),
        "{block}"
    );
    // The tip node is a 7px square centred on the line's end; the caption follows it.
    assert!(
        block.contains(&format!(
            "<rect class=\"pk-ag-fork-node pk-ag-neutral\" x=\"{}\" y=\"{}\" width=\"7\" height=\"7\">",
            fmt1(collab.node_left()),
            fmt1(collab.y - 3.5)
        )),
        "{block}"
    );
    assert!(
        block.contains(&format!("x=\"{}\"", fmt1(collab.x1 + 7.5))),
        "{block}"
    );
    assert!(block.contains(">omp</text>"), "{block}");
    // A branch and a path are blue squares; the lane kind rides along.
    assert!(html.has("data-kind=\"branch\"") && html.has("data-kind=\"path\""));
    assert_eq!(html.count("pk-ag-fork-node pk-ag-info"), 2);
    // One elbow per edge: forks drop, merges climb back.
    assert_eq!(row.matches("class=\"pk-ag-edge pk-ag-edge-fork").count(), 3);
    assert_eq!(
        row.matches("class=\"pk-ag-edge pk-ag-edge-merge").count(),
        1
    );
    for edge in &layout.items[0].edges {
        assert!(row.contains(&format!("d=\"{}\"", edge.d)), "{}", edge.d);
    }
}

#[test]
fn a_dead_lane_fades_with_its_elbow() {
    let html = forked_case().render();
    let row = html.item("imp-1");
    assert_eq!(row.matches("pk-ag-fork pk-ag-dead").count(), 1);
    let dead = html.find("g", "pk-ag-fork pk-ag-dead");
    assert!(dead.contains("data-kind=\"path\""), "{dead}");
    assert_eq!(
        row.matches("pk-ag-edge pk-ag-edge-fork pk-ag-dead").count(),
        1
    );
}

#[test]
fn long_lane_captions_are_cut_and_keep_the_full_name_in_the_tooltip() {
    let mut tree = forked();
    tree.lanes[0].label = "selfdev@FHKDV644W6-MBP".to_string();
    let html = Case {
        tree: Some(tree),
        ..forked_case()
    }
    .render();
    assert!(html.has(">selfdev@FHKDV64…</text>"), "{}", html.0);
    assert!(html.has("<title>collab selfdev@FHKDV644W6-MBP</title>"));
}

#[test]
fn with_tree_off_no_fork_is_drawn_even_when_a_tree_is_passed() {
    let html = forked_case().state(|state| state.tree = false).render();
    assert!(!html.has("pk-ag-fork"));
    assert!(!html.has("pk-ag-edge"));
    assert!(html.has("aria-pressed=\"false\" title=\"draw forks as child lanes\""));
}

// -- the demo window at full size -----------------------------------------------

/// The sample window: 30 days, TREE on, the compact bar.
fn demo(show_all: bool) -> (Case, sample::Activity) {
    let activity = sample::activity();
    let case = Case {
        id: "activity",
        state: GraphState {
            window: "30d".to_string(),
            tree: true,
            compact: true,
            graph_width: 1200,
            show_all,
            ..GraphState::default()
        },
        events: activity.events.clone(),
        tree: Some(activity.tree.clone()),
        selected: None,
    };
    (case, activity)
}

fn render_demo(case: Case) -> Html {
    let mut dom = VirtualDom::new_with_props(DemoProbe, DemoProbeProps { case });
    dom.rebuild_in_place();
    Html(dioxus_ssr::render(&dom))
}

fn demo_layout(case: &Case) -> Graph {
    view::graph(
        &case.events,
        &case.state,
        case.tree.as_ref(),
        sample::DEMO_NOW,
    )
}

#[test]
fn the_demo_window_draws_the_newest_twenty_of_2351_with_its_three_no_item_rows() {
    let (case, _) = demo(false);
    let html = render_demo(case.clone());
    assert_eq!(html.count("class=\"pk-ag-item"), 20);
    assert_eq!(html.text("activity-show-all"), "show all (2351)");
    for (kind, count, label) in [
        ("branch_head", "758", "no item: branch head"),
        ("commit", "253", "no item: commit"),
        ("pr_merged", "212", "no item: pr merged"),
    ] {
        assert_eq!(html.text(&format!("activity-strip-count-{kind}")), count);
        assert!(html.has(&format!("<span class=\"pk-ag-tl-label\">{label}</span>")));
    }
    // The cap keeps the document small however large the window is.
    assert!(html.0.len() < 500_000, "{} bytes", html.0.len());
    // Every colour the legend names is drawn on the canvas.
    for tone in Tone::ALL {
        assert!(
            html.has(&format!("pk-ag-glyph pk-ag-{}", tone.name())),
            "{tone:?} never drawn"
        );
    }
    assert!(html.has("pk-ag-fork-node"));
    assert!(html.has("pk-ag-gap"));
    let layout = demo_layout(&case);
    assert_eq!(html.count("class=\"pk-ag-gap\""), layout.gaps.len());
}

#[test]
fn show_all_draws_every_item_of_the_demo_window() {
    let (case, _) = demo(true);
    let layout = demo_layout(&case);
    assert_eq!(layout.items.len(), 2351);
    let html = render_demo(case);
    assert_eq!(html.count("class=\"pk-ag-item"), 2351);
    assert_eq!(html.text("activity-show-all"), "show newest 20");
}

/// The demo probe renders the sample's own clock, not the fixtures'.
#[component]
fn DemoProbe(case: Case) -> Element {
    rsx! {
        EventGraph {
            state: case.state,
            events: Rc::new(case.events),
            tree: case.tree.map(Rc::new),
            now: sample::DEMO_NOW,
            on_action: |_| {},
        }
    }
}
