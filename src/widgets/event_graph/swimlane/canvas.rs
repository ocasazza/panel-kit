//! The SVG canvas of the GRAPH mode: the time-warped backdrop, the density
//! strip, one row per item with its dots, forks and labels, and the axis.
//!
//! The layout arrives with every coordinate resolved; this only emits DOM.
//! Dots carry no handlers and no signals: a click on one reaches its item's
//! row, which is the one thing that listens.

use dioxus::prelude::*;
use panel_kit_core::widgets::event_graph::query::GraphAction;
use panel_kit_core::widgets::event_graph::swimlane::metrics::{label_px, FORK_NODE_PX};
use panel_kit_core::widgets::event_graph::swimlane::{
    fmt1, slug, Dot, Fork, Graph, LooseRow, StripRow, TickKind,
};

use super::shared::Shared;

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

/// Where a grid line ends: day lines run the full height, time lines stop at the axis.
fn tick_end(kind: TickKind, height: i64, axis_y: f64) -> String {
    match kind {
        TickKind::Day => height.to_string(),
        TickKind::Time => fmt1(axis_y),
    }
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
fn glyph(dot: &Dot, y: f64) -> Element {
    let name = dot.tone.name();
    let (cx, cy) = (fmt1(dot.x), fmt1(y));
    if dot.is_cluster() {
        rsx! {
            g { class: "pk-ag-cluster pk-ag-glyph pk-ag-{name}", "data-count": "{dot.count}",
                circle {
                    class: "pk-ag-dot pk-ag-{name}",
                    cx: "{cx}",
                    cy: "{cy}",
                    r: "5",
                    dangerous_inner_html: "{tip(&dot.cluster_title())}",
                }
                text {
                    class: "pk-ag-cluster-count",
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
                class: "pk-ag-dot pk-ag-glyph pk-ag-{name}",
                cx: "{cx}",
                cy: "{cy}",
                r: "3",
                dangerous_inner_html: "{tip(&dot.title)}",
            }
        }
    }
}

/// A fork lane under its item: its line, a square tip node and the caption.
fn fork_row(fork: &Fork) -> Element {
    let name = fork.tone.name();
    let y = fmt1(fork.y);
    let node_x = fork.node_left();
    let caption_x = fork.x1 + FORK_NODE_PX;
    let hit_x = node_x - 2.0;
    let hit_w = caption_x + label_px(&fork.label) + 4.0 - hit_x;
    rsx! {
        g {
            class: if fork.dead { "pk-ag-fork pk-ag-dead" } else { "pk-ag-fork" },
            "data-kind": "{fork.kind.as_str()}",
            rect {
                class: "pk-ag-hit",
                x: "{fmt1(hit_x)}",
                y: "{fmt1(fork.y - 6.0)}",
                width: "{fmt1(hit_w)}",
                height: "12",
                dangerous_inner_html: "{tip(&fork.title)}",
            }
            line {
                class: "pk-ag-line pk-ag-fork-line",
                x1: "{fmt1(fork.x0)}",
                x2: "{fmt1(fork.x1)}",
                y1: "{y}",
                y2: "{y}",
            }
            rect {
                class: "pk-ag-fork-node pk-ag-{name}",
                x: "{fmt1(node_x)}",
                y: "{fmt1(fork.y - NODE / 2.0)}",
                width: "{NODE}",
                height: "{NODE}",
            }
            text {
                class: "pk-ag-fork-label",
                x: "{fmt1(caption_x)}",
                y: "{y}",
                dominant_baseline: "central",
                "{fork.label}"
            }
        }
    }
}

/// One item as a button: its line, dots, forks and elbows, and its id at the
/// tip (and at the start of a long line). A component of its own, so picking
/// an item re-renders two rows, not all of them.
#[component]
fn ItemRow(
    id: String,
    graph: Shared<Graph>,
    at: usize,
    picked: bool,
    on_select: EventHandler<String>,
) -> Element {
    let item = &graph.items[at];
    let name = item.tone.name();
    let mut class = format!("pk-ag-item pk-ag-{name}");
    if item.done {
        class.push_str(" pk-ag-done");
    }
    if picked {
        class.push_str(" on");
    }
    let tooltip = item.tooltip();
    let hit = item.hit_rect();
    let y = fmt1(item.y);
    let (click, press) = (item.id.clone(), item.id.clone());
    rsx! {
        g {
            id: "{id}-graph-{slug(&item.id)}",
            class: "{class}",
            role: "button",
            tabindex: "0",
            "aria-pressed": "{picked}",
            "aria-label": "{tooltip}",
            "data-id": "{item.id}",
            onclick: move |_| on_select.call(click.clone()),
            onkeydown: move |event| {
                if presses(&event) {
                    event.prevent_default();
                    on_select.call(press.clone());
                }
            },
            rect {
                class: "pk-ag-hit",
                x: "{fmt1(hit.x)}",
                y: "{fmt1(hit.y)}",
                width: "{fmt1(hit.w)}",
                height: "{fmt1(hit.h)}",
                dangerous_inner_html: "{tip(&tooltip)}",
            }
            line {
                class: "pk-ag-line",
                x1: "{fmt1(item.x0)}",
                x2: "{fmt1(item.x1)}",
                y1: "{y}",
                y2: "{y}",
            }
            for dot in item.dots.iter() {
                {glyph(dot, item.y)}
            }
            for fork in item.forks.iter() {
                {fork_row(fork)}
            }
            for edge in item.edges.iter() {
                path {
                    class: if edge.dead {
                        format!("pk-ag-edge pk-ag-edge-{} pk-ag-dead", edge.kind.as_str())
                    } else {
                        format!("pk-ag-edge pk-ag-edge-{}", edge.kind.as_str())
                    },
                    d: "{edge.d}",
                }
            }
            if let Some(start) = item.start_label_x {
                text {
                    class: "pk-ag-label pk-ag-label-start",
                    x: "{fmt1(start)}",
                    y: "{y}",
                    text_anchor: "end",
                    dominant_baseline: "central",
                    "{item.id}"
                }
            }
            text {
                class: "pk-ag-label",
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
            class: "pk-ag-strip",
            onclick: move |_| on_action.call(GraphAction::ToggleStripKind(kind.to_string())),
            rect {
                class: "pk-ag-hit",
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
            class: "pk-ag-loose",
            onclick: move |_| on_action.call(GraphAction::ToggleStripKind(kind.to_string())),
            rect {
                class: "pk-ag-hit",
                x: "0",
                y: "{fmt1(row.y - height / 2.0)}",
                width: "{width}",
                height: "{fmt1(height)}",
                dangerous_inner_html: "{tip(&title)}",
            }
            line { class: "pk-ag-loose-line", x1: "0", x2: "{width}", y1: "{y}", y2: "{y}" }
            for dot in row.dots.iter() {
                {glyph(dot, row.y)}
            }
        }
    }
}

/// The canvas of one layout. Its props compare by identity, so a render that
/// changes nothing about the layout or the selection costs one pointer test.
#[component]
pub(super) fn Canvas(
    id: String,
    graph: Shared<Graph>,
    selected: Option<String>,
    on_action: EventHandler<GraphAction>,
    on_select: EventHandler<String>,
) -> Element {
    let layout: &Graph = &graph;
    let hatch = format!("{id}-hatch");
    let (width, height) = (layout.width, layout.height);
    let row_h = layout.loose_y.unwrap_or(7.0) * 2.0;
    let axis_y = layout.axis_y;
    rsx! {
        div { class: "pk-ag-canvas",
            svg {
                class: "pk-ag-svg",
                width: "{width}",
                height: "{height}",
                view_box: "0 0 {width} {height}",
                role: "group",
                "aria-label": "{layout.aria_label()}",
                defs {
                    pattern {
                        id: "{hatch}",
                        width: "4",
                        height: "4",
                        pattern_units: "userSpaceOnUse",
                        pattern_transform: "rotate(45)",
                        line { class: "pk-ag-hatch-line", x1: "0", y1: "0", x2: "0", y2: "4" }
                    }
                }
                g { class: "pk-ag-gaps", "aria-hidden": "true",
                    for (at , gap) in layout.gaps.iter().enumerate() {
                        rect {
                            key: "{at}",
                            class: "pk-ag-gap",
                            x: "{fmt1(gap.x0)}",
                            y: "0",
                            width: "{fmt1(gap.x1 - gap.x0)}",
                            height: "{fmt1(axis_y)}",
                            fill: "url(#{hatch})",
                            dangerous_inner_html: "{tip(&gap.title())}",
                        }
                    }
                }
                g { class: "pk-ag-grid", "aria-hidden": "true",
                    for (at , tick) in layout.ticks.iter().enumerate() {
                        line {
                            key: "{at}",
                            class: "pk-ag-tick pk-ag-tick-{tick.kind.as_str()}",
                            x1: "{fmt1(tick.x)}",
                            x2: "{fmt1(tick.x)}",
                            y1: "0",
                            y2: "{tick_end(tick.kind, height, axis_y)}",
                        }
                    }
                }
                for row in layout.strip.iter().filter(|row| !row.open) {
                    {strip_row(&id, row, width, row_h, on_action)}
                }
                for row in layout.loose.iter() {
                    {loose_row(row, width, row_h, on_action)}
                }
                for (at , item) in layout.items.iter().enumerate() {
                    ItemRow {
                        key: "{item.id}",
                        id: id.clone(),
                        graph: graph.clone(),
                        at,
                        picked: selected.as_deref() == Some(item.id.as_str()),
                        on_select,
                    }
                }
                g { class: "pk-ag-axis", "aria-hidden": "true",
                    for (at , tick) in layout.ticks.iter().enumerate() {
                        text {
                            key: "{at}",
                            class: "pk-ag-tick-label pk-ag-tick-label-{tick.kind.as_str()}",
                            x: "{fmt1(tick.x + 3.0)}",
                            y: "{fmt1(axis_y + label_drop(tick.kind))}",
                            "{tick.label}"
                        }
                    }
                }
            }
        }
    }
}
