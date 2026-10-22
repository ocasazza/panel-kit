//! The toolbar pieces: mode chips, the query line with its datalist and TREE
//! toggle, and the row of active query tokens.

use std::collections::BTreeSet;

use dioxus::prelude::*;
use panel_kit_core::widgets::trace::query::{
    Completion, GraphAction, LanePick, ModeChip, QueryToken,
};
use panel_kit_core::widgets::trace::swimlane::{Sequence, BUSIEST_LANES};

use super::shared::Shared;

/// Lanes the lane menu lists at once; its filter reaches the rest.
const MENU_ROWS: usize = 100;

/// The GRAPH / LANES / COLLAB chips; the current mode's is pressed and inverted.
pub(super) fn modes(id: &str, chips: &[ModeChip], on_action: EventHandler<GraphAction>) -> Element {
    rsx! {
        div { class: "pk-tr-modes", role: "group", aria_label: "view mode",
            for chip in chips.iter() {
                {
                    let token = chip.mode.token();
                    rsx! {
                        button {
                            key: "{token}",
                            id: "{id}-modes-{token}",
                            r#type: "button",
                            class: if chip.pressed { "pk-tr-chip on" } else { "pk-tr-chip" },
                            aria_pressed: "{chip.pressed}",
                            aria_label: "{chip.aria_label}",
                            onclick: move |_| on_action.call(GraphAction::SetMode(token.to_string())),
                            span { class: "pk-tr-chip-dot", aria_hidden: "true" }
                            "{chip.label}"
                        }
                    }
                }
            }
        }
    }
}

/// The ACTIVITY chip of the LANES mode: plain activity as dots on the
/// lifelines, with the count of such events in the window.
pub(super) fn activity_chip(
    id: &str,
    on: bool,
    quiet: usize,
    on_toggle: EventHandler<bool>,
) -> Element {
    rsx! {
        button {
            id: "{id}-activity",
            r#type: "button",
            class: if on { "pk-tr-chip on" } else { "pk-tr-chip" },
            aria_pressed: "{on}",
            title: "plain activity as dots on the lifelines",
            onclick: move |_| on_toggle.call(!on),
            span { class: "pk-tr-chip-dot", aria_hidden: "true" }
            "activity"
            span { class: "pk-tr-count", "{quiet}" }
        }
    }
}

/// The lane menu of the LANES mode: a quiet `lanes N` button over a checklist
/// of every lane of the window, busiest first, with a filter box and
/// `busiest 8` / `all` / `none`. A lane that is not checked is folded into the
/// narrow `+N` lane. The lane the diagram is filtered to is framed.
pub(super) fn lane_menu(
    id: &str,
    sequence: &Sequence,
    filter: &str,
    focus: Option<&str>,
    on_action: EventHandler<GraphAction>,
) -> Element {
    let total = sequence.roster.len();
    let drawn = sequence.roster.iter().filter(|lane| lane.shown).count();
    let shown: BTreeSet<String> = sequence
        .roster
        .iter()
        .filter(|lane| lane.shown)
        .map(|lane| lane.agent.clone())
        .collect();
    let needle = filter.trim().to_lowercase();
    let matching: Vec<_> = sequence
        .roster
        .iter()
        .filter(|lane| needle.is_empty() || lane.agent.to_lowercase().contains(&needle))
        .collect();
    let more = matching.len().saturating_sub(MENU_ROWS);
    let busiest: BTreeSet<String> = sequence
        .roster
        .iter()
        .take(BUSIEST_LANES)
        .map(|lane| lane.agent.clone())
        .collect();
    let typed = filter.to_string();
    rsx! {
        details { id: "{id}-lanes-menu", class: "pk-dt-menu pk-tr-lanes-menu", "data-dt-menu": "lanes",
            summary {
                id: "{id}-lanes-button",
                class: "pk-btn",
                title: "{drawn} of {total} lanes drawn",
                "lanes {total} \u{25be}"
            }
            div { class: "pk-dt-menu-body", role: "group", aria_label: "lanes",
                div { class: "pk-tr-lanes-tools",
                    input {
                        id: "{id}-lanes-filter",
                        class: "pk-tr-lanes-filter",
                        r#type: "search",
                        name: "lane",
                        value: "{typed}",
                        placeholder: "filter lanes",
                        aria_label: "filter lanes",
                        autocomplete: "off",
                        oninput: move |event| on_action.call(GraphAction::LaneFilter(event.value())),
                    }
                    button {
                        id: "{id}-lanes-busiest",
                        r#type: "button",
                        class: "pk-btn",
                        onclick: move |_| on_action.call(GraphAction::PickLanes(LanePick::Only(busiest.clone()))),
                        "busiest {BUSIEST_LANES}"
                    }
                    button {
                        id: "{id}-lanes-all",
                        r#type: "button",
                        class: "pk-btn",
                        onclick: move |_| on_action.call(GraphAction::PickLanes(LanePick::All)),
                        "all"
                    }
                    button {
                        id: "{id}-lanes-none",
                        r#type: "button",
                        class: "pk-btn",
                        onclick: move |_| on_action.call(GraphAction::PickLanes(LanePick::Only(BTreeSet::new()))),
                        "none"
                    }
                }
                for lane in matching.into_iter().take(MENU_ROWS) {
                    {
                        let agent = lane.agent.clone();
                        let mut next = shown.clone();
                        if lane.shown {
                            next.remove(&agent);
                        } else {
                            next.insert(agent.clone());
                        }
                        let pick = if next.len() == total { LanePick::All } else { LanePick::Only(next) };
                        let framed = lane.shown && focus == Some(lane.agent.as_str());
                        rsx! {
                            div {
                                key: "{lane.agent}",
                                class: if framed { "pk-dt-menu-row pk-tr-lane-row on" } else { "pk-dt-menu-row pk-tr-lane-row" },
                                "data-lane": "{lane.agent}",
                                label { class: "pk-dt-check",
                                    input {
                                        r#type: "checkbox",
                                        checked: lane.shown,
                                        aria_label: "draw {lane.agent}",
                                        onclick: move |_| on_action.call(GraphAction::PickLanes(pick.clone())),
                                    }
                                    span { class: "pk-tr-lane-name", title: "{lane.agent}", " {lane.agent}" }
                                }
                                span { class: "pk-tr-count", "{lane.count}" }
                            }
                        }
                    }
                }
                if more > 0 {
                    p { class: "pk-tr-lanes-more", "+{more} more: filter to find them" }
                }
            }
        }
    }
}

/// One removable chip per active query token; clicking one submits the line
/// without it. Nothing is drawn while no token is active.
pub(super) fn tokens(
    id: &str,
    tokens: &[QueryToken],
    on_action: EventHandler<GraphAction>,
) -> Element {
    if tokens.is_empty() {
        return rsx! {};
    }
    rsx! {
        ul { id: "{id}-filters-tokens", class: "pk-tr-tokens", aria_label: "active filters",
            for token in tokens.iter() {
                {
                    let remove = token.remove.clone();
                    rsx! {
                        li { key: "{token.label}",
                            button {
                                r#type: "button",
                                class: "pk-tr-token on",
                                title: "remove {token.label}",
                                aria_label: "remove {token.label}",
                                onclick: move |_| on_action.call(GraphAction::Query(remove.clone())),
                                span { "{token.label}" }
                                span { class: "pk-tr-token-x", aria_hidden: "true", "×" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The datalist behind the query box, apart from the box so typing never
/// re-diffs thousands of options.
#[component]
fn Options(id: String, completions: Shared<Vec<Completion>>) -> Element {
    rsx! {
        datalist { id: "{id}-filters-options",
            for completion in completions.iter() {
                option { value: "{completion.value}", "{completion.label}" }
            }
        }
    }
}

/// The query line: the text box, its completions, the TREE toggle and the
/// tokens the last line ignored.
///
/// The box shows the state's canonical line until it is focused; from then on
/// it shows the draft, which Enter, a datalist pick or leaving the box submits.
#[component]
pub(super) fn QueryLine(
    id: String,
    line: String,
    tree: bool,
    errors: Option<String>,
    completions: Shared<Vec<Completion>>,
    on_action: EventHandler<GraphAction>,
) -> Element {
    let mut draft = use_signal(String::new);
    let mut editing = use_signal(|| false);
    let mut sent = use_signal(|| None::<String>);

    // The same text is sent once per edit: Enter and the change that follows
    // it are one submission.
    let send = use_callback(move |text: String| {
        if sent.peek().as_deref() == Some(text.as_str()) {
            return;
        }
        sent.set(Some(text.clone()));
        on_action.call(GraphAction::Query(text));
    });

    let shown = if *editing.read() {
        draft.read().clone()
    } else {
        line.clone()
    };
    let typed = shown.clone();
    let restore = line.clone();
    let seed = line;

    rsx! {
        form {
            id: "{id}-filters",
            class: "pk-tr-query",
            role: "search",
            onsubmit: move |event| {
                event.prevent_default();
                let text = if *editing.peek() { draft.peek().clone() } else { typed.clone() };
                send.call(text);
            },
            input {
                id: "{id}-filters-input",
                class: if *editing.read() { "pk-tr-query-input on" } else { "pk-tr-query-input" },
                r#type: "text",
                name: "q",
                value: "{shown}",
                list: "{id}-filters-options",
                placeholder: "agent:worker item:imp-16 kind:commit range:7d",
                aria_label: "filter activity",
                autocomplete: "off",
                spellcheck: "false",
                onfocus: move |_| {
                    draft.set(seed.clone());
                    sent.set(None);
                    editing.set(true);
                },
                onblur: move |_| editing.set(false),
                oninput: move |event| {
                    draft.set(event.value());
                    sent.set(None);
                },
                onchange: move |event| send.call(event.value()),
                onkeydown: move |event| match event.key() {
                    Key::Enter => {
                        event.prevent_default();
                        send.call(draft.peek().clone());
                    }
                    Key::Escape => draft.set(restore.clone()),
                    _ => {}
                },
            }
            Options { id: id.clone(), completions }
            button {
                id: "{id}-filters-tree",
                r#type: "button",
                class: if tree { "pk-tr-tree on" } else { "pk-tr-tree" },
                aria_pressed: "{tree}",
                title: "draw forks as child lanes",
                onclick: move |_| on_action.call(GraphAction::ToggleTree),
                "tree"
            }
            if let Some(text) = errors {
                span { id: "{id}-filters-errors", class: "pk-tr-errors", role: "status", "{text}" }
            }
        }
    }
}
