//! The SVG canvas of the GRAPH mode: the density strip, one row per item with
//! its dots, forks and labels, and the axis, over a time-warped backdrop.
//!
//! The layout arrives with every coordinate resolved; this only emits DOM.
//! The canvas is three SVGs of one coordinate system: the strip pinned to the
//! top of the scroller, the axis pinned to the bottom, and between them the
//! item rows, of which only the ones near the viewport are mounted.
//! A dot or a fork leaf opens its own node in the inspector and stops the click
//! there; any other click on an item reaches the item's row, which opens the
//! item. The rows are the only keyboard stops.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::widgets::trace::query::GraphAction;
use panel_kit_core::widgets::trace::swimlane::metrics::{label_px, FORK_NODE_PX, ROW};
use panel_kit_core::widgets::trace::swimlane::{
    fmt1, slug, Dot, Fork, Graph, Item, LooseRow, Node, StripRow, TickKind,
};
use panel_kit_core::widgets::trace::Event;

use super::shared::Shared;
use super::window;

/// Side of a fork's tip node.
const NODE: f64 = 7.0;

/// A real SVG `<title>`, escaped. An rsx `title` element is created without
/// the SVG namespace in the browser and never shows a tooltip, so the title
/// goes in as the element's `innerHTML`.
pub(super) fn tip(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    out.push_str("<title>");
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out.push_str("</title>");
    out
}

/// How far under the axis line a tick's label sits: day labels get the lower row.
fn label_drop(kind: TickKind) -> f64 {
    match kind {
        TickKind::Day => 23.0,
        TickKind::Time => 11.0,
    }
}

/// Enter and Space press a row.
fn presses(event: &KeyboardEvent) -> bool {
    match event.key() {
        Key::Enter => true,
        Key::Character(c) => c == " ",
        _ => false,
    }
}

/// One dot: a ring or disc for an event, a numbered bubble for a burst.
fn glyph(dot: &Dot, y: f64, on_pick: EventHandler<Node>) -> Element {
    let name = dot.tone.name();
    let (cx, cy) = (fmt1(dot.x), fmt1(y));
    let node = if dot.is_cluster() {
        Node::Cluster(dot.members.clone())
    } else {
        Node::Event(dot.id.clone())
    };
    let pick = move |event: MouseEvent| {
        event.stop_propagation();
        on_pick.call(node.clone());
    };
    if dot.is_cluster() {
        rsx! {
            g {
                class: "pk-tr-cluster pk-tr-glyph pk-tr-{name}",
                "data-count": "{dot.count}",
                onclick: pick,
                circle {
                    class: "pk-tr-dot pk-tr-{name}",
                    cx: "{cx}",
                    cy: "{cy}",
                    r: "5",
                    dangerous_inner_html: "{tip(&dot.cluster_title())}",
                }
                text {
                    class: "pk-tr-cluster-count",
                    x: "{cx}",
                    y: "{cy}",
                    text_anchor: "middle",
                    dominant_baseline: "central",
                    "{dot.count}"
                }
            }
        }
    } else {
        rsx! {
            circle {
                class: "pk-tr-dot pk-tr-glyph pk-tr-{name}",
                onclick: pick,
                cx: "{cx}",
                cy: "{cy}",
                r: "3",
                dangerous_inner_html: "{tip(&dot.title)}",
            }
        }
    }
}

/// A fork lane under its item: its line, a square tip node and the caption.
fn fork_row(fork: &Fork, on_pick: EventHandler<Node>) -> Element {
    let name = fork.tone.name();
    let y = fmt1(fork.y);
    let node_x = fork.node_left();
    let caption_x = fork.x1 + FORK_NODE_PX;
    let hit_x = node_x - 2.0;
    let hit_w = caption_x + label_px(&fork.label) + 4.0 - hit_x;
    let lane = fork.id.clone();
    rsx! {
        g {
            class: if fork.dead { "pk-tr-fork pk-tr-dead" } else { "pk-tr-fork" },
            "data-kind": "{fork.kind.as_str()}",
            onclick: move |event| {
                event.stop_propagation();
                on_pick.call(Node::Lane(lane.clone()));
            },
            rect {
                class: "pk-tr-hit",
                x: "{fmt1(hit_x)}",
                y: "{fmt1(fork.y - 6.0)}",
                width: "{fmt1(hit_w)}",
                height: "12",
                dangerous_inner_html: "{tip(&fork.title)}",
            }
            line {
                class: "pk-tr-line pk-tr-fork-line",
                x1: "{fmt1(fork.x0)}",
                x2: "{fmt1(fork.x1)}",
                y1: "{y}",
                y2: "{y}",
            }
            rect {
                class: "pk-tr-fork-node pk-tr-{name}",
                x: "{fmt1(node_x)}",
                y: "{fmt1(fork.y - NODE / 2.0)}",
                width: "{NODE}",
                height: "{NODE}",
            }
            text {
                class: "pk-tr-fork-label",
                x: "{fmt1(caption_x)}",
                y: "{y}",
                dominant_baseline: "central",
                "{fork.label}"
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    /// How many times `ItemRow` has rendered on this thread.
    static ROW_RENDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// One item as a button: its line, dots, forks and elbows, and its id at the
/// tip (and at the start of a long line). A component of its own, so picking
/// an item re-renders two rows, not all of them.
#[component]
fn ItemRow(
    id: String,
    item: Shared<Item>,
    picked: bool,
    on_select: EventHandler<String>,
    on_open: EventHandler<Node>,
) -> Element {
    #[cfg(test)]
    ROW_RENDERS.with(|renders| renders.set(renders.get() + 1));
    let item: &Item = &item;
    let name = item.tone.name();
    let mut class = format!("pk-tr-item pk-tr-{name}");
    if item.done {
        class.push_str(" pk-tr-done");
    }
    if picked {
        class.push_str(" on");
    }
    let tooltip = item.tooltip();
    let hit = item.hit_rect();
    let y = fmt1(item.y);
    let (click, press) = (item.id.clone(), item.id.clone());
    let pick = {
        let owner = item.id.clone();
        move |node: Node| {
            on_select.call(owner.clone());
            on_open.call(node);
        }
    };
    let pick = use_callback(pick);
    let (click_node, press_node) = (Node::Item(click.clone()), Node::Item(press.clone()));
    rsx! {
        g {
            id: "{id}-graph-{slug(&item.id)}",
            class: "{class}",
            role: "button",
            tabindex: "0",
            "aria-pressed": "{picked}",
            "aria-label": "{tooltip}",
            "data-id": "{item.id}",
            onclick: move |_| {
                on_select.call(click.clone());
                on_open.call(click_node.clone());
            },
            onkeydown: move |event| {
                if presses(&event) {
                    event.prevent_default();
                    on_select.call(press.clone());
                    on_open.call(press_node.clone());
                }
            },
            rect {
                class: "pk-tr-hit",
                x: "{fmt1(hit.x)}",
                y: "{fmt1(hit.y)}",
                width: "{fmt1(hit.w)}",
                height: "{fmt1(hit.h)}",
                dangerous_inner_html: "{tip(&tooltip)}",
            }
            line {
                class: "pk-tr-line",
                x1: "{fmt1(item.x0)}",
                x2: "{fmt1(item.x1)}",
                y1: "{y}",
                y2: "{y}",
            }
            for dot in item.dots.iter() {
                {glyph(dot, item.y, pick)}
            }
            for fork in item.forks.iter() {
                {fork_row(fork, pick)}
            }
            for edge in item.edges.iter() {
                path {
                    class: if edge.dead {
                        format!("pk-tr-edge pk-tr-edge-{} pk-tr-dead", edge.kind.as_str())
                    } else {
                        format!("pk-tr-edge pk-tr-edge-{}", edge.kind.as_str())
                    },
                    d: "{edge.d}",
                }
            }
            if let Some(start) = item.start_label_x {
                text {
                    class: "pk-tr-label pk-tr-label-start",
                    x: "{fmt1(start)}",
                    y: "{y}",
                    text_anchor: "end",
                    dominant_baseline: "central",
                    "{item.id}"
                }
            }
            text {
                class: "pk-tr-label",
                x: "{fmt1(item.label_x)}",
                y: "{y}",
                dominant_baseline: "central",
                "{item.id}"
            }
        }
    }
}

/// A closed no-item row: a bar of density cells, one click from open.
fn strip_row(
    id: &str,
    row: &StripRow,
    width: u32,
    height: f64,
    on_action: EventHandler<GraphAction>,
) -> Element {
    let kind = row.kind.as_str();
    rsx! {
        g {
            key: "{kind}",
            id: "{id}-strip-row-{kind}",
            class: "pk-tr-strip",
            onclick: move |_| on_action.call(GraphAction::ToggleStripKind(kind.to_string())),
            rect {
                class: "pk-tr-hit",
                x: "0",
                y: "{fmt1(row.y0)}",
                width: "{width}",
                height: "{fmt1(height)}",
                dangerous_inner_html: "{tip(&row.title())}",
            }
            for cell in row.cells.iter() {
                rect {
                    x: "{cell.x}",
                    y: "{fmt1(row.y0 + 2.0)}",
                    width: "{cell.w}",
                    height: "{fmt1(height - 4.0)}",
                    fill_opacity: "{cell.alpha}",
                }
            }
        }
    }
}

/// An open no-item row: a dashed line with its events as dots on it.
fn loose_row(
    row: &LooseRow,
    width: u32,
    height: f64,
    on_action: EventHandler<GraphAction>,
    on_open: EventHandler<Node>,
) -> Element {
    let kind = row.kind.as_str();
    let y = fmt1(row.y);
    let title = format!(
        "{} {} events with no work item",
        row.count,
        row.kind.label()
    );
    rsx! {
        g {
            key: "{kind}",
            class: "pk-tr-loose",
            onclick: move |_| on_action.call(GraphAction::ToggleStripKind(kind.to_string())),
            rect {
                class: "pk-tr-hit",
                x: "0",
                y: "{fmt1(row.y - height / 2.0)}",
                width: "{width}",
                height: "{fmt1(height)}",
                dangerous_inner_html: "{tip(&title)}",
            }
            line { class: "pk-tr-loose-line", x1: "0", x2: "{width}", y1: "{y}", y2: "{y}" }
            for dot in row.dots.iter() {
                {glyph(dot, row.y, on_open)}
            }
        }
    }
}

/// Where the item rows begin: under the no-item strip rows.
fn strip_height(layout: &Graph) -> f64 {
    layout.strip.len() as f64 * ROW
}

/// The items of the mounted rows, kept between renders: a row that stays
/// mounted keeps its handle, so it is not rendered again, and an item is
/// built once however far the window moves in small steps.
#[derive(Default)]
struct ItemCache {
    graph: Option<Shared<Graph>>,
    items: HashMap<u32, Shared<Item>>,
}

impl ItemCache {
    /// The items with a row in `y0..y1` graph px, top row first.
    fn window(
        &mut self,
        graph: &Shared<Graph>,
        events: &[Event],
        y0: f64,
        y1: f64,
    ) -> Vec<Shared<Item>> {
        if self.graph.as_ref() != Some(graph) {
            self.graph = Some(graph.clone());
            self.items.clear();
        }
        let spans = &graph.row_spans()[graph.span(y0, y1)];
        let mut kept = HashMap::with_capacity(spans.len());
        let rows = spans
            .iter()
            .map(|span| {
                let item = self.items.remove(&span.item).unwrap_or_else(|| {
                    Shared::new(graph.item(events, span.item).into_owned())
                });
                kept.insert(span.item, item.clone());
                item
            })
            .collect();
        self.items = kept;
        rows
    }
}

/// Grid lines and idle hatching over the pixel rows `from..to`. Every part of
/// the canvas draws its own stretch at the same x, so the parts read as one.
/// The axis part (`foot`) carries the day lines only: time lines stop above it.
fn backdrop(id: &str, layout: &Graph, from: f64, to: f64, foot: bool) -> Element {
    let hatch = format!("{id}-hatch");
    let (y, end) = (fmt1(from), fmt1(to));
    rsx! {
        if !foot {
            g { class: "pk-tr-gaps", "aria-hidden": "true",
                for (at , gap) in layout.gaps.iter().enumerate() {
                    rect {
                        key: "{at}",
                        class: "pk-tr-gap",
                        x: "{fmt1(gap.x0)}",
                        y: "{y}",
                        width: "{fmt1(gap.x1 - gap.x0)}",
                        height: "{fmt1(to - from)}",
                        fill: "url(#{hatch})",
                        dangerous_inner_html: "{tip(&gap.title())}",
                    }
                }
            }
        }
        g { class: "pk-tr-grid", "aria-hidden": "true",
            for (at , tick) in layout.ticks.iter().enumerate().filter(|(_, tick)| !foot || tick.kind == TickKind::Day) {
                line {
                    key: "{at}",
                    class: "pk-tr-tick pk-tr-tick-{tick.kind.as_str()}",
                    x1: "{fmt1(tick.x)}",
                    x2: "{fmt1(tick.x)}",
                    y1: "{y}",
                    y2: "{end}",
                }
            }
        }
    }
}

/// The no-item strip: its closed rows as density bars and its open rows as
/// dotted lines. Pinned to the top of the scroller.
#[component]
fn Top(
    id: String,
    graph: Shared<Graph>,
    on_action: EventHandler<GraphAction>,
    on_open: EventHandler<Node>,
) -> Element {
    let layout: &Graph = &graph;
    let (width, height) = (layout.width, strip_height(layout));
    let row_h = layout.loose_y.unwrap_or(7.0) * 2.0;
    rsx! {
        svg {
            class: "pk-tr-svg pk-tr-top",
            width: "{width}",
            height: "{height}",
            view_box: "0 0 {width} {height}",
            role: "group",
            "aria-label": "events with no work item",
            {backdrop(&id, layout, 0.0, height, false)}
            for row in layout.strip.iter().filter(|row| !row.open) {
                {strip_row(&id, row, width, row_h, on_action)}
            }
            for row in layout.loose.iter() {
                {loose_row(row, width, row_h, on_action, on_open)}
            }
        }
    }
}

/// The time axis: day lines and the tick labels. Pinned to the bottom.
#[component]
fn Foot(graph: Shared<Graph>) -> Element {
    let layout: &Graph = &graph;
    let (width, axis_y) = (layout.width, layout.axis_y);
    let height = layout.height as f64 - axis_y;
    rsx! {
        svg {
            class: "pk-tr-svg pk-tr-foot",
            width: "{width}",
            height: "{height}",
            view_box: "0 {fmt1(axis_y)} {width} {height}",
            "aria-hidden": "true",
            {backdrop("", layout, axis_y, layout.height as f64, true)}
            g { class: "pk-tr-axis", "aria-hidden": "true",
                for (at , tick) in layout.ticks.iter().enumerate() {
                    text {
                        key: "{at}",
                        class: "pk-tr-tick-label pk-tr-tick-label-{tick.kind.as_str()}",
                        x: "{fmt1(tick.x + 3.0)}",
                        y: "{fmt1(axis_y + label_drop(tick.kind))}",
                        "{tick.label}"
                    }
                }
            }
        }
    }
}

/// The item rows: an SVG as tall as all of them with only `start..end` of its
/// pixel rows mounted. Props compare by identity, so a render that moves
/// neither the layout, the window nor the selection costs a pointer test.
#[component]
fn Rows(
    id: String,
    graph: Shared<Graph>,
    events: Shared<Vec<Event>>,
    start: usize,
    end: usize,
    selected: Option<String>,
    on_select: EventHandler<String>,
    on_open: EventHandler<Node>,
) -> Element {
    let cache = use_hook(|| Rc::new(RefCell::new(ItemCache::default())));
    let layout: &Graph = &graph;
    let (width, top) = (layout.width, strip_height(layout));
    let height = layout.axis_y - top;
    let hatch = format!("{id}-hatch");
    let mounted = cache.borrow_mut().window(
        &graph,
        &events,
        top + start as f64 * ROW,
        top + end as f64 * ROW,
    );
    rsx! {
        svg {
            class: "pk-tr-svg",
            width: "{width}",
            height: "{height}",
            view_box: "0 {fmt1(top)} {width} {height}",
            role: "group",
            "aria-label": "{layout.aria_label()}",
            "data-start": "{start}",
            "data-end": "{end}",
            defs {
                pattern {
                    id: "{hatch}",
                    width: "4",
                    height: "4",
                    pattern_units: "userSpaceOnUse",
                    pattern_transform: "rotate(45)",
                    line { class: "pk-tr-hatch-line", x1: "0", y1: "0", x2: "0", y2: "4" }
                }
            }
            {backdrop(&id, layout, top, layout.axis_y, false)}
            for item in mounted {
                ItemRow {
                    key: "{item.id}",
                    id: id.clone(),
                    picked: selected.as_deref() == Some(item.id.as_str()),
                    item: item.clone(),
                    on_select,
                    on_open,
                }
            }
        }
    }
}

/// The canvas of one layout, windowed by the scroller's `(scrolled, height)`.
/// The window moves only when `scroll` does, which the scroller reports when
/// the rows on screen leave the mounted ones. `events` is the slice the
/// layout was made from.
#[component]
pub(super) fn Canvas(
    id: String,
    graph: Shared<Graph>,
    events: Shared<Vec<Event>>,
    scroll: Signal<(f64, f64)>,
    selected: Option<String>,
    on_action: EventHandler<GraphAction>,
    on_select: EventHandler<String>,
    on_open: EventHandler<Node>,
) -> Element {
    let (scrolled, height) = scroll();
    let (start, end) = window::mounted(graph.item_count(), scrolled, height, ROW, graph.rows);
    rsx! {
        if !graph.strip.is_empty() {
            Top { id: id.clone(), graph: graph.clone(), on_action, on_open }
        }
        Rows {
            id: id.clone(),
            graph: graph.clone(),
            events,
            start,
            end,
            selected,
            on_select,
            on_open,
        }
        Foot { graph: graph.clone() }
    }
}

#[cfg(test)]
mod tests {
    use dioxus::dioxus_core::NoOpMutations;
    use panel_kit_core::widgets::trace::query::GraphState;
    use panel_kit_core::widgets::trace::swimlane::view;
    use panel_kit_core::widgets::trace::EventKind;

    use super::*;

    /// 2026-10-08T00:00:00Z, the end of the week the events fall in.
    const NOW: i64 = 1_791_417_600;

    /// `n` items of two notes a hundred hours apart: each line crosses the idle
    /// stretch between the two bursts, so no two share a row and the graph is
    /// `n` rows tall.
    fn events(n: usize) -> Vec<Event> {
        let since = (NOW - 7 * 86_400) * 1_000_000;
        (0..n)
            .flat_map(|at| {
                let id = format!("imp-{at}");
                let first = since + (at as i64 % 60) * 3_600_000_000;
                [0, 100].map(|hours| {
                    Event::new(
                        format!("{id}/{hours}"),
                        first + hours * 3_600_000_000,
                        EventKind::Note,
                    )
                    .agent("worker")
                    .item(&id)
                })
            })
            .collect()
    }

    /// The show-all layout of `n` such items, built lazily, and its events.
    fn layout(n: usize) -> (Shared<Graph>, Shared<Vec<Event>>) {
        let state = GraphState {
            window: "7d".to_string(),
            graph_width: 1400,
            show_all: true,
            ..GraphState::default()
        };
        let events = events(n);
        let graph = view::graph(&events, &state, None, NOW);
        assert!(graph.items.is_empty(), "show all is laid out lazily");
        (Shared::new(graph), Shared::new(events))
    }

    /// What a mounted probe lets the test move: the scroller and the selection.
    #[derive(Clone, Copy)]
    struct Signals {
        scroll: Signal<(f64, f64)>,
        selected: Signal<Option<String>>,
    }

    /// The signals of a mounted probe.
    #[derive(Clone)]
    struct Handle(Rc<RefCell<Option<Signals>>>);

    impl PartialEq for Handle {
        fn eq(&self, _: &Self) -> bool {
            false
        }
    }

    #[component]
    fn Probe(
        graph: Shared<Graph>,
        events: Shared<Vec<Event>>,
        selected: Option<String>,
        handle: Handle,
    ) -> Element {
        let scroll = use_signal(|| (0.0, window::DEFAULT_VIEWPORT));
        let picked = use_signal(|| selected.clone());
        *handle.0.borrow_mut() = Some(Signals {
            scroll,
            selected: picked,
        });
        // Handles outlive a render, as the host's do: closures made here would
        // differ every time and every row would render again.
        let on_action = use_callback(|_: GraphAction| {});
        let on_select = use_callback(|_: String| {});
        let on_open = use_callback(|_: Node| {});
        rsx! {
            Canvas {
                id: "t".to_string(),
                graph,
                events,
                scroll,
                selected: picked(),
                on_action,
                on_select,
                on_open,
            }
        }
    }

    /// A canvas over a layout and the way to scroll and pick in it.
    struct Rig {
        dom: VirtualDom,
        handle: Handle,
    }

    impl Rig {
        fn new(layout: &(Shared<Graph>, Shared<Vec<Event>>), selected: Option<&str>) -> Self {
            let handle = Handle(Rc::new(RefCell::new(None)));
            let props = ProbeProps {
                graph: layout.0.clone(),
                events: layout.1.clone(),
                selected: selected.map(str::to_string),
                handle: handle.clone(),
            };
            let mut dom = VirtualDom::new_with_props(Probe, props);
            dom.rebuild_in_place();
            Rig { dom, handle }
        }

        fn html(&self) -> String {
            dioxus_ssr::render(&self.dom)
        }

        fn signals(&self) -> Signals {
            self.handle.0.borrow().expect("the canvas is mounted")
        }

        /// The markup after the scroller reports `top` px scrolled.
        fn scrolled(&mut self, top: f64) -> String {
            let mut signal = self.signals().scroll;
            self.dom
                .in_runtime(|| signal.set((top, window::DEFAULT_VIEWPORT)));
            self.dom.render_immediate(&mut NoOpMutations);
            self.html()
        }

        /// Picks `item`, or nothing, and renders.
        fn pick(&mut self, item: Option<&str>) {
            let mut signal = self.signals().selected;
            self.dom
                .in_runtime(|| signal.set(item.map(str::to_string)));
            self.dom.render_immediate(&mut NoOpMutations);
        }
    }

    /// Rows rendered on this thread so far.
    fn renders() -> usize {
        ROW_RENDERS.with(std::cell::Cell::get)
    }

    fn mounted(html: &str) -> usize {
        html.matches("class=\"pk-tr-item").count()
    }

    fn has(html: &str, item: &str) -> bool {
        html.contains(&format!("id=\"t-graph-{}\"", slug(item)))
    }

    /// The ids of the first and the last row of the layout.
    fn ends((graph, events): &(Shared<Graph>, Shared<Vec<Event>>)) -> (String, String) {
        let spans = graph.row_spans();
        let id = |at: usize| graph.item(events, spans[at].item).id.clone();
        (id(0), id(spans.len() - 1))
    }

    /// The `data-start` and `data-end` of the rows svg.
    fn mounted_rows(html: &str) -> (usize, usize) {
        let read = |key: &str| {
            let at = html.find(key).unwrap() + key.len();
            html[at..at + html[at..].find('"').unwrap()].parse().unwrap()
        };
        (read("data-start=\""), read("data-end=\""))
    }

    #[test]
    fn the_first_screen_mounts_the_top_rows_and_not_the_bottom_one() {
        let layout = layout(400);
        let (first, last) = ends(&layout);
        let html = Rig::new(&layout, None).html();
        assert!(has(&html, &first), "the first row is mounted");
        assert!(!has(&html, &last), "the last row waits for the scroll");
        assert_eq!(mounted(&html), 30, "24 rows on screen and 6 of overscan");
        assert_eq!(mounted_rows(&html), (0, 30));
    }

    #[test]
    fn the_rows_svg_keeps_the_height_of_every_row_whichever_are_mounted() {
        let layout = layout(400);
        let height = format!("height=\"{}\"", layout.0.axis_y);
        let mut rig = Rig::new(&layout, None);
        assert!(rig.html().contains(&height));
        assert!(rig.scrolled(2_800.0).contains(&height));
        assert!(rig.scrolled(1e9).contains(&height));
        assert!(layout.0.axis_y > 400.0 * 14.0, "400 rows of 14px");
    }

    #[test]
    fn scrolling_mounts_the_rows_under_the_viewport_and_drops_the_ones_left_behind() {
        let layout = layout(400);
        let (first, last) = ends(&layout);
        let mut rig = Rig::new(&layout, None);
        let middle = rig.scrolled(2_800.0);
        assert!(!has(&middle, &first) && !has(&middle, &last));
        assert_eq!(mounted_rows(&middle), (194, 230));
        assert_eq!(mounted(&middle), 36, "24 rows on screen and 6 of overscan twice");
        let bottom = rig.scrolled(1e9);
        assert!(has(&bottom, &last), "the last row is mounted at the end");
        assert!(!has(&bottom, &first));
        assert!(mounted(&bottom) <= 30);
        let top = rig.scrolled(0.0);
        assert!(has(&top, &first) && !has(&top, &last));
    }

    #[test]
    fn the_selection_survives_scrolling_out_and_back_in() {
        let layout = layout(400);
        let (_, last) = ends(&layout);
        let mut rig = Rig::new(&layout, Some(&last));
        let top = rig.html();
        assert!(!has(&top, &last) && !top.contains("pk-tr-neutral on"));
        let bottom = rig.scrolled(1e9);
        let row = bottom
            .split("<g id=\"t-graph-")
            .find(|row| row.starts_with(&slug(&last)))
            .expect("the selected row is mounted at the end");
        assert!(row.contains("pk-tr-item pk-tr-neutral on"), "{row}");
        assert!(row.contains("aria-pressed=\"true\""), "{row}");
        assert_eq!(bottom.matches("aria-pressed=\"true\"").count(), 1);
        assert!(!rig.scrolled(0.0).contains("pk-tr-neutral on"));
        assert!(rig.scrolled(1e9).contains("pk-tr-item pk-tr-neutral on"));
    }

    #[test]
    fn a_small_layout_is_mounted_whole_whatever_the_scroll() {
        let layout = layout(window::WHOLE);
        let mut rig = Rig::new(&layout, None);
        assert_eq!(mounted(&rig.html()), window::WHOLE);
        assert_eq!(mounted(&rig.scrolled(1e9)), window::WHOLE);
    }

    #[test]
    fn one_item_more_than_the_whole_limit_starts_the_window() {
        let layout = layout(window::WHOLE + 1);
        assert_eq!(mounted(&Rig::new(&layout, None).html()), 30);
    }

    #[test]
    fn rows_that_stay_mounted_keep_their_handle_and_a_new_layout_drops_them() {
        let (graph, events) = layout(400);
        let top = strip_height(&graph);
        let mut cache = ItemCache::default();
        let before = cache.window(&graph, &events, top, top + 30.0 * ROW);
        let after = cache.window(&graph, &events, top + 10.0 * ROW, top + 40.0 * ROW);
        let kept = after.iter().filter(|row| before.contains(row)).count();
        assert!(kept >= 19, "rows 10 to 30 are in both windows: {kept}");
        assert!(after.len() <= 31, "{} rows", after.len());
        let (other, _) = layout(400);
        let fresh = cache.window(&other, &events, top, top + 30.0 * ROW);
        assert!(fresh.iter().all(|row| !before.contains(row)));
    }

    #[test]
    fn picking_a_row_renders_the_rows_it_changes_and_no_other() {
        let layout = layout(400);
        let spans = layout.0.row_spans();
        let id = |at: usize| layout.0.item(&layout.1, spans[at].item).id.clone();
        let (first, second) = (id(0), id(1));
        let mut rig = Rig::new(&layout, None);
        assert_eq!(renders(), 30, "every mounted row renders once at mount");
        rig.pick(Some(&first));
        assert_eq!(renders(), 31, "the row picked, and none of the other 29");
        rig.pick(Some(&second));
        assert_eq!(renders(), 33, "the row let go and the row picked");
        rig.pick(None);
        assert_eq!(renders(), 34, "the row let go");
        rig.pick(None);
        assert_eq!(renders(), 34, "nothing changed, nothing renders");
    }

    #[test]
    fn a_scroll_step_renders_the_rows_that_enter_the_window_and_no_other() {
        let layout = layout(400);
        let mut rig = Rig::new(&layout, None);
        assert_eq!(renders(), 30);
        // Ten rows down: the window goes from rows 0..30 to rows 4..40.
        let html = rig.scrolled(10.0 * ROW);
        assert_eq!(mounted_rows(&html), (4, 40));
        assert_eq!(renders(), 40, "the ten rows that entered, not the 26 that stayed");
    }
}
