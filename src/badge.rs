//! Dioxus painter for the shared badge model.
//!
//! The semantic badge contract lives in `panel-kit-core::badge::BadgeSpec` so
//! browser and terminal callers route the same actions. This module only maps
//! that spec to web DOM, ARIA, CSS classes, and the web-only CSS accent escape
//! hatch.

use dioxus::prelude::*;

use panel_kit_core::badge::display_label;
use panel_kit_core::widgets::badge::{perceived_brightness, tint_over};
pub use panel_kit_core::badge::{tag_hue, BadgeAction, BadgeClickKind, BadgeKind, BadgeSpec, Rgb};

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

fn rgb_css((r, g, b): Rgb) -> String {
    format!("rgb({r},{g},{b})")
}

fn class_for(spec: &BadgeSpec) -> String {
    let mut class = format!("badge {}", kind_class(&spec.kind));
    if spec.active {
        class.push_str(" active");
    }
    if spec.small {
        class.push_str(" small");
    }
    class
}

fn style_for(spec: &BadgeSpec, accent_color: Option<&str>) -> String {
    let mut style = String::new();
    if let Some(c) = spec.override_color {
        let (br, bg, bb) = tint_over(c, (255, 255, 255), 0.30);
        let fg = if perceived_brightness(c) < 0.55 { "var(--fg)" } else { "var(--bg)" };
        style.push_str(&format!(
            "--badge-bg:{};--badge-c:rgb({br},{bg},{bb});--badge-fg:{fg};",
            rgb_css(c)
        ));
    }
    if let Some(c) = spec.accent_color {
        let c = rgb_css(c);
        style.push_str(&format!("--badge-c:{c};--badge-fg:{c};"));
    }
    if let Some(c) = accent_color {
        style.push_str(&format!("--badge-c:{c};--badge-fg:{c};"));
    }
    style
}

/// Action delivered by a primary body click.
pub(crate) fn body_action(spec: &BadgeSpec) -> BadgeAction {
    spec.primary_action()
}

/// Action delivered by the optional `+` affordance.
pub(crate) fn plus_action(spec: &BadgeSpec) -> BadgeAction {
    spec.plus_action()
}

/// Action delivered by the optional `×` affordance.
pub(crate) fn x_action(spec: &BadgeSpec) -> BadgeAction {
    spec.x_action()
}

/// Action delivered by pointer hover when hover emission is enabled.
pub(crate) fn hover_action(spec: &BadgeSpec) -> Option<BadgeAction> {
    spec.hover_action()
}

/// Paint the badge body button with an already-selected primary action.
fn badge_body_button(
    aria_label: String,
    label: String,
    action: BadgeAction,
    on_action: EventHandler<BadgeAction>,
) -> Element {
    rsx! {
        button {
            class: "badge-main",
            r#type: "button",
            aria_label,
            onclick: move |_| on_action.call(action.clone()),
            span { class: "badge-label", "{label}" }
        }
    }
}

/// Paint a secondary badge affordance with an already-selected action.
fn badge_affordance(
    class: &'static str,
    aria_label: String,
    text: &'static str,
    action: BadgeAction,
    on_action: EventHandler<BadgeAction>,
) -> Element {
    rsx! {
        button {
            class,
            r#type: "button",
            aria_label,
            onclick: move |_| on_action.call(action.clone()),
            "{text}"
        }
    }
}

/// Omit an absent optional badge affordance without cloning the whole spec.
fn optional_badge_affordance(
    action: Option<(BadgeAction, String)>,
    class: &'static str,
    text: &'static str,
    on_action: EventHandler<BadgeAction>,
) -> Element {
    match action {
        Some((action, label)) => badge_affordance(class, label, text, action, on_action),
        None => rsx! {},
    }
}

/// Paint a badge from borrowed core fields while capturing only event actions.
pub(crate) fn paint_badge(
    spec: &BadgeSpec,
    accent_color: Option<&str>,
    on_action: EventHandler<BadgeAction>,
) -> Element {
    let label = display_label(&spec.kind, &spec.value);
    let class = class_for(spec);
    let style = style_for(spec, accent_color);
    let access = format!("badge:{}={}", spec.field, spec.value);
    let body = body_action(spec);
    let plus = spec.with_plus.then(|| {
        (
            plus_action(spec),
            format!("Add filter: {}={}", spec.field, spec.value),
        )
    });
    let x = spec.with_x.then(|| {
        (
            x_action(spec),
            format!("Toggle filter: {}={}", spec.field, spec.value),
        )
    });
    let hover = hover_action(spec);

    rsx! {
        span {
            class: "{class}",
            style: "{style}",
            role: "group",
            title: "{spec.value}",
            onpointerenter: move |_| {
                if let Some(action) = hover.as_ref() {
                    on_action.call(action.clone());
                }
            },
            {badge_body_button(access, label, body, on_action)}
            {optional_badge_affordance(plus, "badge-btn badge-plus", "+", on_action)}
            {optional_badge_affordance(x, "badge-btn badge-x", "\u{00D7}", on_action)}
        }
    }
}

/// Pill-shaped web badge painted from one shared [`BadgeSpec`].
///
/// `accent_color` is intentionally web-only: it accepts arbitrary CSS color
/// strings such as `var(--accent)` that other renderers cannot interpret.
/// Portable RGB accents belong in [`BadgeSpec::accent_color`].
#[component]
pub fn Badge(
    /// Shared renderer-neutral badge semantics.
    spec: BadgeSpec,
    /// Optional web-only CSS color overriding the portable RGB accent.
    #[props(default)]
    accent_color: Option<String>,
    /// Receives the badge action selected by [`BadgeSpec`] semantics.
    on_action: EventHandler<BadgeAction>,
) -> Element {
    paint_badge(&spec, accent_color.as_deref(), on_action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[component]
    fn BadgeProbe() -> Element {
        let spec = BadgeSpec { active: true, with_plus: true, with_x: true, accent_color: Some((94, 243, 140)), ..BadgeSpec::new("tag", "alpha", BadgeKind::Tag) };

        rsx! {
            Badge { spec, accent_color: Some("var(--accent)".to_string()), on_action: move |_| {} }
        }
    }

    #[test]
    fn web_badge_renders_spec_label_aria_and_accent_escape_hatch() {
        let html = dioxus_ssr::render_element(rsx! { BadgeProbe {} });

        assert!(html.contains("role=\"group\""), "{html}");
        assert!(html.contains("badge:tag=alpha"), "{html}");
        assert!(html.contains("class=\"badge badge-tag active\""), "{html}");
        assert!(html.contains("--badge-c:rgb(94,243,140);"), "{html}");
        assert!(html.contains("--badge-fg:var(--accent);"), "{html}");
        assert!(html.contains("Add filter: tag=alpha"), "{html}");
        assert!(html.contains("Toggle filter: tag=alpha"), "{html}");
    }

    #[test]
    fn badge_actions_match_across_backends() {
        let tag = BadgeSpec { with_plus: true, with_x: true, emit_hover: true, ..BadgeSpec::new("tag", "alpha", BadgeKind::Tag) };
        let clicked = BadgeSpec { click_kind: BadgeClickKind::Clicked, ..BadgeSpec::new("tag", "alpha", BadgeKind::Tag) };
        let link = BadgeSpec::new("link", "Panel Kit", BadgeKind::Wikilink { resolved: true, target: "Panel Kit".into() });
        let url = BadgeSpec::new("url", "panel-kit", BadgeKind::Url { href: "https://example.com/panel-kit".into(), host: "example.com".into() });

        assert_eq!(body_action(&tag), tag.primary_action());
        assert_eq!(body_action(&clicked), clicked.primary_action());
        assert_eq!(body_action(&link), link.primary_action());
        assert_eq!(body_action(&url), url.primary_action());
        assert_eq!(plus_action(&tag), tag.plus_action());
        assert_eq!(x_action(&tag), tag.x_action());
        assert_eq!(hover_action(&tag), tag.hover_action());
    }
}
