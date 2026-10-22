//! Spinner demo — exercises both states of `panel_kit::Spinner`.
//!
//! Run with: `dx serve --example spinner --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`)
//!
//! What it demonstrates:
//! - `Spinner {}` with no `label` (the default, ring only — the label span
//!   is not rendered at all).
//! - `Spinner { label }` with a label.
//! - A live text input driving the label: clear it and the spinner drops
//!   back to the ring-only form.

use dioxus::prelude::*;
use panel_kit::{Spinner, CSS};

const DEMO_CSS: &str = "
body { overflow: auto !important; }
.demo { padding: 1rem; max-width: 720px; margin: 0 auto; }
.demo h1 { font-size: 1rem; }
.row { display: flex; align-items: center; gap: 1rem; margin: .8rem 0;
  border: 1px solid var(--color-divider); border-radius: 4px; background: var(--color-surface);
  padding: .6rem .8rem; }
.row .k { color: var(--color-text-muted); font-size: .72rem; width: 16rem; flex: none; }
.demo input { background: var(--color-background); color: var(--color-text); border: 1px solid var(--color-border);
  border-radius: 3px; font-family: var(--mono); font-size: .78rem; padding: .3rem .5rem; }
";

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut label = use_signal(|| "loading the vault…".to_string());

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        div { class: "demo",
            h1 { "panel-kit spinner demo" }
            div { class: "row",
                span { class: "k", "no label (default)" }
                Spinner {}
            }
            div { class: "row",
                span { class: "k", "with label" }
                Spinner { label: "indexing 1,204 notes" }
            }
            div { class: "row",
                span { class: "k", "live label (clear it → ring only)" }
                Spinner { label: "{label}" }
            }
            div { class: "row",
                span { class: "k", "label text" }
                input {
                    value: "{label}",
                    oninput: move |e| label.set(e.value()),
                }
            }
        }
    }
}
