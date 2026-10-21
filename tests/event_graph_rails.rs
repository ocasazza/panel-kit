//! The rail painter's markup against real SSR output: rows, tones, chips,
//! separators, selection, dead lanes, header counts, empty state, stylesheet.

use dioxus::prelude::*;
use panel_kit::widgets::event_graph::EventRail;
use panel_kit_core::widgets::event_graph::facts::{event_facts, lane_facts, line_count, Expansion};
use panel_kit_core::widgets::event_graph::rails::{
    vertical, vertical_with, RailOptions, RailView, VerticalGraph,
};
use panel_kit_core::widgets::event_graph::time::from_utc;
use panel_kit_core::widgets::event_graph::{
    Edge, EdgeKind, Event, EventKind, ItemRef, Lane, LaneKind, Micros, SessionTree,
};

const CSS: &str = include_str!("../assets/panel-kit.css");
const MIN: Micros = 60_000_000;

// -- fixtures -----------------------------------------------------------------

fn at(minute: i64) -> Micros {
    from_utc(2026, 10, 1, 0, 0, 0) + minute * MIN
}

fn ev(id: &str, minute: i64, agent: &str, kind: EventKind) -> Event {
    Event::new(id, at(minute), kind)
        .agent(agent)
        .item("imp-16")
        .summary(format!("{id} said"))
}

fn lane(id: &str, from: i64, to: i64, agent: Option<&str>, kind: LaneKind, label: &str) -> Lane {
    Lane {
        id: id.to_owned(),
        item_id: "imp-16".to_owned(),
        kind,
        label: label.to_owned(),
        from: at(from),
        to: at(to),
        chosen: None,
        parent: None,
        agent: agent.map(str::to_owned),
    }
}

fn edge(kind: EdgeKind, lane: &str, minute: i64) -> Edge {
    Edge {
        kind,
        lane: lane.to_owned(),
        item_id: "imp-16".to_owned(),
        at: at(minute),
    }
}

/// Every tone once, a live lane (omp), a dead lane with an event (bob) and a
/// dead lane with only a marker row (feat/x).
fn events() -> Vec<Event> {
    vec![
        ev("e1", 0, "clara", EventKind::Note),
        ev("e2", 5, "clara", EventKind::ItemClaimed),
        ev("e3", 10, "omp", EventKind::RunStarted),
        ev("e4", 11, "bob", EventKind::Note),
        ev("e5", 20, "omp", EventKind::PrMerged),
        ev("e6", 30, "clara", EventKind::Handoff),
        ev("e7", 40, "clara", EventKind::ItemStatus).status("blocked"),
    ]
}

fn tree() -> SessionTree {
    let mut bob = lane("l/bob", 11, 12, Some("bob"), LaneKind::Collab, "bob");
    bob.chosen = Some(false);
    let mut feat = lane("l/feat", 25, 26, None, LaneKind::Branch, "feat/x");
    feat.chosen = Some(false);
    SessionTree {
        agent: Some("clara".to_owned()),
        lanes: vec![
            lane("l/omp", 10, 20, Some("omp"), LaneKind::Collab, "omp"),
            bob,
            feat,
        ],
        edges: vec![
            edge(EdgeKind::Fork, "l/omp", 10),
            edge(EdgeKind::Merge, "l/omp", 20),
            edge(EdgeKind::Fork, "l/bob", 11),
            edge(EdgeKind::Merge, "l/bob", 12),
            edge(EdgeKind::Fork, "l/feat", 25),
        ],
    }
}

fn graph() -> VerticalGraph {
    vertical(
        &events(),
        &ItemRef {
            id: "imp-16".to_owned(),
            status: Some("claimed".to_owned()),
        },
        &tree(),
    )
}

fn opts() -> RailOptions {
    RailOptions {
        dom_id: "rail".to_owned(),
        ..RailOptions::default()
    }
}

#[component]
fn Probe(view: RailView, #[props(default)] expanded: Expansion) -> Element {
    rsx! { EventRail { view, expanded, on_select: move |_id: String| {} } }
}

fn render(view: RailView) -> Html {
    render_with(view, &Expansion::new())
}

fn render_with(view: RailView, open: &Expansion) -> Html {
    let mut dom = VirtualDom::new_with_props(
        Probe,
        ProbeProps {
            view,
            expanded: open.clone(),
        },
    );
    dom.rebuild_in_place();
    Html(dioxus_ssr::render(&dom))
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

    /// Opening tags `<name …>` in order.
    fn tags(&self, name: &str) -> Vec<&str> {
        let open = format!("<{name} ");
        self.0
            .match_indices(&open)
            .map(|(at, _)| &self.0[at..at + self.0[at..].find('>').unwrap() + 1])
            .collect()
    }

    /// Opening tags whose `class` has `token` among its space-separated names.
    fn with_class(&self, name: &str, token: &str) -> Vec<&str> {
        self.tags(name)
            .into_iter()
            .filter(|tag| class_of(tag).split(' ').any(|c| c == token))
            .collect()
    }

    /// The `<li>` of a row, whole.
    fn row(&self, id: &str) -> &str {
        let at = self
            .0
            .find(&format!("id=\"rail-row-{id}\""))
            .unwrap_or_else(|| panic!("no row {id}: {}", self.0));
        let start = self.0[..at].rfind("<li").unwrap();
        &self.0[start..at + self.0[at..].find("</li>").unwrap()]
    }
}

fn class_of(tag: &str) -> &str {
    let at = tag.find("class=\"").map_or(0, |at| at + 7);
    tag[at..].split('"').next().unwrap_or("")
}

fn dot<'a>(html: &'a Html, tone: &str) -> Vec<&'a str> {
    html.with_class("circle", tone)
}

// -- header and rows ----------------------------------------------------------

#[test]
fn heading_shows_title_and_event_and_lane_counts_without_markers() {
    let html = render(graph().view(&opts()));
    assert!(
        html.has("<span class=\"pk-rail-title\">session tree</span>"),
        "{}",
        html.0
    );
    // 7 events; 3 lanes; the feat/x marker row is not an event.
    assert!(
        html.has("<span class=\"pk-rail-meta\">7 events · 3 lanes</span>"),
        "{}",
        html.0
    );
    assert_eq!(html.count("<li "), 8, "7 events + 1 lane marker");
}

#[test]
fn heading_can_be_switched_off_and_retitled() {
    let off = render(graph().view(&RailOptions {
        heading: false,
        ..opts()
    }));
    assert!(!off.has("<figcaption"));
    let titled = render(graph().view(&RailOptions {
        title: "timeline".to_owned(),
        ..opts()
    }));
    assert!(titled.has(">timeline</span>"));
    assert!(titled.has("aria-label=\"timeline\""));
}

#[test]
fn rows_render_in_time_order_with_stamp_and_label() {
    let html = render(graph().view(&opts()));
    let ids: Vec<usize> = ["e1", "e2", "e3", "e4", "e5", "e6", "e7"]
        .iter()
        .map(|id| html.0.find(&format!("id=\"rail-row-{id}\"")).unwrap())
        .collect();
    assert!(
        ids.windows(2).all(|w| w[0] < w[1]),
        "document order is time order"
    );
    let first = html.row("e1");
    assert!(
        first.contains(
            "<time class=\"pk-rail-time\" datetime=\"2026-10-01T00:00:00Z\">00:00</time>"
        ),
        "{first}"
    );
    assert!(
        first.contains("<span class=\"pk-rail-label\">e1 said</span>"),
        "{first}"
    );
    assert!(first.contains("data-kind=\"note\""));
}

#[test]
fn rows_spanning_days_stamp_the_date() {
    let mut late = events();
    late.push(ev("e8", 60 * 30, "clara", EventKind::Note));
    let g = vertical(
        &late,
        &ItemRef {
            id: "imp-16".to_owned(),
            status: None,
        },
        &tree(),
    );
    let html = render(g.view(&opts()));
    assert!(html.has(">10-01 00:00</time>"), "{}", html.0);
    assert!(html.has(">10-02 06:00</time>"), "{}", html.0);
}

// -- gutter -------------------------------------------------------------------

#[test]
fn svg_gutter_has_trunk_lanes_edges_and_one_dot_per_row() {
    let g = graph();
    let html = render(g.view(&opts()));
    let svg = html.tags("svg")[0];
    assert!(svg.contains(&format!("width=\"{}\"", g.gutter)), "{svg}");
    assert!(svg.contains(&format!("height=\"{}\"", g.height)), "{svg}");
    assert!(svg.contains("aria-hidden=\"true\""));
    assert!(html.has(&format!("--pk-rail-gutter: {}px;", g.gutter)));
    assert_eq!(html.with_class("line", "pk-rail-trunk").len(), 1);
    assert_eq!(
        html.with_class("line", "pk-rail-lane").len(),
        3,
        "one line per lane"
    );
    // omp and bob merge back, feat/x stays open.
    assert_eq!(html.with_class("path", "fork").len(), 3);
    assert_eq!(html.with_class("path", "merge").len(), 2);
    assert_eq!(html.with_class("circle", "pk-rail-dot").len(), 8);
    for edge in &g.item.edges {
        assert!(html.has(&format!("d=\"{}\"", edge.d)), "{}", edge.d);
    }
    let e2 = &g.item.rows.iter().find(|r| r.id == "e2").unwrap();
    assert!(html.has(&format!("cx=\"{}\" cy=\"{}\"", e2.x, e2.y)));
}

#[test]
fn dots_carry_their_tone_class_and_data_tone() {
    let html = render(graph().view(&opts()));
    assert_eq!(dot(&html, "blue").len(), 2, "claimed, run started");
    assert_eq!(dot(&html, "green").len(), 1);
    assert_eq!(dot(&html, "yellow").len(), 1);
    assert_eq!(dot(&html, "red").len(), 1);
    assert_eq!(dot(&html, "hollow").len(), 3, "two notes and the marker");
    assert!(dot(&html, "red")[0].contains("data-tone=\"err\""));
    assert!(dot(&html, "hollow")[0].contains("data-tone=\"neutral\""));
    assert!(
        html.has("<title>e1 said") || html.has("<title>Oct 1 00:00"),
        "{}",
        html.0
    );
}

#[test]
fn trunk_tone_follows_the_item_and_done_fades_it() {
    let html = render(graph().view(&opts()));
    // claimed item: blue trunk; a merged PR on the item marks it done.
    let canvas = html.with_class("div", "pk-rail-canvas")[0];
    assert!(class_of(canvas).split(' ').any(|c| c == "blue"), "{canvas}");
    assert!(class_of(canvas).split(' ').any(|c| c == "done"), "{canvas}");
}

// -- chips --------------------------------------------------------------------

#[test]
fn chip_sits_on_each_lanes_first_row_only() {
    let html = render(graph().view(&opts()));
    assert_eq!(html.count("class=\"pk-rail-chip\""), 2, "{}", html.0);
    assert!(html
        .row("e3")
        .contains("<span class=\"pk-rail-chip\">[OMP]</span>"));
    assert!(html
        .row("e4")
        .contains("<span class=\"pk-rail-chip\">[BOB]</span>"));
    assert!(!html.row("e5").contains("pk-rail-chip"), "later omp row");
    assert!(!html.row("e1").contains("pk-rail-chip"), "trunk");
}

#[test]
fn lane_marker_row_is_static_and_chipless() {
    let html = render(graph().view(&opts()));
    let marker = html.with_class("li", "marker");
    assert_eq!(marker.len(), 1);
    assert!(marker[0].contains("data-kind=\"lane\""));
    let whole = html.row("lane:l/feat");
    assert!(whole.contains("branch feat/x"));
    assert!(!whole.contains("pk-rail-chip"));
    assert!(!whole.contains("<button"));
}

// -- separators ---------------------------------------------------------------

#[test]
fn separators_mark_every_row_only_when_on() {
    let on = render(graph().view(&RailOptions {
        separators: true,
        ..opts()
    }));
    assert_eq!(on.with_class("li", "sep").len(), 8);
    let off = render(graph().view(&opts()));
    assert_eq!(off.with_class("li", "sep").len(), 0);
    assert!(CSS.contains(".pk-rail-row.sep"));
}

// -- selection ----------------------------------------------------------------

#[test]
fn rows_are_static_spans_without_on_select() {
    let html = render(graph().view(&opts()));
    assert_eq!(html.count("<button"), 0);
    assert_eq!(html.with_class("span", "pk-rail-hit").len(), 8);
    assert_eq!(html.count("aria-pressed"), 0);
}

#[test]
fn with_on_select_every_row_is_a_disclosure_button() {
    let html = render(graph().view(&RailOptions {
        on_select: true,
        ..opts()
    }));
    let expanded: Vec<_> = html
        .tags("button")
        .into_iter()
        .filter(|b| b.contains("aria-expanded"))
        .collect();
    assert_eq!(expanded.len(), 8, "7 events + the marker are all toggleable");
    assert!(expanded.iter().all(|b| b.contains("type=\"button\"")));
    assert!(expanded
        .iter()
        .all(|b| b.contains("aria-controls=\"rail-detail-")));
    assert!(
        !html.0.contains("aria-pressed"),
        "selection is no longer a pressed state"
    );
    assert_eq!(html.with_class("li", "on").len(), 0);
    // the heading gains the two quiet actions
    let heading: Vec<_> = html
        .tags("button")
        .into_iter()
        .filter(|b| b.contains("pk-dt-btn"))
        .collect();
    assert_eq!(heading.len(), 2, "expand all and collapse all");
    assert!(html.tags("button").len() == 10, "8 rows + 2 heading actions");
}

#[test]
fn the_selected_row_row_hit_and_dot_all_read_as_on() {
    let html = render(graph().view(&RailOptions {
        on_select: true,
        selected: Some("e2".to_owned()),
        ..opts()
    }));
    let row = html.row("e2");
    assert!(row.contains("class=\"pk-rail-hit on\""), "{row}");
    assert!(row.contains("aria-expanded=\"false\""), "{row}");
    assert!(
        row.starts_with("<li") && class_of(row).split(' ').any(|c| c == "on"),
        "{row}"
    );
    assert_eq!(html.with_class("li", "on").len(), 1);
    let on_dots = html.with_class("circle", "on");
    assert_eq!(on_dots.len(), 1);
    assert!(on_dots[0].contains("blue"));
    // markers are toggleable but never take the selection
    assert!(!html.row("lane:l/feat").contains("pk-rail-hit on"));
}

#[test]
fn selection_is_ignored_on_a_rail_without_on_select() {
    let html = render(graph().view(&RailOptions {
        selected: Some("e2".to_owned()),
        ..opts()
    }));
    assert_eq!(html.with_class("li", "on").len(), 0);
    assert_eq!(html.with_class("circle", "on").len(), 0);
}

fn item() -> ItemRef {
    ItemRef {
        id: "imp-16".to_owned(),
        status: Some("claimed".to_owned()),
    }
}

fn open_only(ids: &[&str]) -> Expansion {
    let mut open = Expansion::new();
    open.open_all(ids.iter().copied());
    open
}

#[test]
fn a_closed_rail_renders_its_rows_byte_for_byte_unchanged() {
    // static rows: the pre-expansion markup, byte for byte
    let html = render(graph().view(&opts()));
    assert_eq!(
        html.row("e1"),
        concat!(
            "<li id=\"rail-row-e1\" class=\"pk-rail-row\" data-kind=\"note\">",
            "<span class=\"pk-rail-hit\" title=\"Oct 1 00:00 · note · clara · e1 said\">",
            "<time class=\"pk-rail-time\" datetime=\"2026-10-01T00:00:00Z\">00:00</time>",
            "<span class=\"pk-rail-label\">e1 said</span></span>"
        )
    );
    // interactive rows: closed disclosure buttons with a quiet caret
    let live = render(graph().view(&RailOptions {
        on_select: true,
        ..opts()
    }));
    assert_eq!(
        live.row("e1"),
        concat!(
            "<li id=\"rail-row-e1\" class=\"pk-rail-row\" data-kind=\"note\">",
            "<button class=\"pk-rail-hit\" type=\"button\" title=\"Oct 1 00:00 · note · clara · e1 said\" aria-expanded=\"false\" aria-controls=\"rail-detail-e1\">",
            "<time class=\"pk-rail-time\" datetime=\"2026-10-01T00:00:00Z\">00:00</time>",
            "<span class=\"pk-rail-caret\" aria-hidden=\"true\">▸</span>",
            "<span class=\"pk-rail-label\">e1 said</span></button>"
        )
    );
    assert!(!live.0.contains("pk-rail-detail"));
}

fn open_graph(ids: &[&str]) -> VerticalGraph {
    vertical_with(&events(), &item(), &tree(), &open_only(ids), 56)
}

#[test]
fn an_open_row_lists_its_facts_as_sibling_rows_and_shifts_the_rest() {
    let g = open_graph(&["e1"]);
    let html = render(g.view(&RailOptions {
        on_select: true,
        ..opts()
    }));
    // the fact block sits between its row and the next row
    let row_at = html.0.find("id=\"rail-row-e1\"").unwrap();
    let detail_at = html.0.find("id=\"rail-detail-e1\"").unwrap();
    let next_at = html.0.find("id=\"rail-row-e2\"").unwrap();
    assert!(row_at < detail_at && detail_at < next_at, "{}", html.0);
    let lines = line_count(&event_facts(&events()[0], 56));
    assert_eq!(html.count("pk-rail-fact-line"), lines);
    assert!(html.0.contains("aria-label=\"e1 said details\""));
    // the row reads as an expanded disclosure: open caret, blue frame class
    let row = html.row("e1");
    assert!(row.contains("aria-expanded=\"true\""), "{row}");
    assert!(row.contains("aria-controls=\"rail-detail-e1\""), "{row}");
    assert!(row.contains("\u{25be}"), "{row}");
    assert!(class_of(row).split(' ').any(|c| c == "open"), "{row}");
    // other rows stay closed with their own detail id
    let closed = html.row("e2");
    assert!(closed.contains("aria-expanded=\"false\""), "{closed}");
    assert!(closed.contains("\u{25b8}"), "{closed}");
    assert!(!html.0.contains("id=\"rail-detail-e2\""));
    // the gutter follows the shifted geometry
    let e2 = g.item.rows.iter().find(|r| r.id == "e2").unwrap();
    assert!(html.has(&format!("cy=\"{}\"", e2.y)));
    assert!(html.tags("svg")[0].contains(&format!("height=\"{}\"", g.height)));
}

#[test]
fn an_open_lane_marker_shows_its_lane_facts() {
    let g = open_graph(&["lane:l/feat"]);
    let html = render(g.view(&RailOptions {
        on_select: true,
        ..opts()
    }));
    let tree = tree();
    let feat = tree.lanes.iter().find(|l| l.id == "l/feat").unwrap();
    assert_eq!(
        html.count("pk-rail-fact-line"),
        line_count(&lane_facts(feat, 56))
    );
    assert!(html.0.contains("id=\"rail-detail-lane:l/feat\""));
    let row = html.row("lane:l/feat");
    assert!(row.contains("aria-expanded=\"true\""), "{row}");
    assert!(row.contains("aria-controls=\"rail-detail-lane:l/feat\""));
}

fn heading_buttons(html: &Html) -> Vec<&str> {
    html.tags("button")
        .into_iter()
        .filter(|b| b.contains("pk-dt-btn"))
        .collect()
}

#[test]
fn expand_and_collapse_disable_when_there_is_nothing_to_do() {
    let none = render(graph().view(&RailOptions {
        on_select: true,
        ..opts()
    }));
    let h = heading_buttons(&none);
    assert_eq!(h.len(), 2);
    assert!(!h[0].contains("disabled"), "something to expand: {:?}", h);
    assert!(h[1].contains("disabled=true"), "nothing to collapse: {:?}", h);

    let ids = ["e1", "e2", "e3", "e4", "e5", "e6", "e7", "lane:l/feat"];
    let all = render_with(
        open_graph(&ids).view(&RailOptions {
            on_select: true,
            ..opts()
        }),
        &open_only(&ids),
    );
    let h = heading_buttons(&all);
    assert_eq!(h.len(), 2);
    assert!(h[0].contains("disabled=true"), "nothing to expand: {:?}", h);
    assert!(!h[1].contains("disabled"), "something to collapse: {:?}", h);

    // a rail without on_select has no actions at all
    assert!(heading_buttons(&render(graph().view(&opts()))).is_empty());
}

#[test]
fn selected_state_has_its_own_rules_beyond_hue() {
    for selector in [".pk-rail-hit.on", ".pk-rail-dot.on"] {
        let at = CSS
            .find(selector)
            .unwrap_or_else(|| panic!("no {selector}"));
        let rule = &CSS[at..at + CSS[at..].find('}').unwrap()];
        assert!(
            rule.contains("outline")
                || rule.contains("stroke-width")
                || rule.contains("box-shadow"),
            "{selector} must change shape: {rule}"
        );
    }
}

// -- dead lanes ---------------------------------------------------------------

#[test]
fn dead_lanes_dim_their_rows_lines_elbows_and_dots() {
    let html = render(graph().view(&opts()));
    assert!(class_of(html.row("e4")).split(' ').any(|c| c == "dead"));
    assert!(class_of(html.row("lane:l/feat"))
        .split(' ')
        .any(|c| c == "dead"));
    assert!(!class_of(html.row("e3")).split(' ').any(|c| c == "dead"));
    assert_eq!(html.with_class("li", "dead").len(), 2);
    assert_eq!(
        html.with_class("line", "dead").len(),
        2,
        "bob and feat/x lines"
    );
    assert_eq!(
        html.with_class("path", "dead").len(),
        3,
        "bob fork+merge, feat/x fork"
    );
    assert_eq!(html.with_class("circle", "dead").len(), 2);
}

// -- empty state --------------------------------------------------------------

#[test]
fn a_rail_without_rows_says_so_and_draws_no_gutter() {
    let g = vertical(
        &[],
        &ItemRef {
            id: "imp-16".to_owned(),
            status: None,
        },
        &SessionTree {
            agent: Some("clara".to_owned()),
            ..SessionTree::default()
        },
    );
    let html = render(g.view(&opts()));
    assert!(
        html.has("<p class=\"pk-rail-empty\" id=\"rail-empty\">nothing recorded yet</p>"),
        "{}",
        html.0
    );
    assert!(html.has("<span class=\"pk-rail-title\">session tree</span>"));
    assert!(!html.has("pk-rail-meta"));
    assert!(!html.has("<svg"));
    assert!(!html.has("<ol"));
}

// -- safety and stylesheet ----------------------------------------------------

/// Text nodes only: dioxus-ssr 0.6 writes attribute values unescaped, while
/// the web renderer sets attributes through the DOM.
#[test]
fn hostile_text_is_escaped_and_no_script_is_emitted() {
    let sneaky = ev("s1", 1, "clara", EventKind::Note).summary("<script>alert(1)</script>");
    let g = vertical(
        &[sneaky],
        &ItemRef {
            id: "imp-16".to_owned(),
            status: None,
        },
        &SessionTree::default(),
    );
    let html = render(g.view(&opts()));
    assert!(
        html.has(">&lt;script&gt;alert(1)&lt;/script&gt;</span>"),
        "{}",
        html.0
    );
    // The tooltip attribute is the one place SSR leaves raw.
    let sans_title = html.0.replace(
        " title=\"Oct 1 00:01 · note · clara · <script>alert(1)</script>\"",
        "",
    );
    assert!(!sans_title.contains("<script"), "{sans_title}");
    assert!(!render(graph().view(&opts())).has("<script"));
}

#[test]
fn stylesheet_covers_every_class_the_painter_emits_and_only_known_variables() {
    let closed = render(graph().view(&RailOptions {
        on_select: true,
        selected: Some("e2".to_owned()),
        separators: true,
        ..opts()
    }));
    let open = render(open_graph(&["e1", "lane:l/feat"]).view(&RailOptions {
        on_select: true,
        ..opts()
    }));
    let css = CSS
        .split("/* ── Event graph: rails")
        .nth(1)
        .and_then(|rest| rest.split("/* ── Event graph: swimlane").next())
        .expect("the rail rules sit between the event graph section markers");
    let mut tokens: Vec<String> = Vec::new();
    for html in [&closed, &open] {
        for (at, _) in html.0.match_indices("class=\"") {
            let rest = &html.0[at + 7..];
            tokens.extend(
                rest[..rest.find('"').unwrap()]
                    .split(' ')
                    .map(str::to_owned),
            );
        }
    }
    tokens.sort();
    tokens.dedup();
    for token in tokens
        .iter()
        .filter(|t| !matches!(t.as_str(), "pk-widget" | "fork" | "merge"))
    {
        assert!(css.contains(&format!(".{token}")), "no rule for .{token}");
    }
    let allowed = [
        "--blue",
        "--green",
        "--yellow",
        "--red",
        "--fg",
        "--dim",
        "--line",
        "--line2",
        "--bg",
        "--panel",
        "--mono",
        "--body-size",
        "--body-line-height",
        "--label-size",
        "--label-tracking",
        "--pk-rail-row",
        "--pk-rail-labels",
        "--pk-rail-gutter",
    ];
    for (at, _) in css.match_indices("var(") {
        let name = css[at + 4..].split(')').next().unwrap();
        assert!(allowed.contains(&name), "unexpected variable {name}");
    }
    assert!(css.contains("text-overflow: ellipsis"));
}
