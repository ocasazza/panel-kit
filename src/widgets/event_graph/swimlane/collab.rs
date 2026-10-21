//! COLLAB body: who worked with whom, and the threads of items two or more
//! agents touched.

use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::widgets::event_graph::swimlane::{slug, Collaboration, Pair, Thread};
use panel_kit_core::widgets::event_graph::time::{format_iso8601, format_step_time};

/// The COLLAB mode body.
#[component]
pub(super) fn CollabBody(
    id: String,
    collab: Rc<Collaboration>,
    selected: Option<String>,
    on_select: EventHandler<String>,
) -> Element {
    let selected = selected.as_deref();
    rsx! {
        section { id: "{id}-collaboration", class: "pk-ag-collab", aria_label: "collaboration",
            if collab.pairs.is_empty() {
                p { id: "{id}-collaboration-empty", class: "pk-ag-empty",
                    "no two agents worked the same item"
                }
            } else {
                table { id: "{id}-collaboration-pairs", class: "pk-ag-pairs",
                    thead {
                        tr {
                            th { scope: "col", "pair" }
                            th { scope: "col", "shared items" }
                            th { scope: "col", "notes" }
                            th { scope: "col", "reviews" }
                            th { scope: "col", "handoffs" }
                            th { scope: "col", "branches" }
                            th { scope: "col", "co-commits" }
                        }
                    }
                    tbody {
                        for pair in collab.pairs.iter() {
                            {pair_row(&id, pair, selected, on_select)}
                        }
                    }
                }
            }
            if !collab.threads.is_empty() {
                h3 { class: "pk-ag-threads-head", "threads" }
                ol { id: "{id}-collaboration-threads", class: "pk-ag-threads",
                    for thread in collab.threads.iter() {
                        {thread_item(&id, thread, selected, on_select)}
                    }
                }
            }
        }
    }
}

/// A button that selects `item`; lit while it is the selected one.
fn item_link(item: &str, selected: Option<&str>, on_select: EventHandler<String>) -> Element {
    let on = selected == Some(item);
    let class = if on { "pk-ag-link on" } else { "pk-ag-link" };
    let target = item.to_string();
    rsx! {
        button {
            class: "{class}",
            r#type: "button",
            aria_pressed: "{on}",
            onclick: move |_| on_select.call(target.clone()),
            "{item}"
        }
    }
}

fn pair_row(
    id: &str,
    pair: &Pair,
    selected: Option<&str>,
    on_select: EventHandler<String>,
) -> Element {
    rsx! {
        tr { id: "{id}-collaboration-pair-{slug(&pair.a)}-{slug(&pair.b)}",
            th { scope: "row", class: "pk-ag-pair-names",
                "{pair.a} "
                span { class: "pk-ag-tee", "↔" }
                " {pair.b}"
            }
            td { class: "pk-ag-pair-items",
                for item in pair.shared_items.iter() {
                    {item_link(item, selected, on_select)}
                }
            }
            td {
                span { class: "pk-ag-a", "{pair.notes_ab}" }
                " → "
                span { class: "pk-ag-b", "{pair.notes_ba}" }
            }
            td { "{pair.reviews}" }
            td { class: "pk-ag-pair-handoffs",
                if pair.handoffs.is_empty() {
                    span { class: "pk-ag-faint", "0" }
                }
                for handoff in pair.handoffs.iter() {
                    span {
                        title: "{handoff.from} -> {handoff.to} on {handoff.item_id} at {format_iso8601(handoff.at)}",
                        "{handoff.from} → {handoff.to}"
                    }
                }
            }
            td { class: "pk-ag-pair-branches",
                for branch in pair.branches.iter() {
                    span { "{branch}" }
                }
            }
            td { "{pair.co_commits}" }
        }
    }
}

fn thread_item(
    id: &str,
    thread: &Thread,
    selected: Option<&str>,
    on_select: EventHandler<String>,
) -> Element {
    let mut class = String::from("pk-ag-thread");
    if thread.stale {
        class.push_str(" pk-ag-stale");
    }
    if selected == Some(thread.item_id.as_str()) {
        class.push_str(" on");
    }
    let stale = thread.stale.then(|| {
        let agent = thread.stale_agent.as_deref().unwrap_or("");
        match thread.stale_since {
            Some(at) => format!("⚠ stale holder {agent} since {}", format_iso8601(at)),
            None => format!("⚠ stale holder {agent}"),
        }
    });
    rsx! {
        li {
            id: "{id}-collaboration-thread-{slug(&thread.item_id)}",
            class: "{class}",
            title: thread.title.as_deref(),
            div { class: "pk-ag-thread-head",
                {item_link(&thread.item_id, selected, on_select)}
                if let Some(title) = thread.title.as_deref() {
                    span { class: "pk-ag-thread-title", "{title}" }
                }
                if let Some(stale) = stale {
                    span { class: "pk-ag-warn", "{stale}" }
                }
            }
            ol { class: "pk-ag-steps",
                for step in thread.steps.iter() {
                    li {
                        time { datetime: "{format_iso8601(step.at)}", "{format_step_time(step.at)}" }
                        span { class: "pk-ag-step-agent", {step.agent.clone().unwrap_or_default()} }
                        span { class: "pk-ag-step-label", "{step.label}" }
                    }
                }
            }
        }
    }
}
