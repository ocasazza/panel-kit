//! The toolbar pieces: mode chips, the query line with its datalist and TREE
//! toggle, and the row of active query tokens.

use dioxus::prelude::*;
use panel_kit_core::widgets::event_graph::query::{Completion, GraphAction, ModeChip, QueryToken};

use super::shared::Shared;

/// The GRAPH / LANES / COLLAB chips; the current mode's is pressed and inverted.
pub(super) fn modes(id: &str, chips: &[ModeChip], on_action: EventHandler<GraphAction>) -> Element {
    rsx! {
        div { class: "pk-ag-modes", role: "group", aria_label: "view mode",
            for chip in chips.iter() {
                {
                    let token = chip.mode.token();
                    rsx! {
                        button {
                            key: "{token}",
                            id: "{id}-modes-{token}",
                            r#type: "button",
                            class: if chip.pressed { "pk-ag-chip on" } else { "pk-ag-chip" },
                            aria_pressed: "{chip.pressed}",
                            aria_label: "{chip.aria_label}",
                            onclick: move |_| on_action.call(GraphAction::SetMode(token.to_string())),
                            span { class: "pk-ag-chip-dot", aria_hidden: "true" }
                            "{chip.label}"
                        }
                    }
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
        ul { id: "{id}-filters-tokens", class: "pk-ag-tokens", aria_label: "active filters",
            for token in tokens.iter() {
                {
                    let remove = token.remove.clone();
                    rsx! {
                        li { key: "{token.label}",
                            button {
                                r#type: "button",
                                class: "pk-ag-token on",
                                title: "remove {token.label}",
                                aria_label: "remove {token.label}",
                                onclick: move |_| on_action.call(GraphAction::Query(remove.clone())),
                                span { "{token.label}" }
                                span { class: "pk-ag-token-x", aria_hidden: "true", "×" }
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
            class: "pk-ag-query",
            role: "search",
            onsubmit: move |event| {
                event.prevent_default();
                let text = if *editing.peek() { draft.peek().clone() } else { typed.clone() };
                send.call(text);
            },
            input {
                id: "{id}-filters-input",
                class: if *editing.read() { "pk-ag-query-input on" } else { "pk-ag-query-input" },
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
                class: if tree { "pk-ag-tree on" } else { "pk-ag-tree" },
                aria_pressed: "{tree}",
                title: "draw forks as child lanes",
                onclick: move |_| on_action.call(GraphAction::ToggleTree),
                "tree"
            }
            if let Some(text) = errors {
                span { id: "{id}-filters-errors", class: "pk-ag-errors", role: "status", "{text}" }
            }
        }
    }
}
