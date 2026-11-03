//! Web hint registry and hint bar over the core hints model.
//!
//! Ported from jump-cannon's `hints.rs`: a global registry any component can
//! publish a contextual hint into (what is under the pointer / focused, and
//! which hotkeys act on it). The topbar renders the most recently published
//! entry; each publisher owns one slot keyed by its `source`.

use dioxus::prelude::*;
use panel_kit_core::widgets::hints::{Hint, HintRegistry};

/// Publish order: the last entry is the one shown.
static HINTS: GlobalSignal<HintRegistry> = Signal::global(HintRegistry::default);

/// Publish `hint`, replacing any earlier entry from the same source and
/// making it the shown one. No-op write when the shown entry is unchanged.
pub fn publish(hint: Hint) {
    if HINTS.peek().shown() == Some(&hint) {
        return;
    }
    HINTS.write().publish(hint);
}

/// Drop `source`'s entry; the previously published hint (if any) shows.
pub fn clear(source: &'static str) {
    if HINTS.read().contains(source) {
        HINTS.write().clear(source);
    }
}

/// The currently shown hint, if any.
pub fn shown() -> Option<Hint> {
    HINTS.peek().shown().cloned()
}

/// Topbar mount: the shown hint as title · body plus hotkey chips. Renders
/// the (empty) region unconditionally so the header never reflows.
#[component]
pub fn HintBar() -> Element {
    let registry = HINTS.read();
    let shown = registry.shown();
    rsx! {
        div { class: "pk-hintbar", role: "status", aria_live: "polite",
            if let Some(h) = shown {
                span { class: "pk-hintbar-title", "{h.title}" }
                if let Some(b) = &h.body {
                    span { class: "pk-hintbar-body", "{b}" }
                }
                for (label, keys) in h.hotkeys.iter() {
                    span { class: "pk-hintbar-key",
                        "{label}"
                        kbd { "{keys}" }
                    }
                }
            }
        }
    }
}
