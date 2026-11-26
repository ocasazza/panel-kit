//! The LANES body as a dense timeline: one row per agent, its label beside a
//! wrapping line of `[dot][caption]` nodes joined by a 1px rule, the newest
//! [`LIMIT`] drawn and the rest folded into `+N earlier`. A node opens its
//! event in the inspector.

use dioxus::prelude::*;
use panel_kit_core::widgets::trace::rails::timeline::{
    timeline_view, FlatLane, TimelineInput, TimelineOptions,
};
use panel_kit_core::widgets::trace::swimlane::Node;

use super::shared::Shared;

/// The newest nodes a lane draws.
pub const LIMIT: usize = 100;

/// Paints `lanes` under `{id}-timeline`.
#[component]
pub fn TimelineBody(
    id: String,
    lanes: Shared<Vec<FlatLane>>,
    picked: Option<String>,
    on_open: EventHandler<Node>,
) -> Element {
    let view = timeline_view(
        TimelineInput::Lanes(&lanes),
        &TimelineOptions {
            dom_id: format!("{id}-timeline"),
            title: "lanes".to_owned(),
            empty: "nothing recorded in this window".to_owned(),
            heading: false,
            on_select: true,
            limit: Some(LIMIT),
            current: picked,
            ..TimelineOptions::default()
        },
    );
    rsx! {
        section { id: "{view.dom_id}", class: "pk-tl", aria_label: "{view.title}",
            if let Some(empty) = &view.empty {
                p { id: "{view.dom_id}-empty", class: "pk-tl-empty", "{empty}" }
            }
            if !view.lanes.is_empty() {
                ol { id: "{view.dom_id}-lanes", class: "pk-tl-lanes",
                    for lane in view.lanes {
                        li { key: "{lane.dom_id}", id: "{lane.dom_id}", class: "pk-tl-lane",
                            span { class: "pk-tl-label", title: "{lane.label}", "{lane.label}" }
                            span { class: "pk-tl-nodes",
                                if let (Some(text), Some(earlier_id)) = (&lane.earlier, &lane.earlier_dom_id) {
                                    span { id: "{earlier_id}", class: "pk-tl-earlier", "{text}" }
                                }
                                for node in lane.nodes {
                                    button {
                                        key: "{node.dom_id}",
                                        id: "{node.dom_id}",
                                        r#type: "button",
                                        class: if node.current { "pk-tl-node pk-tl-current" } else { "pk-tl-node" },
                                        title: "{node.title}",
                                        "data-kind": "{node.kind}",
                                        "data-tone": "{node.tone.name()}",
                                        "data-inferred": node.inferred.then_some("true"),
                                        onclick: {
                                            let event = node.id.clone();
                                            move |_| on_open.call(Node::Event(event.clone()))
                                        },
                                        span { class: "pk-tl-dot", aria_hidden: "true" }
                                        span { class: "pk-tl-caption", "{node.caption}" }
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
