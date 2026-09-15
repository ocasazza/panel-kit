//! Monaco editor workspace demo — reactive editor APIs inside draggable
//! panel-kit chrome.
//!
//! Run with: `dx serve --example editor --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`).
//!
//! Asset prerequisite: the editor loads its vendored bundle from
//! `assets/vendor/monaco-editor-0.56.0/` relative to the served root (see
//! `panel_kit::editor::MONACO_ASSET_DIR`). A consuming app must copy that
//! directory into its served assets or call
//! `panel_kit::editor::set_monaco_asset_base` before first mount.
//!
//! What it demonstrates:
//! - Monaco lives directly in a V2-persisted workspace panel. Drag another
//!   panel across the editor: root pointer routing plus pointer capture keeps
//!   the panel drag alive when the editor surface would otherwise swallow it.
//! - The default `panel-kit-dark` Monaco preset is generated from
//!   `panel_kit_core::tokens::DARK` and `tokens::MONO`. Switching to Monaco's
//!   high-contrast preset and back makes the reactive theme prop observable.
//! - Two-way `Signal<String>` binding, `on_change`, `on_ready`, and the
//!   imperative `EditorHandle` value/layout controls.
//! - Reactive language and read-only props, the `pest` Monarch tokenizer,
//!   and the minimal `toml` language on real samples.
//! - The workspace root translates keyboard and pointer events through the
//!   core reducer; editor text focus remains protected by the key policy.

use dioxus::events::PointerEvent as DioxusPointerEvent;
use dioxus::prelude::*;
use panel_kit::editor::{EditorHandle, MonacoEditor, PANEL_KIT_DARK_THEME};
use panel_kit::{LayoutBuilder, PanelKind, PanelWin, CSS};
use panel_kit_core::frame::Placement;
use panel_kit_core::persist::SavePolicy;
use serde::{Deserialize, Serialize};

#[path = "support/composable_workspace.rs"]
mod composable_workspace;

const DEMO_CSS: &str = "
.topbar .editor-state { color: var(--dim); }
.controls { display: flex; flex-wrap: wrap; gap: .4rem; align-items: center; }
.controls button { background: var(--bg); color: var(--dim); border: 1px solid var(--line2);
  border-radius: 3px; padding: .25rem .6rem; font-family: var(--mono);
  font-size: .72rem; cursor: pointer; }
.controls button.on { color: var(--fg); border-color: var(--accent); }
.controls .sep { color: var(--dim); }
.editor-note { color: var(--dim); font-size: .78rem; }
.preview { border: 1px solid var(--line2); border-radius: 4px; background: var(--panel);
  padding: .5rem .6rem; min-height: 6rem; max-height: 12rem; overflow: auto;
  font-size: .72rem; white-space: pre-wrap; margin: 0; }
.log { border: 1px solid var(--line2); border-radius: 4px; background: var(--panel);
  padding: .5rem .6rem; min-height: 4rem; max-height: 10rem; overflow: auto; }
.log div { font-size: .72rem; color: var(--fg); }
.log div:nth-child(n+2), .log .none { color: var(--dim); }
";
/// localStorage key retained for saved-layout compatibility.
///
/// Stable `Panel` serde IDs are unchanged: `Editor`, `Controls`, `Mirror`.
const STORAGE_KEY: &str = "panel_kit_example_editor";


const SAMPLE_PEST: &str = r##"// pest grammar for a line graph
line_graph = { SOI ~ line* ~ EOI }
line = _{ node ~ "->" ~ node ~ NEWLINE }
node = @{ ASCII_ALPHANUMERIC+ }

// stack syntax: PUSH/PEEK/POP
quoted = ${ PUSH("\"") ~ (!POP ~ ANY)* ~ POP }

/* ranges and repetition */
hex_pair = { '0'..'9' | 'a'..'f' }{2}
maybe  = ?{ "?" }   // not real pest — invalid modifier, shown dimmed
"##;

const SAMPLE_TOML: &str = r#"[metadata]
id = "pest-line-graph"
version = 3          # format_version
enabled = true

[limits]
max_bytes = 1_048_576
max_nodes = 0x4000

[[schema.fields]]
name = "source"
"#;

fn push_log(log: &mut Signal<Vec<String>>, entry: String) {
    let mut entries = log.write();
    entries.insert(0, entry);
    entries.truncate(12);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditorTheme {
    Canonical,
    HighContrast,
}

impl EditorTheme {
    fn label(self) -> &'static str {
        match self {
            Self::Canonical => "canonical tokens",
            Self::HighContrast => "Monaco high contrast",
        }
    }

    fn monaco_name(self) -> &'static str {
        match self {
            Self::Canonical => PANEL_KIT_DARK_THEME,
            Self::HighContrast => "hc-black",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Panel {
    Editor,
    Controls,
    Mirror,
}

impl PanelKind for Panel {
    fn title(self) -> &'static str {
        match self {
            Self::Editor => "Editor",
            Self::Controls => "Controls",
            Self::Mirror => "Mirror + Events",
        }
    }
}

fn default_layout() -> Vec<PanelWin<Panel>> {
    let mut builder = LayoutBuilder::new();
    vec![
        builder
            .at(Panel::Editor, 16.0, 16.0, 650.0, 520.0)
            .with_tile(3, 3),
        builder
            .at(Panel::Controls, 686.0, 16.0, 360.0, 280.0)
            .with_tile(1, 2),
        builder
            .at(Panel::Mirror, 686.0, 312.0, 360.0, 224.0)
            .with_tile(1, 2),
    ]
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let workspace =
        composable_workspace::use_demo_workspace(STORAGE_KEY, default_layout, SavePolicy::OnSettle);
    composable_workspace::mount_viewport_observer(&workspace);
    let emit = composable_workspace::workspace_event_handler(&workspace);
    let mut text = use_signal(|| SAMPLE_PEST.to_string());
    let mut handle = use_signal(|| None::<EditorHandle>);
    let mut log = use_signal(Vec::<String>::new);
    let mut language = use_signal(|| "pest".to_string());
    let mut read_only = use_signal(|| false);
    let mut theme = use_signal(|| EditorTheme::Canonical);
    let theme_name = theme().monaco_name().to_string();

    let body = move |kind: Panel, _maximized: bool| -> Element {
        match kind {
            Panel::Editor => rsx! {
                MonacoEditor {
                    value: text,
                    language: "{language}",
                    read_only: *read_only.read(),
                    theme: theme_name.clone(),
                    on_change: move |value: String| {
                        push_log(&mut log, format!("on_change: {} bytes", value.len()));
                    },
                    on_ready: move |editor: EditorHandle| {
                        push_log(&mut log, "on_ready: EditorHandle acquired".to_string());
                        handle.set(Some(editor));
                    },
                }
            },
            Panel::Controls => rsx! {
                p { class: "editor-note",
                    "Drag this panel by its inset title across Editor. Pointer capture "
                    "keeps the drag alive over Monaco."
                }
                p { class: "editor-note",
                    code { "{PANEL_KIT_DARK_THEME}" }
                    " is generated from panel_kit_core::tokens::DARK and tokens::MONO."
                }
                div { class: "controls",
                    button {
                        onclick: move |_| {
                            if let Some(editor) = *handle.read() {
                                editor.set_value(SAMPLE_PEST);
                            }
                        },
                        "handle.set_value(sample)"
                    }
                    button {
                        onclick: move |_| {
                            if let Some(editor) = *handle.read() {
                                let value = editor.value();
                                push_log(
                                    &mut log,
                                    format!("handle.value() -> {} bytes", value.len()),
                                );
                            }
                        },
                        "handle.value() → log"
                    }
                    button {
                        onclick: move |_| {
                            if let Some(editor) = *handle.read() {
                                editor.layout();
                            }
                        },
                        "handle.layout()"
                    }
                    span { class: "sep", "|" }
                    button {
                        onclick: move |_| {
                            text.write().push_str("// appended via the bound signal\n")
                        },
                        "signal: append comment"
                    }
                    button {
                        class: if *language.read() == "pest" { "on" } else { "" },
                        onclick: move |_| {
                            language.set("pest".to_string());
                            text.set(SAMPLE_PEST.to_string());
                        },
                        "language: pest"
                    }
                    button {
                        class: if *language.read() == "toml" { "on" } else { "" },
                        onclick: move |_| {
                            language.set("toml".to_string());
                            text.set(SAMPLE_TOML.to_string());
                        },
                        "language: toml"
                    }
                    button {
                        class: if *read_only.read() { "on" } else { "" },
                        onclick: move |_| {
                            let next = !*read_only.read();
                            read_only.set(next);
                        },
                        "read-only"
                    }
                    button {
                        class: if theme() == EditorTheme::Canonical { "on" } else { "" },
                        onclick: move |_| theme.set(EditorTheme::Canonical),
                        "theme: canonical"
                    }
                    button {
                        class: if theme() == EditorTheme::HighContrast { "on" } else { "" },
                        onclick: move |_| theme.set(EditorTheme::HighContrast),
                        "theme: high contrast"
                    }
                }
            },
            Panel::Mirror => rsx! {
                p { class: "editor-note", "Bound signal (two-way mirror)" }
                pre { class: "preview", "{text}" }
                p { class: "editor-note", "Event log" }
                div { class: "log",
                    if log.read().is_empty() {
                        div { class: "none", "no events yet — type in the editor" }
                    }
                    for entry in log.read().iter() {
                        div { "{entry}" }
                    }
                }
            },
        }
    };

    let snapshot = workspace.snapshot.read();
    let mut scratch = workspace.scratch.borrow_mut();
    let frame = composable_workspace::project_workspace(&snapshot, &mut scratch);
    let root_class = panel_kit::widgets::root::root_class(&frame);
    let workspace_class = composable_workspace::workspace_area_class(&frame);
    let workspace_style = frame
        .tile_grid
        .map(panel_kit::widgets::root::tile_grid_style)
        .unwrap_or_default();
    let pointer_move_workspace = workspace.clone();
    let pointer_up_workspace = workspace.clone();
    let pointer_cancel_workspace = workspace.clone();
    let key_workspace = workspace.clone();
    let wheel_workspace = workspace.clone();

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        div {
            class: "{root_class}",
            tabindex: "0",
            onpointermove: move |event: DioxusPointerEvent| {
                composable_workspace::handle_pointer_move(&pointer_move_workspace, &event)
            },
            onpointerup: move |event: DioxusPointerEvent| {
                composable_workspace::handle_pointer_up(&pointer_up_workspace, &event)
            },
            onpointercancel: move |event: DioxusPointerEvent| {
                composable_workspace::handle_pointer_up(&pointer_cancel_workspace, &event)
            },
            onkeydown: move |event: KeyboardEvent| composable_workspace::handle_key(&key_workspace, &event),
            header { class: "topbar",
                h1 { "panel-kit monaco editor demo" }
                span { class: "editor-state",
                    "theme: {theme().label()} · V2 layout · drag Controls across Editor"
                }
            }
            div {
                class: "{workspace_class}",
                style: "{workspace_style}",
                onwheel: move |event| composable_workspace::handle_wheel(&wheel_workspace, &event),
                for panel in frame.panels.iter().copied() {
                    if let Some(meta) = workspace.catalog.get(panel.key) {
                        {
                            let panel_class = format!("panel-{}", meta.slug);
                            let maximized = matches!(panel.placement, Placement::Maximized);
                            panel_kit::widgets::panel::panel_shell(panel, Some(&panel_class), rsx! {
                                {panel_kit::widgets::panel::panel_chrome_with_events(
                                    panel,
                                    meta,
                                    emit.clone(),
                                    Some(panel_kit::widgets::panel::traffic_lights(panel, emit.clone())),
                                    None,
                                )}
                                {panel_kit::widgets::panel::panel_body(body(panel.key, maximized))}
                                {panel_kit::widgets::panel::resize_grip(panel, emit.clone())}
                            })
                        }
                    }
                }
            }
            {panel_kit::widgets::dock::dock(frame.dock, &workspace.catalog, emit.clone())}
        }
    }
}
