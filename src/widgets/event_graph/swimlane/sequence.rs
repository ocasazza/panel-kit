//! The LANES body: a sequence diagram. One lifeline per drawn agent under a
//! sticky row of lane headers, time flowing down in fixed-pitch rows beside a
//! sticky gutter of stamps, an arrow where two agents handed off, passed a
//! baton or reviewed, a bar across every lifeline for a global action, a dot
//! for plain activity.
//!
//! The lanes are as tall as every row; only the rows near the viewport are
//! mounted (`window`). Each lane is at least `LANE_MIN_W` wide and the body
//! scrolls sideways once they do not fit. A row opens its event in the
//! inspector; a lane header narrows the rows to the ones that touch it, which
//! the host keeps in `GraphState::lane`.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::widgets::event_graph::query::{GraphAction, Tone};
use panel_kit_core::widgets::event_graph::swimlane::{
    clock_label, day_label, detail, fmt1, label, shows_count, slug, Columns, EdgeKind, LaneHead,
    Node, SeqRow, Sequence, GUTTER_W, ROW_PITCH,
};
use panel_kit_core::widgets::event_graph::time::format_iso8601;
use panel_kit_core::widgets::event_graph::Event;

use super::canvas::tip;
use super::dom;
use super::shared::{Shared, Slot};
use super::window;

/// The scroller's width before it is measured, and under SSR.
const DEFAULT_WIDTH: f64 = 960.0;
/// Width of one character of the caption face.
const CHAR_PX: f64 = 6.6;
/// Length of an arrowhead, along the arrow.
const HEAD_LEN: f64 = 9.0;
/// Height of an arrowhead, across the arrow.
const HEAD_H: f64 = 10.0;
/// Names a folded lane's tooltip lists before it counts the rest.
const TITLE_NAMES: usize = 40;
/// How far an arrow that begins and ends on one lane reaches out and back.
const LOOP_W: f64 = 22.0;

/// Enter and Space press a row.
fn presses(event: &KeyboardEvent) -> bool {
    match event.key() {
        Key::Enter => true,
        Key::Character(c) => c == " ",
        _ => false,
    }
}

/// `text` cut to `max` characters, with `…` where it was cut.
fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// What every row of one diagram shares.
struct Frame<'a> {
    id: &'a str,
    cols: Columns,
    sequence: &'a Sequence,
    events: &'a [Event],
    picked: Option<&'a str>,
    on_open: EventHandler<Node>,
}

impl Frame<'_> {
    /// The x of a lifeline, in the lanes' own coordinates.
    fn x(&self, lane: u32) -> f64 {
        self.cols.x(lane as usize)
    }

    fn agent(&self, lane: u32) -> &str {
        self.sequence
            .lanes
            .get(lane as usize)
            .map_or("", |lane| lane.agent.as_str())
    }

    /// The id of the arrowhead marker of `tone`.
    fn marker(&self, tone: &str) -> String {
        format!("{}-seq-head-{tone}", self.id)
    }
}

/// A lane header's tooltip: who a folded lane holds, or when this one acted.
fn lane_title(lane: &LaneHead) -> String {
    if lane.folded.is_empty() {
        return format!(
            "{} \u{b7} {} interactions \u{b7} {} \u{2013} {}",
            lane.agent,
            lane.count,
            format_iso8601(lane.first),
            format_iso8601(lane.last),
        );
    }
    let mut names = lane
        .folded
        .iter()
        .take(TITLE_NAMES)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if lane.folded.len() > TITLE_NAMES {
        names.push_str(&format!(", +{} more", lane.folded.len() - TITLE_NAMES));
    }
    names
}

/// The group every row that opens an event shares: key, id, role, handlers, a
/// transparent hit rect `(x, y, width, height)` that carries the tooltip, and
/// what the row draws.
fn open_row(
    frame: &Frame<'_>,
    at: usize,
    event: &Event,
    class: String,
    hit: (f64, f64, f64, f64),
    extra: Element,
) -> Element {
    let caption = label(event);
    let opens = Node::Event(event.id.clone());
    let press = opens.clone();
    let on_open = frame.on_open;
    let is_picked = frame.picked == Some(event.id.as_str());
    let class = if is_picked {
        format!("{class} on")
    } else {
        class
    };
    let (x, y, width, height) = hit;
    rsx! {
        g {
            key: "{at}",
            id: "{frame.id}-seq-{slug(&event.id)}",
            class: "{class}",
            role: "button",
            tabindex: "0",
            "aria-pressed": "{is_picked}",
            "aria-label": "{caption}",
            "data-event": "{event.id}",
            "data-kind": "{event.kind}",
            onclick: move |_| on_open.call(opens.clone()),
            onkeydown: move |key| {
                if presses(&key) {
                    key.prevent_default();
                    on_open.call(press.clone());
                }
            },
            rect {
                class: "pk-ag-seq-hit",
                x: "{fmt1(x)}",
                y: "{fmt1(y)}",
                width: "{fmt1(width)}",
                height: "{fmt1(height)}",
                dangerous_inner_html: "{tip(&detail(event))}",
            }
            {extra}
        }
    }
}

fn edge_row(
    frame: &Frame<'_>,
    at: usize,
    event: &Event,
    from: u32,
    to: u32,
    kind: EdgeKind,
) -> Element {
    let y = at as f64 * ROW_PITCH + 16.0;
    let (fx, tx) = (frame.x(from), frame.x(to));
    let looped = from == to;
    let (start, end) = if looped { (fx + LOOP_W, fx) } else { (fx, tx) };
    let toward = if end >= start { 1.0 } else { -1.0 };
    // The tip stops 1px short of the target lifeline, and the line stops 1px
    // inside the head, so nothing shows past the point.
    let stop = end - 2.0 * toward;
    let dir = if looped {
        "self"
    } else if toward > 0.0 {
        "right"
    } else {
        "left"
    };
    let (cap_x, room, anchor) = if looped {
        (
            start + 6.0,
            frame.cols.width - start - 6.0,
            "pk-ag-seq-cap pk-ag-seq-cap-start",
        )
    } else {
        (
            (start + end) / 2.0,
            (end - start).abs() - 8.0,
            "pk-ag-seq-cap",
        )
    };
    let caption = clip(&label(event), (room.max(0.0) / CHAR_PX) as usize);
    let tone = event.tone().name();
    let class = format!("pk-ag-seq-edge pk-ag-{tone} pk-ag-seq-{}", kind.as_str());
    let (from_name, to_name) = (frame.agent(from).to_string(), frame.agent(to).to_string());
    let reach = (start.min(end) - 6.0, (start - end).abs() + 12.0);
    let extra = rsx! {
        line {
            class: "pk-ag-seq-arrow",
            x1: "{fmt1(start)}",
            x2: "{fmt1(stop)}",
            y1: "{fmt1(y)}",
            y2: "{fmt1(y)}",
            "marker-end": "url(#{frame.marker(tone)})",
            "data-dir": "{dir}",
            "data-from": "{from_name}",
            "data-to": "{to_name}",
            "data-edge": "{kind.as_str()}",
        }
        circle {
            class: "pk-ag-seq-anchor",
            cx: "{fmt1(start)}",
            cy: "{fmt1(y)}",
            r: "2",
        }
        text {
            class: "{anchor}",
            x: "{fmt1(cap_x)}",
            y: "{fmt1(y - 5.0)}",
            "{caption}"
        }
    };
    open_row(
        frame,
        at,
        event,
        class,
        (reach.0, y - 6.0, reach.1, 12.0),
        extra,
    )
}

fn broadcast_row(frame: &Frame<'_>, at: usize, event: &Event) -> Element {
    let y = at as f64 * ROW_PITCH + 16.0;
    let width = frame.cols.width;
    let left = if frame.sequence.lanes.is_empty() {
        0.0
    } else {
        frame.x(0)
    };
    let caption = clip(
        &label(event),
        ((width - left - 12.0).max(0.0) / CHAR_PX) as usize,
    );
    let class = format!("pk-ag-seq-broadcast pk-ag-{}", event.tone().name());
    let extra = rsx! {
        line {
            class: "pk-ag-seq-bar",
            x1: "4",
            x2: "{fmt1(width - 4.0)}",
            y1: "{fmt1(y)}",
            y2: "{fmt1(y)}",
        }
        text {
            class: "pk-ag-seq-cap pk-ag-seq-cap-start",
            x: "{fmt1(left + 6.0)}",
            y: "{fmt1(y - 5.0)}",
            "{caption}"
        }
    };
    open_row(frame, at, event, class, (0.0, y - 6.0, width, 12.0), extra)
}

fn dot_row(frame: &Frame<'_>, at: usize, event: &Event, lane: u32) -> Element {
    let y = at as f64 * ROW_PITCH + 11.0;
    let x = frame.x(lane);
    let room = frame.cols.span(lane as usize) - 16.0;
    let caption = clip(&label(event), (room.max(0.0) / CHAR_PX) as usize);
    let tone = event.tone().name();
    let class = format!("pk-ag-seq-act pk-ag-{tone}");
    let extra = rsx! {
        circle {
            class: "pk-ag-dot pk-ag-{tone}",
            cx: "{fmt1(x)}",
            cy: "{fmt1(y)}",
            r: "3.5",
        }
        text {
            class: "pk-ag-seq-cap pk-ag-seq-cap-start",
            x: "{fmt1(x + 8.0)}",
            y: "{fmt1(y + 4.0)}",
            "{caption}"
        }
    };
    let span = frame.cols.span(lane as usize);
    open_row(
        frame,
        at,
        event,
        class,
        (x - span / 2.0, y - 11.0, span, ROW_PITCH),
        extra,
    )
}

fn day_row(frame: &Frame<'_>, at: usize, day: i64) -> Element {
    let y = at as f64 * ROW_PITCH + 11.0;
    rsx! {
        g { key: "{at}", class: "pk-ag-seq-day", "data-day": "{day_label(day)}",
            line {
                class: "pk-ag-seq-rule",
                x1: "0",
                x2: "{fmt1(frame.cols.width)}",
                y1: "{fmt1(y)}",
                y2: "{fmt1(y)}",
            }
        }
    }
}

/// One row of the lanes, at its index.
fn row_view(frame: &Frame<'_>, at: usize) -> Element {
    let row = frame.sequence.rows[at];
    let event = row.event().and_then(|i| frame.events.get(i as usize));
    match (row, event) {
        (SeqRow::Day { at: day }, _) => day_row(frame, at, day),
        (
            SeqRow::Edge {
                from_lane,
                to_lane,
                kind,
                ..
            },
            Some(event),
        ) => edge_row(frame, at, event, from_lane, to_lane, kind),
        (SeqRow::Broadcast { .. }, Some(event)) => broadcast_row(frame, at, event),
        (SeqRow::Dot { lane, .. }, Some(event)) => dot_row(frame, at, event, lane),
        _ => rsx! {},
    }
}

/// One row of the gutter: a day's label or an event's stamp.
fn gutter_row(frame: &Frame<'_>, at: usize) -> Element {
    let row = frame.sequence.rows[at];
    let y = at as f64 * ROW_PITCH;
    match row {
        SeqRow::Day { at: day } => rsx! {
            text {
                key: "{at}",
                class: "pk-ag-seq-daylabel",
                x: "{fmt1(GUTTER_W - 8.0)}",
                y: "{fmt1(y + 15.0)}",
                "data-day": "{day_label(day)}",
                "{day_label(day)}"
            }
        },
        other => rsx! {
            text {
                key: "{at}",
                class: "pk-ag-seq-stamp",
                x: "{fmt1(GUTTER_W - 8.0)}",
                y: "{fmt1(y + 15.0)}",
                "{clock_label(other.at())}"
            }
        },
    }
}

/// One lane header: the agent, its interaction count while the column is wide
/// enough, and the focus toggle. The folded lane is a plain label.
fn lane_head(
    id: &str,
    lane: &LaneHead,
    span: f64,
    focused: bool,
    on_action: EventHandler<GraphAction>,
) -> Element {
    let style = format!("width: {span}px");
    let count = shows_count(span).then_some(lane.count);
    if !lane.folded.is_empty() {
        return rsx! {
            div {
                key: "{lane.x_index}",
                id: "{id}-seq-lane-{lane.x_index}",
                class: "pk-ag-seq-lane pk-ag-seq-fold",
                style: "{style}",
                title: "{lane_title(lane)}",
                "data-lane": "{lane.agent}",
                span { class: "pk-ag-seq-name", "{lane.agent}" }
                if let Some(count) = count {
                    span { class: "pk-ag-count", "{count}" }
                }
            }
        };
    }
    let next = (!focused).then(|| lane.agent.clone());
    rsx! {
        button {
            key: "{lane.x_index}",
            id: "{id}-seq-lane-{lane.x_index}",
            class: if focused { "pk-ag-seq-lane on" } else { "pk-ag-seq-lane" },
            style: "{style}",
            r#type: "button",
            aria_pressed: "{focused}",
            title: "{lane_title(lane)}",
            "data-lane": "{lane.agent}",
            onclick: move |_| on_action.call(GraphAction::FocusLane(next.clone())),
            span { class: "pk-ag-seq-name", "{lane.agent}" }
            if let Some(count) = count {
                span { class: "pk-ag-count", "{count}" }
            }
        }
    }
}

/// The LANES mode body.
///
/// `sequence` is the diagram to draw, lanes already picked and folded;
/// `focus` is the agent whose rows alone are drawn.
#[component]
pub(super) fn SequenceBody(
    id: String,
    sequence: Shared<Sequence>,
    events: Shared<Vec<Event>>,
    picked: Option<String>,
    focus: Option<String>,
    on_open: EventHandler<Node>,
    on_action: EventHandler<GraphAction>,
) -> Element {
    let memo = use_hook(|| {
        Rc::new(RefCell::new(Slot::<
            (Shared<Sequence>, usize),
            Shared<Sequence>,
        >::default()))
    });
    let lane = focus.as_deref().and_then(|agent| {
        sequence
            .lanes
            .iter()
            .position(|lane| lane.agent == agent && lane.folded.is_empty())
    });
    let shown: Shared<Sequence> = match lane {
        Some(lane) => memo.borrow_mut().get((sequence.clone(), lane), || {
            Shared::new(sequence.touching(lane))
        }),
        None => sequence.clone(),
    };
    let rows = shown.rows.len();
    let mut mounted = use_signal(|| {
        window::y_window(
            0.0,
            window::DEFAULT_VIEWPORT,
            ROW_PITCH,
            rows,
            window::OVERSCAN,
        )
    });
    let mut room = use_signal(|| DEFAULT_WIDTH);

    if sequence.is_empty() {
        let why = if sequence.read == 0 {
            "nothing recorded in this window"
        } else if !sequence.activity && sequence.quiet > 0 {
            "no two agents met in this window; activity shows the rest"
        } else {
            "no two agents met in this window"
        };
        return rsx! {
            p { id: "{id}-sequence-empty", class: "pk-ag-empty", "{why}" }
        };
    }

    let (start, end) = {
        let (start, end) = *mounted.read();
        (start.min(rows), end.min(rows))
    };
    let scroll_id = format!("{id}-sequence-scroll");
    let lanes = sequence.lanes.len();
    let cols = sequence.columns(*room.read());
    let height = shown.height();
    let frame = Frame {
        id: &id,
        cols,
        sequence: &shown,
        events: &events,
        picked: picked.as_deref(),
        on_open,
    };
    let on_scroll = {
        let scroll_id = scroll_id.clone();
        move |_| {
            let Some(scroll) = dom::scroll(&scroll_id) else {
                return;
            };
            if !window::covers(*mounted.peek(), scroll.top, scroll.height, ROW_PITCH, rows) {
                mounted.set(window::y_window(
                    scroll.top,
                    scroll.height,
                    ROW_PITCH,
                    rows,
                    window::OVERSCAN,
                ));
            }
        }
    };
    let on_resize = move |event: dioxus::prelude::Event<ResizeData>| {
        if let Ok(size) = event.get_border_box_size() {
            if (size.width - *room.peek()).abs() >= 1.0 {
                room.set(size.width);
            }
        }
    };
    let focused = lane.map(|at| sequence.lanes[at].agent.clone());
    let total = GUTTER_W + cols.width;
    let aria = format!(
        "{} lanes, {} rows of handoffs, reviews and shared actions",
        lanes, shown.drawn
    );
    rsx! {
        section {
            id: "{id}-sequence",
            class: "pk-ag-seq",
            aria_label: "sequence",
            "data-lanes": "{lanes}",
            "data-rows": "{rows}",
            "data-start": "{start}",
            "data-end": "{end}",
            "data-scrolls": "{cols.scrolls}",
            "data-focus": focused.clone(),
            div {
                id: "{scroll_id}",
                class: "pk-ag-seq-scroll",
                tabindex: "-1",
                onscroll: on_scroll,
                onresize: on_resize,
                div {
                    id: "{id}-sequence-head",
                    class: "pk-ag-seq-head",
                    style: "width: {total}px",
                    span {
                        class: "pk-ag-seq-corner",
                        style: "width: {GUTTER_W}px",
                        aria_hidden: "true",
                    }
                    for head in sequence.lanes.iter() {
                        {lane_head(
                            &id,
                            head,
                            cols.span(head.x_index),
                            focused.as_deref() == Some(head.agent.as_str()) && head.folded.is_empty(),
                            on_action,
                        )}
                    }
                }
                div { class: "pk-ag-seq-body", style: "width: {total}px",
                    svg {
                        class: "pk-ag-seq-gutter",
                        width: "{GUTTER_W}",
                        height: "{fmt1(height)}",
                        view_box: "0 0 {GUTTER_W} {fmt1(height)}",
                        "aria-hidden": "true",
                        for at in start..end {
                            {gutter_row(&frame, at)}
                        }
                    }
                    svg {
                        class: "pk-ag-svg pk-ag-seq-svg",
                        width: "{fmt1(cols.width)}",
                        height: "{fmt1(height)}",
                        view_box: "0 0 {fmt1(cols.width)} {fmt1(height)}",
                        role: "group",
                        "aria-label": "{aria}",
                        defs {
                            for tone in Tone::ALL {
                                marker {
                                    key: "{tone.name()}",
                                    id: "{frame.marker(tone.name())}",
                                    "viewBox": "0 0 {HEAD_LEN} {HEAD_H}",
                                    "refX": "{HEAD_LEN - 1.0}",
                                    "refY": "{HEAD_H / 2.0}",
                                    "markerWidth": "{HEAD_LEN}",
                                    "markerHeight": "{HEAD_H}",
                                    "markerUnits": "userSpaceOnUse",
                                    orient: "auto",
                                    path {
                                        class: "pk-ag-seq-mark pk-ag-{tone.name()}",
                                        d: "M0 0 L{HEAD_LEN} {HEAD_H / 2.0} L0 {HEAD_H} Z",
                                    }
                                }
                            }
                        }
                        g { "aria-hidden": "true",
                            for head in sequence.lanes.iter() {
                                line {
                                    key: "{head.x_index}",
                                    class: "pk-ag-seq-life",
                                    x1: "{fmt1(frame.x(head.x_index as u32))}",
                                    x2: "{fmt1(frame.x(head.x_index as u32))}",
                                    y1: "0",
                                    y2: "{fmt1(height)}",
                                }
                            }
                        }
                        g {
                            for at in start..end {
                                {row_view(&frame, at)}
                            }
                        }
                    }
                }
            }
            p { id: "{id}-sequence-foot", class: "pk-ag-seq-foot",
                "{sequence.read} events \u{b7} {shown.drawn} rows"
                if sequence.hidden > 0 {
                    " \u{b7} {sequence.hidden} with no agent not drawn"
                }
            }
        }
    }
}
