use dioxus::prelude::*;

const DEMO_CSS: &str = "
.boot-demo-ready { position: fixed; z-index: 2147483001; right: 14px; bottom: 14px;
  border: 1px solid var(--line2); border-radius: 3px; padding: .35rem .65rem;
  background: var(--bg); color: var(--fg); cursor: pointer; }
.boot-demo-app { display: grid; place-items: center; height: 100vh; }
";

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut ready = use_signal(|| false);
    rsx! {
        style { {panel_kit::CSS} }
        style { {DEMO_CSS} }
        if !*ready.read() {
            panel_kit::LoadingWorkspace {
                title: "PANEL KIT",
                status: "loading workspace…",
            }
            button {
                class: "boot-demo-ready",
                r#type: "button",
                onclick: move |_| ready.set(true),
                "finish loading"
            }
        } else {
            div { class: "boot-demo-app", "Application content replaced the loading workspace." }
        }
    }
}
