//! Grouped searchable dropdown with host-owned popup and selection state.
//!
//! Run with: `dx serve --example dropdown --platform web`

use dioxus::prelude::*;
use panel_kit::widgets::{Dropdown, DropdownAction, DropdownItem, DropdownState};
use panel_kit::CSS;

const DEMO_CSS: &str = "
body { overflow:auto !important; }
.demo { max-width:34rem; margin:0 auto; padding:2rem; }
.demo h1 { margin:0 0 1rem; font-size:.8rem; }
.demo p { color:var(--dim); }
.field { width:min(100%, 28rem); }
";

fn main() {
    dioxus::launch(App);
}

fn items() -> Vec<DropdownItem> {
    [
        (
            "spot-v74-000",
            "Real trot — long production policy label",
            "Spot / v74",
        ),
        ("spot-v74-001", "Velocity tracker", "Spot / v74"),
        ("spot-v72-011", "Baseline gait", "Spot / v72"),
        ("snake-sandbox", "Snake sandbox", "Sandboxes"),
    ]
    .into_iter()
    .map(|(value, label, group)| DropdownItem {
        value: value.into(),
        label: label.into(),
        group: group.into(),
    })
    .collect()
}

#[component]
fn App() -> Element {
    let state = use_signal(DropdownState::default);
    let mut selected = use_signal(String::new);

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        main { class: "demo",
            h1 { "host-owned dropdown" }
            div { class: "field",
                Dropdown {
                    items: items(),
                    state,
                    selected: selected(),
                    placeholder: "select a policy".to_string(),
                    searchable: true,
                    on_action: move |action: DropdownAction| {
                        if let DropdownAction::Select { value } = action {
                            selected.set(value);
                        }
                    },
                }
            }
            p {
                "selected: "
                if selected().is_empty() { "none" } else { "{selected}" }
            }
        }
    }
}
