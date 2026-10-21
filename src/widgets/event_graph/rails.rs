//! `EventRail`: paints a [`RailView`] (session tree / item timeline) as an SVG
//! gutter of trunk, lane lines, elbows and dots beside one row per event.
//!
//! Build the view with `rails::vertical(..).view(&RailOptions)`; the painter
//! decides nothing. Rows with `Hit::Button` are toggle buttons (`aria-pressed`,
//! class `on` when selected; Enter/Space click natively) that call `on_select`
//! with the row id; `Hit::Static` rows are inert spans. Styled by the
//! `pk-rail-*` rules in `assets/panel-kit.css`.

use dioxus::prelude::*;
use panel_kit_core::widgets::event_graph::rails::{Hit, RailCanvas, RailRow, RailView};
use panel_kit_core::widgets::event_graph::Tone;

/// Dot radius in px: filled dots are 7px across, hollow ones keep 7px outer
/// width with the 1.5px stroke drawn inside the same box.
fn dot_radius(tone: Tone) -> &'static str {
    match tone {
        Tone::Hollow => "2.75",
        _ => "3.5",
    }
}

/// `<title>` markup for an SVG leaf: dioxus-web builds an rsx `title` in the
/// XHTML namespace, so the native SVG tooltip only works through inner HTML.
fn svg_tip(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 15);
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

fn dead_class(dead: bool) -> &'static str {
    if dead {
        " dead"
    } else {
        ""
    }
}

fn rails(canvas: &RailCanvas) -> Element {
    let width = canvas.gutter;
    let height = canvas.height;
    rsx! {
        svg {
            class: "pk-rail-svg",
            width: "{width}",
            height: "{height}",
            view_box: "0 0 {width} {height}",
            "aria-hidden": "true",
            line {
                class: "pk-rail-trunk",
                x1: "{canvas.trunk_x}",
                x2: "{canvas.trunk_x}",
                y1: "{canvas.trunk_y0}",
                y2: "{canvas.trunk_y1}",
            }
            for fork in canvas.forks.iter() {
                line {
                    key: "{fork.id}",
                    class: "pk-rail-lane{dead_class(fork.dead)}",
                    x1: "{fork.x}",
                    x2: "{fork.x}",
                    y1: "{fork.y0}",
                    y2: "{fork.y1}",
                }
            }
            for (at, edge) in canvas.edges.iter().enumerate() {
                path {
                    key: "{at}",
                    class: "pk-rail-edge {edge.kind.as_str()}{dead_class(edge.dead)}",
                    d: "{edge.d}",
                }
            }
            for row in canvas.rows.iter() {
                circle {
                    key: "{row.row_id}",
                    class: "pk-rail-dot {row.tone.colour()}{dead_class(row.dead)}{on_class(row)}",
                    "data-tone": "{row.tone.name()}",
                    cx: "{row.x}",
                    cy: "{row.y}",
                    r: dot_radius(row.tone),
                    dangerous_inner_html: svg_tip(&row.title),
                }
            }
        }
    }
}

fn on_class(row: &RailRow) -> &'static str {
    if row.selected {
        " on"
    } else {
        ""
    }
}

fn cells(row: &RailRow) -> Element {
    rsx! {
        time { class: "pk-rail-time", datetime: "{row.iso}", "{row.stamp}" }
        if let Some(chip) = row.chip.as_ref() {
            span { class: "pk-rail-chip", "{chip}" }
        }
        span { class: "pk-rail-label", "{row.label}" }
    }
}

fn row_item(row: &RailRow, on_select: Option<EventHandler<String>>) -> Element {
    let mut class = String::from("pk-rail-row");
    if row.lane_marker {
        class.push_str(" marker");
    }
    if row.dead {
        class.push_str(" dead");
    }
    if row.separator {
        class.push_str(" sep");
    }
    if row.selected {
        class.push_str(" on");
    }
    let hit = match row.hit {
        Hit::Button => {
            let id = row.row_id.clone();
            let pressed = if row.selected { "true" } else { "false" };
            let hit_class = if row.selected {
                "pk-rail-hit on"
            } else {
                "pk-rail-hit"
            };
            rsx! {
                button {
                    class: hit_class,
                    r#type: "button",
                    title: "{row.title}",
                    aria_pressed: pressed,
                    onclick: move |_| {
                        if let Some(handler) = on_select {
                            handler.call(id.clone());
                        }
                    },
                    {cells(row)}
                }
            }
        }
        Hit::Static => rsx! {
            span { class: "pk-rail-hit", title: "{row.title}", {cells(row)} }
        },
    };
    rsx! {
        li {
            key: "{row.row_id}",
            id: "{row.dom_id}",
            class: "{class}",
            "data-kind": "{row.kind.as_str()}",
            {hit}
        }
    }
}

/// The rail: heading with counts, SVG gutter and one row per event, or the
/// empty-state line when the view has no rows. `on_select` receives the id of
/// a clicked `Hit::Button` row; the host owns the selection state (see
/// `VerticalGraph::select_node`) and passes it back through the next view.
#[component]
pub fn EventRail(
    /// Resolved rail from `VerticalGraph::view`.
    view: RailView,
    /// Called with the row id when a button row is clicked.
    #[props(default)]
    on_select: Option<EventHandler<String>>,
) -> Element {
    let dom_id = &view.dom_id;
    rsx! {
        figure {
            id: "{dom_id}",
            class: "pk-widget pk-rail",
            aria_label: "{view.title}",
            if let Some(heading) = view.heading.as_ref() {
                figcaption { class: "pk-rail-head", id: "{dom_id}-head",
                    span { class: "pk-rail-title", "{heading.title}" }
                    if let Some(meta) = heading.meta.as_ref() {
                        span { class: "pk-rail-meta", "{meta}" }
                    }
                }
            }
            if let Some(empty) = view.empty {
                p { class: "pk-rail-empty", id: "{dom_id}-empty", "{empty}" }
            }
            if let Some(canvas) = view.canvas.as_ref() {
                div {
                    class: "pk-rail-canvas {canvas.tone.colour()}{done_class(canvas.done)}",
                    style: "--pk-rail-gutter: {canvas.gutter}px;",
                    {rails(canvas)}
                    ol { class: "pk-rail-rows", id: "{dom_id}-rows",
                        for row in canvas.rows.iter() {
                            {row_item(row, on_select)}
                        }
                    }
                }
            }
        }
    }
}

fn done_class(done: bool) -> &'static str {
    if done {
        " done"
    } else {
        ""
    }
}
