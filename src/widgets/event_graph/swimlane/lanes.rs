//! LANES body: one flat lane per agent, then the events no agent owns behind
//! a toggle, grouped by source.

use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::widgets::event_graph::query::GraphAction;
use panel_kit_core::widgets::event_graph::swimlane::{ActivityLane, Split};

/// Nodes a lane draws, newest first kept; older ones collapse to `+N earlier`.
const LANE_NODE_CAP: usize = 100;

/// The LANES mode body.
#[component]
pub(super) fn LanesBody(
    id: String,
    split: Rc<Split>,
    unattributed_open: bool,
    on_action: EventHandler<GraphAction>,
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
                    {lane_item(&id, lane)}
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
                            {lane_item(&id, lane)}
                        }
                    }
                }
            }
        }
    }
}

/// One lane: its head and its newest [`LANE_NODE_CAP`] nodes, oldest first.
fn lane_item(id: &str, lane: &ActivityLane) -> Element {
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
                    li {
                        key: "{node.id}",
                        id: "{id}-node-{node.id}",
                        class: "pk-ag-node pk-ag-{node.kind.tone().name()}",
                        "data-kind": "{node.kind}",
                        title: "{node.detail}",
                        span { class: "pk-ag-dot", aria_hidden: "true" }
                        span { class: "pk-ag-caption", "{node.label}" }
                    }
                }
            }
        }
    }
}
