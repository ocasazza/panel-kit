//! Interactive semantic table: sorting, filtering, focus and selection over
//! one borrowed model, with the host owning every piece of state.
//!
//! Run with: `dx serve --example table --platform web`
//!
//! What it demonstrates:
//! - Multi-key sort cycled from the header (descending, ascending, off) with
//!   numeric columns compared as numbers (`10` after `9`, never before).
//! - A text filter over every cell, tag chips combined All/Any, and a
//!   filtered-empty state that says how many rows exist.
//! - Focus (j/k, arrows, Home/End, PageUp/PageDown) drawn as an outline and
//!   `aria-current`, selection toggled by Space or a click, `a` selects all.
//! - Stable ids: the focus and selection follow a row across a re-sort, which
//!   an index-based table cannot do.
//! - The state as JSON, the shape a URL or localStorage would carry.

use dioxus::prelude::*;
use panel_kit::widgets::table;
use panel_kit::CSS;
use panel_kit_core::widgets::table::{
    visible, ColumnWidth, Density, TableCell, TableColumn, TableCommand, TableModel, TableRow,
    TableState, TableView, TagMode, TextAlign,
};

const DEMO_CSS: &str = "
body { overflow:auto !important; }
.demo { max-width:64rem; margin:0 auto; padding:2rem; }
.demo h1 { margin:0 0 .3rem; font-size:.8rem; }
.demo p.hint { margin:0 0 1rem; color:var(--dim); font-size:.72rem; }
.controls { display:flex; align-items:center; gap:.6rem; margin-bottom:.6rem; flex-wrap:wrap; }
.controls input[type=text] { background:var(--bg); color:var(--fg); border:1px solid var(--line2);
  border-radius:3px; padding:.2rem .45rem; font-family:var(--mono); font-size:.72rem; width:16rem; }
.controls button { background:var(--bg); color:var(--dim); border:1px solid var(--line2);
  border-radius:3px; padding:.15rem .5rem; font-size:.68rem; cursor:pointer; }
.controls button.on { color:var(--fg); border-color:var(--accent); }
.state { margin-top:.7rem; color:var(--dim); font-size:.68rem; white-space:pre-wrap;
  font-family:var(--mono); }
";

const TAGS: &[&str] = &["p1", "seat", "ingest"];

fn model() -> TableModel {
    let columns = vec![
        TableColumn {
            key: "id".into(),
            title: "Item".into(),
            width: ColumnWidth::Fixed { value: 10 },
            align: TextAlign::Left,
        },
        TableColumn {
            key: "priority".into(),
            title: "P".into(),
            width: ColumnWidth::Fixed { value: 4 },
            align: TextAlign::Left,
        },
        TableColumn {
            key: "title".into(),
            title: "Title".into(),
            width: ColumnWidth::Flex { weight: 6 },
            align: TextAlign::Left,
        },
        TableColumn {
            key: "tokens".into(),
            title: "Tokens".into(),
            width: ColumnWidth::Fixed { value: 9 },
            align: TextAlign::Right,
        },
        TableColumn {
            key: "state".into(),
            title: "State".into(),
            width: ColumnWidth::Fixed { value: 10 },
            align: TextAlign::Left,
        },
    ];

    type DemoRow<'a> = (&'a str, &'a str, &'a str, u32, &'a str, &'a [&'a str]);
    let rows: [DemoRow; 6] = [
        (
            "imp-9",
            "P2",
            "Trace the ingest path end to end",
            940,
            "open",
            &["ingest"],
        ),
        (
            "imp-10",
            "P1",
            "Seat restart: keep the warm weights",
            12_400,
            "claimed",
            &["p1", "seat"],
        ),
        (
            "imp-3",
            "P2",
            "The ledger prunes a done item twice",
            310,
            "review",
            &["ingest"],
        ),
        (
            "imp-1",
            "P0",
            "Quorum check steps down on one missed beat",
            88_150,
            "done",
            &["p1"],
        ),
        (
            "imp-42",
            "P3",
            "Dock restore forgets the section it held",
            75,
            "open",
            &["seat"],
        ),
        (
            "imp-7",
            "P1",
            "Model health lag hides a demotion",
            2_050,
            "blocked",
            &["p1", "ingest"],
        ),
    ];

    let rows = rows
        .into_iter()
        .map(|(id, priority, title, tokens, state, tags)| TableRow {
            id: id.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            cells: vec![
                TableCell::Text(id.into()),
                TableCell::Text(priority.into()),
                TableCell::Text(title.into()),
                TableCell::Meter {
                    ratio: f64::from(tokens) / 100_000.0,
                    text: tokens.to_string(),
                    color: Some((120, 160, 255)),
                },
                TableCell::Status {
                    label: state.into(),
                    color: match state {
                        "open" => (120, 160, 255),
                        "claimed" => (255, 189, 46),
                        "review" => (200, 120, 255),
                        "blocked" => (229, 72, 77),
                        _ => (39, 201, 63),
                    },
                },
            ],
        })
        .collect();

    TableModel { columns, rows }
}

/// Whether the browser's focus is in a text field: the demo owns one global
/// key handler and must not turn typed text into table shortcuts.
fn typing() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        return web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.active_element())
            .map(|element| {
                let tag = element.tag_name();
                tag == "INPUT" || tag == "TEXTAREA" || element.has_attribute("contenteditable")
            })
            .unwrap_or(false);
    }
    #[cfg(not(target_arch = "wasm32"))]
    false
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let model = use_hook(model);
    let mut state = use_signal(TableState::default);
    let mut scratch: Signal<Vec<usize>> = use_signal(Vec::new);

    let ordered = {
        let mut buffer = scratch.write();
        visible(&model, &state.read(), &mut buffer);
        buffer.clone()
    };
    let shown: Vec<&TableRow> = ordered.iter().map(|index| &model.rows[*index]).collect();
    let ids: Vec<String> = shown.iter().map(|row| row.id.clone()).collect();

    let on_command = {
        let ids = ids.clone();
        EventHandler::new(move |command: TableCommand| {
            let mut next = state.read().clone();
            if next.apply(&command, &ids) {
                state.set(next);
            }
        })
    };

    let on_key = {
        let ids = ids.clone();
        move |event: KeyboardEvent| {
            // A keystroke that lands in the filter box is text, not a shortcut.
            if typing() {
                return;
            }
            let character = match event.key() {
                Key::Character(text) => Some(text.to_string()),
                _ => None,
            };
            let command = match (event.key(), character.as_deref()) {
                (_, Some("j")) => Some(TableCommand::Down),
                (_, Some("k")) => Some(TableCommand::Up),
                (_, Some(" ")) => Some(TableCommand::ToggleSelected),
                (_, Some("a")) => Some(TableCommand::ToggleSelectAll),
                (Key::ArrowDown, _) => Some(TableCommand::Down),
                (Key::ArrowUp, _) => Some(TableCommand::Up),
                (Key::PageDown, _) => Some(TableCommand::PageDown(8)),
                (Key::PageUp, _) => Some(TableCommand::PageUp(8)),
                (Key::Home, _) => Some(TableCommand::FocusFirst),
                (Key::End, _) => Some(TableCommand::FocusLast),
                _ => None,
            };
            if let Some(command) = command {
                event.prevent_default();
                let mut next = state.read().clone();
                if next.apply(&command, &ids) {
                    state.set(next);
                }
            }
        }
    };

    let query = state.read().filter.query.clone();
    let tag_mode = state.read().filter.tag_mode;
    let density = state.read().density;
    let tags_on: Vec<String> = state.read().filter.tags.clone();
    let selected = state.read().selected.len();

    let view = TableView {
        columns: &model.columns,
        rows: &shown,
        state: &state.read(),
        total: model.rows.len(),
    };

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        main { class: "demo", tabindex: "0", onkeydown: on_key,
            h1 { "semantic table · sort, filter, focus, select" }
            p { class: "hint",
                "click a header to sort (▼ desc, ▲ asc, again to clear) · j/k or arrows move focus · space selects · a selects all · type to filter"
            }
            div { class: "controls",
                input {
                    r#type: "text",
                    placeholder: "filter every cell…",
                    value: "{query}",
                    oninput: move |event| {
                        let mut next = state.read().clone();
                        next.filter.query = event.value();
                        state.set(next);
                    },
                }
                for tag in TAGS.iter().copied() {
                    {
                        let on = tags_on.iter().any(|t| t == tag);
                        rsx! {
                            button {
                                class: if on { "on" } else { "" },
                                onclick: move |_| {
                                    let mut next = state.read().clone();
                                    match next.filter.tags.iter().position(|t| t == tag) {
                                        Some(index) => {
                                            next.filter.tags.remove(index);
                                        }
                                        None => next.filter.tags.push(tag.to_string()),
                                    }
                                    state.set(next);
                                },
                                "#{tag}"
                            }
                        }
                    }
                }
                button {
                    onclick: move |_| {
                        let mut next = state.read().clone();
                        next.filter.tag_mode = match next.filter.tag_mode {
                            TagMode::All => TagMode::Any,
                            TagMode::Any => TagMode::All,
                        };
                        state.set(next);
                    },
                    match tag_mode {
                        TagMode::All => "all tags",
                        TagMode::Any => "any tag",
                    }
                }
                button {
                    onclick: move |_| {
                        let mut next = state.read().clone();
                        next.density = match next.density {
                            Density::Comfortable => Density::Dense,
                            Density::Dense => Density::Comfortable,
                        };
                        state.set(next);
                    },
                    match density {
                        Density::Comfortable => "dense",
                        Density::Dense => "comfy",
                    }
                }
                button {
                    onclick: move |_| state.set(TableState::default()),
                    "reset"
                }
            }
            {table::table(view, Some(on_command))}
            div { class: "state",
                "{selected} selected · {shown.len()} of {model.rows.len()} shown"
                "\n{serde_json::to_string_pretty(&*state.read()).unwrap_or_default()}"
            }
        }
    }
}
