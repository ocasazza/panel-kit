//! `EventRail`: paints a [`RailView`] (session tree / item timeline) as an SVG
//! gutter of trunk, lane lines, elbows and dots beside one row per event.
//!
//! Build the view with `rails::vertical_with(.., &open, opts.wrap).view(&opts)`;
//! the painter decides nothing. Interactive rows (`Hit::Button`) are disclosure
//! buttons (`aria-expanded`, class `open` when open; Enter/Space click natively)
//! that call `on_toggle` with the row id and, on selectable rows, `on_select`;
//! Escape on a focused open row collapses it. `Hit::Static` rows are inert
//! spans. The host owns the [`Expansion`] and passes it back through the next
//! view. Styled by the `pk-rail-*` rules in `assets/panel-kit.css`.

use dioxus::prelude::*;
use panel_kit_core::widgets::event_graph::facts::Expansion;
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

/// The disclosure caret: the table's fold glyphs, quiet until hover, focus or
/// open. Only button rows carry one.
fn caret(open: bool) -> Element {
    rsx! {
        span {
            class: "pk-rail-caret",
            aria_hidden: "true",
            if open { "\u{25be}" } else { "\u{25b8}" }
        }
    }
}

fn cells(row: &RailRow) -> Element {
    rsx! {
        time { class: "pk-rail-time", datetime: "{row.iso}", "{row.stamp}" }
        if let Some(chip) = row.chip.as_ref() {
            span { class: "pk-rail-chip", "{chip}" }
        }
        if row.hit == Hit::Button {
            {caret(row.open)}
        }
        span { class: "pk-rail-label", "{row.label}" }
    }
}

fn hit_class(row: &RailRow) -> String {
    let mut class = String::from("pk-rail-hit");
    if row.open {
        class.push_str(" open");
    }
    if row.selected {
        class.push_str(" on");
    }
    class
}

fn row_item(
    row: &RailRow,
    on_select: Option<EventHandler<String>>,
    on_toggle: Option<EventHandler<String>>,
) -> Element {
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
    if row.open {
        class.push_str(" open");
    }
    if row.selected {
        class.push_str(" on");
    }
    let hit = match row.hit {
        Hit::Button => {
            let toggle_id = row.row_id.clone();
            let select_id = row.row_id.clone();
            let esc_id = row.row_id.clone();
            let selectable = !row.lane_marker;
            let open = row.open;
            let class = hit_class(row);
            rsx! {
                button {
                    class: "{class}",
                    r#type: "button",
                    title: "{row.title}",
                    aria_expanded: "{row.open}",
                    aria_controls: "{row.detail_dom_id}",
                    onclick: move |_| {
                        if let Some(handler) = on_toggle {
                            handler.call(toggle_id.clone());
                        }
                        if selectable {
                            if let Some(handler) = on_select {
                                handler.call(select_id.clone());
                            }
                        }
                    },
                    onkeydown: move |e: KeyboardEvent| {
                        if e.key() == Key::Escape && open {
                            if let Some(handler) = on_toggle {
                                handler.call(esc_id.clone());
                            }
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

/// The open row's facts as a sibling block: ghost slots of the row's own cells
/// keep the grid aligned to the label column, one 22px row per fact line.
fn detail_item(row: &RailRow) -> Element {
    let width = row
        .facts
        .iter()
        .map(|fact| fact.label.len())
        .max()
        .unwrap_or(0);
    let label = format!("{} details", row.label);
    rsx! {
        li {
            key: "detail-{row.row_id}",
            id: "{row.detail_dom_id}",
            class: "pk-rail-detail",
            role: "group",
            aria_label: "{label}",
            div {
                class: "pk-rail-detail-in",
                span { class: "pk-rail-time pk-rail-ghost", "{row.stamp}" }
                if let Some(chip) = row.chip.as_ref() {
                    span { class: "pk-rail-chip pk-rail-ghost", "{chip}" }
                }
                span { class: "pk-rail-caret pk-rail-ghost" }
                div {
                    class: "pk-rail-facts",
                    style: "--pk-rail-labels: {width}ch",
                    for fact in row.facts.iter() {
                        div { class: "pk-rail-fact", key: "{fact.label}",
                            span { class: "pk-rail-fact-label", "{fact.label}" }
                            div { class: "pk-rail-fact-value",
                                for (at, line) in fact.lines.iter().enumerate() {
                                    span {
                                        key: "{at}",
                                        class: "pk-rail-fact-line",
                                        title: "{line}",
                                        "{line}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The rail: heading with counts and, on an interactive rail, the expand and
/// collapse actions; SVG gutter; one row per event with a fact block under
/// every open row; or the empty-state line when the view has no rows.
///
/// The host owns the open set: `on_toggle` receives the clicked row's id,
/// `on_open_all` every row id and `on_close_all` nothing; the next view
/// resolves the state (see `VerticalGraph::view`). `on_select` still receives
/// a selectable row's id for hosts that also keep a selection.
#[component]
pub fn EventRail(
    /// Resolved rail from `VerticalGraph::view`.
    view: RailView,
    /// Open rows; drives `aria-expanded`, the fact blocks and the heading
    /// actions' disabled states.
    #[props(default)]
    expanded: Expansion,
    /// Called with the row id when a button row is clicked: toggle its facts.
    #[props(default)]
    on_toggle: Option<EventHandler<String>>,
    /// Called with the id of a clicked selectable row: move the selection.
    #[props(default)]
    on_select: Option<EventHandler<String>>,
    /// Called with every row id for the heading's `expand all`.
    #[props(default)]
    on_open_all: Option<EventHandler<Vec<String>>>,
    /// Called for the heading's `collapse all`.
    #[props(default)]
    on_close_all: Option<EventHandler<()>>,
) -> Element {
    let dom_id = &view.dom_id;
    let interactive = view
        .canvas
        .as_ref()
        .is_some_and(|canvas| canvas.rows.iter().any(|row| row.hit == Hit::Button));
    let all_open = interactive
        && view
            .canvas
            .as_ref()
            .is_some_and(|canvas| canvas.rows.iter().all(|row| expanded.is_open(&row.row_id)));
    let ids: Vec<String> = view
        .canvas
        .iter()
        .flat_map(|canvas| canvas.rows.iter().map(|row| row.row_id.clone()))
        .collect();
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
                    if interactive {
                        button {
                            r#type: "button",
                            class: "pk-dt-btn",
                            disabled: all_open,
                            onclick: move |_| {
                                if let Some(handler) = on_open_all {
                                    handler.call(ids.clone());
                                }
                            },
                            "expand all"
                        }
                        button {
                            r#type: "button",
                            class: "pk-dt-btn",
                            disabled: expanded.is_empty(),
                            onclick: move |_| {
                                if let Some(handler) = on_close_all {
                                    handler.call(());
                                }
                            },
                            "collapse all"
                        }
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
                            {row_item(row, on_select, on_toggle)}
                            if row.open {
                                {detail_item(row)}
                            }
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
