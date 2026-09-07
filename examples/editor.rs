//! Monaco editor demo — exercises every interface of
//! `panel_kit::editor::MonacoEditor`.
//!
//! Run with: `dx serve --example editor --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`)
//!
//! Asset prerequisite: the editor loads its vendored bundle from
//! `assets/vendor/monaco-editor-0.56.0/` relative to the served root (see
//! `panel_kit::editor::MONACO_ASSET_DIR`). When running from this repo that
//! path is panel-kit's own `assets/` tree; a consuming app must copy that
//! directory into its own served assets (trunk: `<link data-trunk
//! rel="copy-dir" href="assets/vendor" />`) or call
//! `panel_kit::editor::set_monaco_asset_base` before first mount.
//!
//! What it demonstrates:
//! - Two-way binding: the editor and the read-only `<pre>` mirror of the
//!   same `Signal<String>` stay in sync both ways (type in the editor, or
//!   use the "append"/"load sample" buttons to write the signal and watch
//!   the editor update with the cursor preserved).
//! - The pull/event interface: `on_change` entries land in the event log
//!   (typed and `handle.set_value` edits fire it; external signal writes
//!   do not).
//! - The imperative `EditorHandle` (from `on_ready`): set value, read value,
//!   switch language, toggle read-only, force layout.
//! - Reactive props: the language and read-only buttons drive props, not
//!   the handle, and the open editor follows.
//! - The `pest` Monarch tokenizer (rule definitions, builtins, strings,
//!   operators, comments) and the minimal `toml` language on real samples.

use dioxus::prelude::*;
use panel_kit::editor::{EditorHandle, MonacoEditor};
use panel_kit::CSS;

const DEMO_CSS: &str = "
body { overflow: auto !important; }
.demo { padding: 1rem; max-width: 980px; margin: 0 auto; }
.demo h1 { font-size: 1rem; }
.demo h2 { font-size: .8rem; color: var(--dim); text-transform: uppercase;
  letter-spacing: .06em; margin: 1.2rem 0 .4rem; }
.controls { display: flex; flex-wrap: wrap; gap: .4rem; align-items: center; }
.controls button { background: var(--bg); color: var(--dim); border: 1px solid var(--line2);
  border-radius: 3px; padding: .25rem .6rem; font-size: .72rem; cursor: pointer; }
.controls button.on { color: var(--fg); border-color: var(--accent); }
.controls .sep { color: var(--line2); }
.editor-wrap { height: 420px; border: 1px solid var(--line2); border-radius: 4px;
  overflow: hidden; margin: .5rem 0; }
.preview { border: 1px solid var(--line); border-radius: 4px; background: var(--panel);
  padding: .5rem .6rem; min-height: 6rem; max-height: 12rem; overflow: auto;
  font-size: .72rem; white-space: pre-wrap; margin: 0; }
.log { border: 1px solid var(--line); border-radius: 4px; background: var(--panel);
  padding: .5rem .6rem; min-height: 4rem; max-height: 10rem; overflow: auto; }
.log div { font-size: .72rem; color: var(--fg); }
.log div:nth-child(n+2) { color: var(--dim); }
.log .none { color: var(--line2); }
";

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

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Two-way bound document: the editor and the <pre> mirror share it.
    let mut text = use_signal(|| SAMPLE_PEST.to_string());
    let mut handle = use_signal(|| None::<EditorHandle>);
    let mut log = use_signal(Vec::<String>::new);
    // Reactive props driven by the buttons below.
    let mut language = use_signal(|| "pest".to_string());
    let mut read_only = use_signal(|| false);

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        div { class: "demo",
            h1 { "panel-kit monaco editor demo" }

            div { class: "controls",
                // EditorHandle (imperative) controls:
                button {
                    onclick: move |_| {
                        if let Some(h) = *handle.read() {
                            h.set_value(SAMPLE_PEST);
                        }
                    },
                    "handle.set_value(sample)"
                }
                button {
                    onclick: move |_| {
                        if let Some(h) = *handle.read() {
                            let v = h.value();
                            push_log(&mut log, format!("handle.value() -> {} bytes", v.len()));
                        }
                    },
                    "handle.value() → log"
                }
                button {
                    onclick: move |_| {
                        if let Some(h) = *handle.read() {
                            h.layout();
                        }
                    },
                    "handle.layout()"
                }
                span { class: "sep", "|" }
                // Signal writes (external -> editor, cursor preserved):
                button {
                    onclick: move |_| text.write().push_str("// appended via the bound signal\n"),
                    "signal: append comment"
                }
                // Reactive props:
                button {
                    class: if *language.read() == "pest" { "on" } else { "" },
                    onclick: move |_| {
                        language.set("pest".to_string());
                        text.set(SAMPLE_PEST.to_string());
                    },
                    "prop: language=pest"
                }
                button {
                    class: if *language.read() == "toml" { "on" } else { "" },
                    onclick: move |_| {
                        language.set("toml".to_string());
                        text.set(SAMPLE_TOML.to_string());
                    },
                    "prop: language=toml"
                }
                button {
                    class: if *read_only.read() { "on" } else { "" },
                    onclick: move |_| {
                        let next = !*read_only.read();
                        read_only.set(next);
                    },
                    "prop: read_only"
                }
            }

            div { class: "editor-wrap",
                MonacoEditor {
                    value: text,
                    language: "{language}",
                    read_only: *read_only.read(),
                    on_change: move |v: String| {
                        push_log(&mut log, format!("on_change: {} bytes", v.len()));
                    },
                    on_ready: move |h: EditorHandle| {
                        push_log(&mut log, "on_ready: EditorHandle acquired".to_string());
                        handle.set(Some(h));
                    },
                }
            }

            h2 { "bound signal (two-way mirror)" }
            pre { class: "preview", "{text}" }

            h2 { "event log" }
            div { class: "log",
                if log.read().is_empty() {
                    div { class: "none", "no events yet — type in the editor" }
                }
                for entry in log.read().iter() {
                    div { "{entry}" }
                }
            }
        }
    }
}
