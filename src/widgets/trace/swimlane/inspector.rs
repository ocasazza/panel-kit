//! The inspector: what one clicked node holds, docked under the body. An item
//! shows its header and its events as a nested [`TraceRail`] whose rows open in
//! place; a fork lane, a collaboration pair and events that belong to no item
//! show their facts. Content comes from `Node::inspect` (a pair from
//! `Node::inspect_pair`); this only paints it.

use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::widgets::trace::facts::{Expansion, Fact, WRAP_COLS};
use panel_kit_core::widgets::trace::rails::{vertical_with, RailOptions};
use panel_kit_core::widgets::trace::swimlane::{Collaboration, Inspection, ItemHeader, Node};
use panel_kit_core::widgets::trace::{Event, SessionTree};

use crate::widgets::trace::rails::TraceRail;

/// Whether the key press dismisses an open inspector: Escape.
pub(super) fn dismisses(event: &KeyboardEvent) -> bool {
    event.key() == Key::Escape
}

/// One fact row: its label and one ellipsised line per pre-wrapped line, each
/// under a `title` with the full text.
fn fact_row(fact: &Fact) -> Element {
    rsx! {
        div { class: "pk-tr-fact", key: "{fact.label}",
            span { class: "pk-tr-fact-label", "{fact.label}" }
            div { class: "pk-tr-fact-value",
                for (at , line) in fact.lines.iter().enumerate() {
                    span { key: "{at}", class: "pk-tr-fact-line", title: "{line}", "{line}" }
                }
            }
        }
    }
}

/// Fact rows on one grid: the label column is as wide as the longest label.
pub(super) fn fact_list(facts: &[Fact]) -> Element {
    let width = facts.iter().map(|f| f.label.len()).max().unwrap_or(0);
    rsx! {
        div { class: "pk-tr-facts", style: "--pk-tr-labels: {width}ch",
            for fact in facts.iter() {
                {fact_row(fact)}
            }
        }
    }
}

/// One header line: a label and a single value.
fn line(label: &'static str, value: String) -> Fact {
    Fact {
        label,
        lines: vec![value],
    }
}

/// The item's header as facts, the status drawn apart with its toned dot.
fn header(head: &ItemHeader) -> Element {
    let mut before = vec![line("item", head.id.clone())];
    if let Some(title) = &head.title {
        before.push(line("title", title.clone()));
    }
    let mut after = vec![
        line("events", head.events.to_string()),
        line("first", head.first.clone()),
        line("last", head.last.clone()),
    ];
    if !head.agents.is_empty() {
        after.push(line("agents", head.agents.join(", ")));
    }
    after.push(line("lanes", head.lanes.to_string()));
    let width = before
        .iter()
        .chain(after.iter())
        .map(|f| f.label.len())
        .chain([6])
        .max()
        .unwrap_or(6);
    let tone = head.tone.name();
    let status = head.status.as_deref().unwrap_or("none");
    rsx! {
        div { class: "pk-tr-facts", style: "--pk-tr-labels: {width}ch",
            for fact in before.iter() {
                {fact_row(fact)}
            }
            div { class: "pk-tr-fact", key: "status",
                span { class: "pk-tr-fact-label", "status" }
                div { class: "pk-tr-fact-value",
                    span { class: "pk-tr-fact-line", title: "{status}",
                        span { class: "pk-tr-ins-dot pk-tr-{tone}", aria_hidden: "true" }
                        "{status}"
                    }
                }
            }
            for fact in after.iter() {
                {fact_row(fact)}
            }
        }
    }
}

/// The nodes' caption: what kind of thing is open.
fn caption(node: &Node) -> &'static str {
    match node {
        Node::Item(_) => "item",
        Node::Event(_) => "event",
        Node::Cluster(_) => "events",
        Node::Lane(_) => "lane",
        Node::Pair(..) => "pair",
    }
}

/// The inspector region of one node. Remount it per node (key on
/// [`Node::key`]): the open rows of its rail are its own and start from the
/// clicked events.
#[component]
pub(super) fn Inspector(
    id: String,
    node: Node,
    events: Rc<Vec<Event>>,
    tree: Option<Rc<SessionTree>>,
    /// The collaboration a pair node is looked up in.
    #[props(default)]
    collab: Option<Rc<Collaboration>>,
    on_close: EventHandler<()>,
) -> Element {
    let content = match (&node, &collab) {
        (Node::Pair(a, b), Some(collab)) => collab
            .pairs
            .iter()
            .find(|pair| pair.a == *a && pair.b == *b)
            .and_then(|pair| node.inspect_pair(pair, WRAP_COLS)),
        _ => node.inspect(&events, tree.as_deref(), WRAP_COLS),
    };
    let mut expanded = use_signal(|| {
        let mut open = Expansion::new();
        if let Some(content) = &content {
            open.open_all(content.open.iter().map(String::as_str));
        }
        open
    });
    let Some(content) = content else {
        return rsx! {};
    };
    let Inspection {
        header: head,
        item,
        tree: item_tree,
        more,
        blocks,
        ..
    } = &content;
    let rail = item.as_ref().map(|item| {
        let open = expanded.read().clone();
        let view = vertical_with(&events, item, item_tree, &open, WRAP_COLS).view(&RailOptions {
            title: "events".to_owned(),
            heading: false,
            dom_id: format!("{id}-ins"),
            on_select: true,
            wrap: WRAP_COLS,
            ..RailOptions::default()
        });
        rsx! {
            div { class: "pk-tr-ins-rail",
                TraceRail {
                    view,
                    expanded: open,
                    on_toggle: move |row: String| {
                        expanded.write().toggle(&row);
                    },
                }
            }
        }
    });
    let kind = caption(&node);
    let name = match &node {
        Node::Item(id) | Node::Event(id) | Node::Lane(id) => id.clone(),
        Node::Cluster(ids) => format!("{} events", ids.len()),
        Node::Pair(a, b) => format!("{a} \u{2194} {b}"),
    };
    rsx! {
        section {
            id: "{id}-inspector",
            class: "pk-tr-ins",
            role: "region",
            aria_label: "inspector",
            "data-node": "{kind}",
            div {
                class: "pk-tr-ins-bar",
                onkeydown: move |event| {
                    if dismisses(&event) {
                        on_close.call(());
                    }
                },
                span { class: "pk-tr-ins-kind", "{kind}" }
                span { class: "pk-tr-ins-name", title: "{name}", "{name}" }
                button {
                    id: "{id}-inspector-close",
                    class: "pk-btn pk-tr-ins-close",
                    r#type: "button",
                    aria_keyshortcuts: "Escape",
                    onclick: move |_| on_close.call(()),
                    "close"
                }
            }
            div { class: "pk-tr-ins-body",
                if let Some(head) = head {
                    {header(head)}
                }
                {rail}
                if *more > 0 {
                    p { class: "pk-tr-ins-more", "+{more} more" }
                }
                for block in blocks.iter() {
                    div { class: "pk-tr-ins-block", key: "{block.label}",
                        p { class: "pk-tr-ins-cap", title: "{block.label}", "{block.label}" }
                        {fact_list(&block.facts)}
                    }
                }
            }
        }
    }
}
