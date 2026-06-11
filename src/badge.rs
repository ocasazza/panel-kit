//! Reusable pill-shaped badge widget for clickable metadata — a Dioxus
//! port of jump-cannon's egui badge (`graph-renderer/src/ui/badge.rs`).
//!
//! Renders a (field, value) pair as a rounded chip whose click navigates,
//! opens a URL, or toggles a filter depending on [`BadgeKind`]. The egui
//! widget returns a `BadgeAction` from its immediate-mode `show()`; here
//! the action arrives through the `on_action` callback instead, so there
//! is no `None` variant — the callback only fires when something happens.
//! The host app maps fields/values onto kinds and routes the action.
//!
//! Entry points: the [`Badge`] component, [`BadgeKind`] /
//! [`BadgeClickKind`] to configure it, [`BadgeAction`] for what comes
//! back, and [`tag_hue`] for stable per-tag colours.

use dioxus::prelude::*;

// The badge model (kinds, actions, hue derivation) lives in
// panel-kit-core::badge so the terminal shell shares it; this module is
// the Dioxus rendering of that model.
pub use panel_kit_core::badge::{tag_hue, BadgeAction, BadgeClickKind, BadgeKind, Rgb};
use panel_kit_core::badge::display_label;

/// Rec. 709 luma in [0, 1]. Picks a foreground (light vs dark) that stays
/// readable across the full categorical palette.
fn perceived_brightness((r, g, b): Rgb) -> f32 {
    0.2126 * (r as f32 / 255.0) + 0.7152 * (g as f32 / 255.0) + 0.0722 * (b as f32 / 255.0)
}

/// Opaquely blend `over` (at strength `a`) on top of `base` — the border
/// of a community-tinted badge is its bg pushed 30% toward white.
fn tint_over(base: Rgb, over: Rgb, a: f32) -> Rgb {
    let a = a.clamp(0.0, 1.0);
    let mix = |b: u8, o: u8| -> u8 { (b as f32 * (1.0 - a) + o as f32 * a).round() as u8 };
    (
        mix(base.0, over.0),
        mix(base.1, over.1),
        mix(base.2, over.2),
    )
}

fn kind_class(kind: &BadgeKind) -> &'static str {
    match kind {
        BadgeKind::Tag => "badge-tag",
        BadgeKind::Doctype => "badge-doctype",
        BadgeKind::Folder => "badge-folder",
        BadgeKind::Author => "badge-author",
        BadgeKind::Entity { .. } => "badge-entity",
        BadgeKind::Wikilink { resolved: true, .. } => "badge-wikilink",
        BadgeKind::Wikilink { resolved: false, .. } => "badge-wikilink badge-unresolved",
        BadgeKind::Url { .. } => "badge-url",
        BadgeKind::Date => "badge-date",
        BadgeKind::Status => "badge-status",
        BadgeKind::Generic => "badge-generic",
    }
}

/// Pill-shaped chip rendering a `(field, value)` pair of clickable
/// metadata; user interaction arrives as a [`BadgeAction`] through
/// `on_action`.
///
/// Mirrors the egui builder surface: `active` (halo), `with_x` (trailing
/// `×` → [`BadgeAction::Toggle`]), `with_plus` (trailing `+` →
/// [`BadgeAction::AddFilter`], drawn left of `×` when both are on),
/// `small` (cramped chip-strip geometry), `override_color` (community
/// tint: bg = colour, border/fg derived for contrast), `click_kind`,
/// `emit_hover`.
///
/// `accent_color` is the DOM port of the egui modal's one-shot
/// `status_pill` / `ticket_badge` stroke colour: it recolours border +
/// text (any CSS colour, `var(--…)` included) while keeping the dark fill.
///
/// Long values truncate with an ellipsis (the chip carries the full value
/// in `title`); the egui widget sizes to content instead, but unbounded
/// chips don't survive a DOM flex row.
///
/// # Examples
///
/// ```no_run
/// use dioxus::prelude::*;
/// use panel_kit::badge::{Badge, BadgeAction, BadgeKind};
///
/// # fn chips() -> Element {
/// rsx! {
///     // A removable tag chip driving a filter set.
///     Badge {
///         field: "tag",
///         value: "project/alpha",
///         kind: BadgeKind::Tag,
///         active: true,
///         with_x: true,
///         on_action: move |a: BadgeAction| {
///             if let BadgeAction::Toggle { field, value } = a {
///                 // flip the (field, value) filter…
///             }
///         },
///     }
///     // A wikilink chip that navigates on click.
///     Badge {
///         field: "link",
///         value: "Reading List",
///         kind: BadgeKind::Wikilink { resolved: true, target: "Reading List".into() },
///         on_action: move |a: BadgeAction| {
///             if let BadgeAction::Navigate { target } = a {
///                 // open `target`…
///             }
///         },
///     }
/// }
/// # }
/// ```
#[component]
#[allow(clippy::too_many_arguments)]
pub fn Badge(
    /// Attribute name half of the pair (e.g. `"tag"`); carried back in
    /// field-bearing [`BadgeAction`]s and in the accessible
    /// `badge:<field>=<value>` label.
    field: String,
    /// Attribute value half of the pair — the chip's label (except for
    /// [`BadgeKind::Url`], which shows its host) and the chip's `title`
    /// tooltip.
    value: String,
    /// Visual + behavioural kind; see [`BadgeKind`]. Defaults to
    /// [`BadgeKind::Generic`].
    #[props(default = BadgeKind::Generic)]
    kind: BadgeKind,
    /// Draw the active halo — use for badges whose filter is currently
    /// applied.
    #[props(default)]
    active: bool,
    /// Append a trailing `×` button that emits [`BadgeAction::Toggle`]
    /// (which removes when the badge is active).
    #[props(default)]
    with_x: bool,
    /// Append a trailing `+` button that emits [`BadgeAction::AddFilter`];
    /// drawn left of `×` when both are on.
    #[props(default)]
    with_plus: bool,
    /// Cramped geometry for dense chip strips.
    #[props(default)]
    small: bool,
    /// Community tint: the [`Rgb`] becomes the background, with border and
    /// foreground derived for contrast.
    #[props(default)]
    override_color: Option<Rgb>,
    /// Recolour border + text with any CSS colour (`var(--…)` included)
    /// while keeping the dark fill.
    #[props(default)]
    accent_color: Option<String>,
    /// Body-click semantics for non-Wikilink/Url kinds; see
    /// [`BadgeClickKind`].
    #[props(default)]
    click_kind: BadgeClickKind,
    /// Also emit [`BadgeAction::Hovered`] when the pointer enters the chip.
    #[props(default)]
    emit_hover: bool,
    /// Receives every [`BadgeAction`] the chip produces.
    on_action: EventHandler<BadgeAction>,
) -> Element {
    let label = display_label(&kind, &value);
    // Accessible name matches the egui widget's `widget_info` label so
    // test harnesses can find badges by the same key on both stacks.
    let access = format!("badge:{field}={value}");

    let mut class = format!("badge {}", kind_class(&kind));
    if active {
        class.push_str(" active");
    }
    if small {
        class.push_str(" small");
    }

    let mut style = String::new();
    if let Some(c) = override_color {
        let (br, bg_, bb) = tint_over(c, (255, 255, 255), 0.30);
        let fg = if perceived_brightness(c) < 0.55 {
            "var(--fg)"
        } else {
            "var(--bg)"
        };
        style.push_str(&format!(
            "--badge-bg:rgb({},{},{});--badge-c:rgb({br},{bg_},{bb});--badge-fg:{fg};",
            c.0, c.1, c.2
        ));
    }
    if let Some(c) = &accent_color {
        style.push_str(&format!("--badge-c:{c};--badge-fg:{c};"));
    }

    let body_kind = kind.clone();
    let (body_field, body_value) = (field.clone(), value.clone());
    let (hover_field, hover_value) = (field.clone(), value.clone());
    let (plus_field, plus_value) = (field.clone(), value.clone());
    let (x_field, x_value) = (field.clone(), value.clone());

    rsx! {
        span {
            class: "{class}",
            style: "{style}",
            role: "button",
            tabindex: "0",
            aria_label: "{access}",
            title: "{value}",
            onclick: move |_| {
                let action = match &body_kind {
                    BadgeKind::Wikilink { target, .. } => {
                        BadgeAction::Navigate { target: target.clone() }
                    }
                    BadgeKind::Url { href, .. } => BadgeAction::OpenUrl { href: href.clone() },
                    _ => match click_kind {
                        BadgeClickKind::Toggle => BadgeAction::Toggle {
                            field: body_field.clone(),
                            value: body_value.clone(),
                        },
                        BadgeClickKind::Clicked => BadgeAction::Clicked {
                            field: body_field.clone(),
                            value: body_value.clone(),
                        },
                    },
                };
                on_action.call(action);
            },
            onmouseenter: move |_| {
                if emit_hover {
                    on_action.call(BadgeAction::Hovered {
                        field: hover_field.clone(),
                        value: hover_value.clone(),
                    });
                }
            },
            span { class: "badge-label", "{label}" }
            // `+` sits left of `×` when both are on (egui trailing-edge order).
            if with_plus {
                button {
                    class: "badge-btn badge-plus",
                    onclick: move |e| {
                        e.stop_propagation();
                        on_action.call(BadgeAction::AddFilter {
                            field: plus_field.clone(),
                            value: plus_value.clone(),
                        });
                    },
                    "+"
                }
            }
            if with_x {
                button {
                    class: "badge-btn badge-x",
                    onclick: move |e| {
                        e.stop_propagation();
                        // `×` is a toggle (removes when active) — egui parity.
                        on_action.call(BadgeAction::Toggle {
                            field: x_field.clone(),
                            value: x_value.clone(),
                        });
                    },
                    "\u{00D7}"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_hue_deterministic() {
        assert_eq!(tag_hue("alpha"), tag_hue("alpha"));
        assert!((0.0..1.0).contains(&tag_hue("alpha")));
        assert_ne!(tag_hue("alpha"), tag_hue("beta"));
    }

    #[test]
    fn wikilink_label_prefixed() {
        let k = BadgeKind::Wikilink { resolved: true, target: "Page".into() };
        assert_eq!(display_label(&k, "Page"), "\u{27F6} Page");
    }

    #[test]
    fn url_label_prefers_host() {
        let k = BadgeKind::Url { href: "https://example.com/x".into(), host: "example.com".into() };
        assert_eq!(display_label(&k, "https://example.com/x"), "example.com");
        let bare = BadgeKind::Url { href: "https://example.com".into(), host: String::new() };
        assert_eq!(display_label(&bare, "https://example.com"), "https://example.com");
    }
}
