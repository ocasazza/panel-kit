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

/// Browser overflow semantics per core policy. `Wrap` and `Auto` both wrap
/// long lines (the terminal wraps while preserving indentation) and scroll
/// vertically; only `Clip` truncates.
fn policy_style(policy: ScrollPolicy) -> &'static str {
    match policy {
        ScrollPolicy::Clip => "overflow:hidden;white-space:pre;",
        ScrollPolicy::Wrap | ScrollPolicy::Auto => {
            "overflow-y:auto;overflow-x:hidden;white-space:pre-wrap;overflow-wrap:anywhere;"
        }
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
