//! Web text/scroll painter over core scroll policy.

use dioxus::prelude::*;
use panel_kit_core::widgets::ScrollPolicy;

fn policy_name(policy: ScrollPolicy) -> &'static str {
    match policy {
        ScrollPolicy::Clip => "clip",
        ScrollPolicy::Wrap => "wrap",
        ScrollPolicy::Auto => "auto",
    }
}

fn policy_style(policy: ScrollPolicy) -> &'static str {
    match policy {
        ScrollPolicy::Clip => "overflow:hidden;white-space:pre;",
        ScrollPolicy::Wrap => "overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere;",
        ScrollPolicy::Auto => "overflow:auto;white-space:pre;",
    }
}

/// Paint borrowed text with native browser overflow semantics requested by core.
pub fn text(text: &str, policy: ScrollPolicy) -> Element {
    let policy_name = policy_name(policy);
    let style = policy_style(policy);
    rsx! {
        div { class: "pk-widget pk-scroll pk-scroll-{policy_name}", role: "region", aria_label: "scrollable text", "data-scroll-policy": "{policy_name}", style: "{style}",
            pre { class: "pk-scroll-text", "{text}" }
        }
    }
}
