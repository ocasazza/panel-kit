//! Selectable-card painter for one-of-N pickers.
//!
//! Ported from jump-cannon's `selection_card.rs` (the shared card/chip
//! primitives its engine and importer pickers shared). The data lives in
//! [`panel_kit_core::widgets::selection::SelectionCard`]; this module only
//! paints it and reports selection. The `data-*` attributes key regression
//! selectors (`[data-source-id]`, `[data-default]`, `[data-viewing]`, …), and
//! the state classes (`active` / `default` / `ghost` / `viewing`) stay
//! host-composed through `card.class`, so hosts keep their existing selector
//! chains.

use dioxus::prelude::*;
use panel_kit_core::widgets::selection::SelectionCard;

/// One-pill chip used inside [`SelectableCard`]'s chip strip. Returns the raw
/// span so the host can compose multiple into a single chip strip without
/// spawning one rsx! child per chip. `class_suffix` is appended to
/// `pk-select-card-chip` — the host can drive per-kind colour rules in CSS
/// without touching Rust.
pub fn card_chip(class_suffix: &str, label: &str) -> Element {
    let suffix = if class_suffix.is_empty() {
        String::new()
    } else {
        format!(" {class_suffix}")
    };
    rsx! {
        span { class: "pk-select-card-chip{suffix}", "{label}" }
    }
}

/// Paint one selectable card: a `<button>` whose body click selects it.
#[component]
pub fn SelectableCard(
    /// The card data: identity, head, chips, description, disabled state.
    card: SelectionCard,
    /// Fired on body click while the card is enabled.
    #[props(default)]
    on_select: Option<EventHandler<()>>,
    /// Extra body content (an action row, identity facts, a disclosure),
    /// rendered after the description and before the disabled-reason text so
    /// the natural reading order is preserved.
    children: Element,
) -> Element {
    let SelectionCard {
        source_id,
        kind,
        class,
        name,
        subtitle,
        chips,
        description,
        disabled_reason,
        is_default,
        is_viewing,
        is_ghost,
        disabled,
        title,
    } = card;
    rsx! {
        button {
            class: "{class}",
            r#type: "button",
            disabled,
            "data-source-id": "{source_id}",
            "data-kind": "{kind}",
            "data-default": if is_default { "true" } else { "false" },
            "data-viewing": if is_viewing { "true" } else { "false" },
            "data-ghost": if is_ghost { "true" } else { "false" },
            title: "{title}",
            "aria-pressed": if is_viewing { "true" } else { "false" },
            onclick: move |_| {
                if let Some(h) = on_select.as_ref() {
                    h.call(());
                }
            },
            div { class: "pk-select-card-head",
                div { class: "pk-select-card-title",
                    h3 { "{name}" }
                    if !subtitle.is_empty() {
                        code { class: "pk-select-card-subtitle", "{subtitle}" }
                    }
                }
                if !chips.is_empty() {
                    div { class: "pk-select-card-chips",
                        for (class_suffix, label) in chips.iter() {
                            {card_chip(class_suffix, label)}
                        }
                    }
                }
            }
            if !description.is_empty() {
                p { class: "pk-select-card-desc", "{description}" }
            }
            {children}
            if !disabled_reason.is_empty() {
                span { class: "pk-select-card-why", "{disabled_reason}" }
            }
        }
    }
}
