//! LANES body: one flat lane per agent, then the events no agent owns behind
//! a toggle, grouped by source. A node is a disclosure button: it opens the
//! event's facts under its row, on the same grid as a rail row's.

use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::widgets::event_graph::facts::{event_facts, Expansion, WRAP_COLS};
use panel_kit_core::widgets::event_graph::query::GraphAction;
use panel_kit_core::widgets::event_graph::swimlane::{slug, ActivityLane, ActivityNode, Split};
use panel_kit_core::widgets::event_graph::Event;

use super::inspector::fact_list;

/// Nodes a lane draws, newest first kept; older ones collapse to `+N earlier`.
const LANE_NODE_CAP: usize = 100;

/// The LANES mode body.
#[component]
pub(super) fn LanesBody(
    id: String,
    split: Rc<Split>,
    unattributed_open: bool,
    on_action: EventHandler<GraphAction>,
    events: Rc<Vec<Event>>,
    open: Expansion,
    on_toggle: EventHandler<String>,
) -> Element {
    if split.is_empty() {
        return rsx! {
            p { id: "{id}-lanes-empty", class: "pk-ag-empty", "nothing recorded in this window" }
        };
    }
    let unattributed_class = if unattributed_open {
        "pk-ag-unattributed on"
    } else {
        "pk-ag-unattributed"
    };
    let toggle_class = if unattributed_open {
        "pk-ag-unattributed-toggle on"
    } else {
        "pk-ag-unattributed-toggle"
    };
    let caret = if unattributed_open { "▾" } else { "▸" };
    rsx! {
        if !split.lanes.is_empty() {
            ol { id: "{id}-lanes", class: "pk-ag-lane-list pk-ag-lanes", aria_label: "lanes",
                for lane in split.lanes.iter() {
                    {lane_item(&id, lane, &events, &open, on_toggle)}
                }
            }
        }
        if !split.unattributed.is_empty() {
            section { id: "{id}-unattributed", class: "{unattributed_class}",
                button {
                    id: "{id}-unattributed-toggle",
                    class: "{toggle_class}",
                    r#type: "button",
                    aria_expanded: "{unattributed_open}",
                    aria_controls: "{id}-unattributed-lanes",
                    onclick: move |_| on_action.call(GraphAction::ToggleUnattributed),
                    span { class: "pk-ag-caret", aria_hidden: "true", "{caret}" }
                    span { class: "pk-ag-tl-label", "unattributed" }
                    span { id: "{id}-unattributed-count", class: "pk-ag-count",
                        "{split.unattributed_count()}"
                    }
                }
                if unattributed_open {
                    ol {
                        id: "{id}-unattributed-lanes",
                        class: "pk-ag-lane-list",
                        aria_label: "unattributed lanes",
                        for lane in split.unattributed.iter() {
                            {lane_item(&id, lane, &events, &open, on_toggle)}
                        }
                    }
                }
            }
        }
    }
}

/// One lane: its head and its newest [`LANE_NODE_CAP`] nodes, oldest first.
fn lane_item(
    id: &str,
    lane: &ActivityLane,
    events: &[Event],
    open: &Expansion,
    on_toggle: EventHandler<String>,
) -> Element {
    let hidden = lane.nodes.len().saturating_sub(LANE_NODE_CAP);
    let shown = &lane.nodes[hidden..];
    rsx! {
        li {
            id: "{id}-lane-{lane.id}",
            class: "pk-ag-lane",
            "data-lane": "{lane.id}",
            div { class: "pk-ag-lane-head",
                span { class: "pk-ag-lane-label", "{lane.label}" }
                span { class: "pk-ag-count", "{lane.nodes.len()}" }
            }
            ol { class: "pk-ag-nodes",
                if hidden > 0 {
                    li { id: "{id}-lane-{lane.id}-earlier", class: "pk-ag-earlier", "+{hidden} earlier" }
                }
                for node in shown.iter() {
                    {node_item(id, node, events, open, on_toggle)}
                }
            }
        }
    }
}

/// One node: the disclosure button, and its facts under it while open.
fn node_item(
    id: &str,
    node: &ActivityNode,
    events: &[Event],
    open: &Expansion,
    on_toggle: EventHandler<String>,
) -> Element {
    let is_open = open.is_open(&node.id);
    let facts = if is_open {
        events
            .iter()
            .find(|event| slug(&event.id) == node.id)
            .map(|event| event_facts(event, WRAP_COLS))
    } else {
        None
    };
    let caret = if is_open { "\u{25be}" } else { "\u{25b8}" };
    let class = if is_open {
        format!("pk-ag-node pk-ag-{} open", node.kind.tone().name())
    } else {
        format!("pk-ag-node pk-ag-{}", node.kind.tone().name())
    };
    let toggle = node.id.clone();
    rsx! {
        li {
            key: "{node.id}",
            id: "{id}-node-{node.id}",
            class: "{class}",
            "data-kind": "{node.kind}",
            title: "{node.detail}",
            button {
                class: "pk-ag-node-hit",
                r#type: "button",
                aria_expanded: "{is_open}",
                aria_controls: "{id}-node-{node.id}-facts",
                onclick: move |_| on_toggle.call(toggle.clone()),
                span { class: "pk-ag-caret", aria_hidden: "true", "{caret}" }
                span { class: "pk-ag-dot", aria_hidden: "true" }
                span { class: "pk-ag-caption", "{node.label}" }
            }
            if let Some(facts) = facts {
                div {
                    id: "{id}-node-{node.id}-facts",
                    class: "pk-ag-node-facts",
                    role: "group",
                    aria_label: "{node.label} details",
                    {fact_list(&facts)}
                }
            }
        }
    }
}
