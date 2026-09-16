//! Host-owned cascade demo, including parent rerenders while the popup is open.
//!
//! Run with: `dx serve --example cascade --platform web`

use dioxus::prelude::*;
use panel_kit::widgets::{CascadeAction, CascadeItem, CascadeState, CascadingDropdown};
use panel_kit::CSS;

const DEMO_CSS: &str = "
body { overflow:auto !important; }
.demo { max-width:64rem; margin:0 auto; padding:2rem; }
.demo h1 { margin:0 0 1rem; font-size:.8rem; }
.demo p { color:var(--dim); }
.demo button { background:transparent; color:var(--fg); border:1px solid var(--line2);
  border-radius:3px; padding:.3rem .55rem; font:inherit; cursor:pointer; }
";

fn main() {
    dioxus::launch(App);
}

fn items() -> Vec<CascadeItem> {
    let mut items = Vec::new();
    for campaign in ["spot-walk-pbt-v74", "spot-walk-pbt-v72"] {
        for index in 0..3 {
            items.push(CascadeItem {
                path: vec!["spot".into(), campaign.into()],
                value: format!("{campaign}-{index:03}"),
                label: format!("{index:03} — policy with a deliberately long readable label"),
            });
        }
    }
    for robot in ["spider", "snake", "humanoid"] {
        items.push(CascadeItem {
            path: Vec::new(),
            value: robot.into(),
            label: format!("{robot} (sandbox)"),
        });
    }
    items
}

#[component]
fn App() -> Element {
    let mut tick = use_signal(|| 0_u32);
    use_hook(move || {
        spawn(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(250).await;
                tick += 1;
            }
        });
    });

    let mut state = use_signal(CascadeState::default);
    let mut selected_path = use_signal(Vec::<String>::new);
    let mut selected_label = use_signal(String::new);
    let selection = if selected_label().is_empty() {
        "none".to_string()
    } else {
        format!("{} / {}", selected_path().join(" / "), selected_label())
    };

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        main { class: "demo",
            h1 { "host-owned cascade" }
            p { "parent render tick: {tick}" }
            CascadingDropdown {
                items: items(),
                state,
                selected_path: selected_path(),
                selected_label: selected_label(),
                placeholder: "pick a robot or policy".to_string(),
                on_action: move |action: CascadeAction| {
                    if let CascadeAction::Select { path, value } = action {
                        selected_path.set(path);
                        selected_label.set(value);
                    }
                },
            }
            p { "selection: {selection}" }
            button {
                r#type: "button",
                onclick: move |_| state.set(CascadeState::open()),
                "open programmatically"
            }
        }
    }
}
