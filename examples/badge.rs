//! Badge demo — exercises every public parameter of `panel_kit::badge`.
//!
//! Run with: `dx serve --example badge --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`)
//!
//! What it demonstrates:
//! - All ten `BadgeKind` variants: Tag, Doctype, Folder, Author,
//!   `Entity { ty }` (both `Some` and `None`), `Wikilink { resolved, target }`
//!   (both resolved and unresolved), `Url { href, host }` (host shown as the
//!   label, plus the empty-host fallback), Date, Status, Generic.
//! - Live toggles for `active`, `with_x`, `with_plus`, `small`, `emit_hover`,
//!   both `BadgeClickKind`s (`Toggle` / `Clicked`), `override_color`
//!   (community-tint `Rgb`, with one dark and one bright swatch to show the
//!   derived contrast foreground), and `accent_color` (CSS colour string,
//!   `var(--…)` included).
//! - An on-screen event log proving every `BadgeAction` variant fires:
//!   `Toggle` (body click / `×`), `Clicked` (body click under
//!   `BadgeClickKind::Clicked`), `AddFilter` (`+`), `Navigate` (wikilink
//!   body), `OpenUrl` (url body), `Hovered` (pointer enter with `emit_hover`).
//! - A `tag_hue` row: deterministic FNV-1a hue spread over tag values.
//! - Long-value truncation: ellipsis after 28ch, full value in the tooltip.

use dioxus::prelude::*;
use panel_kit::badge::{tag_hue, Badge, BadgeAction, BadgeClickKind, BadgeKind, Rgb};
use panel_kit::CSS;

const DEMO_CSS: &str = "
body { overflow: auto !important; }
.demo { padding: 1rem; max-width: 980px; margin: 0 auto; }
.demo h1 { font-size: 1rem; }
.demo h2 { font-size: .8rem; color: var(--dim); text-transform: uppercase;
  letter-spacing: .06em; margin: 1.2rem 0 .4rem; }
.controls { display: flex; flex-wrap: wrap; gap: .4rem; align-items: center; }
.controls button { background: var(--bg); color: var(--dim); border: 1px solid var(--line2);
  border-radius: 3px; padding: .25rem .6rem; font-size: .72rem; cursor: pointer; }
.controls button.on { color: var(--fg); border-color: var(--accent); }
.controls .sep { color: var(--line2); }
.badge-row { display: flex; flex-wrap: wrap; gap: .5rem; align-items: center; margin: .5rem 0; }
.badge-row .note { color: var(--dim); font-size: .7rem; }
.kind-grid { display: grid; grid-template-columns: 11rem 1fr; gap: .45rem .8rem;
  align-items: center; margin: .5rem 0; }
.kind-grid .k { color: var(--dim); font-size: .72rem; }
.log { border: 1px solid var(--line); border-radius: 4px; background: var(--panel);
  padding: .5rem .6rem; min-height: 8rem; max-height: 14rem; overflow: auto; }
.log div { font-size: .72rem; color: var(--fg); }
.log div:nth-child(n+2) { color: var(--dim); }
.log .none { color: var(--line2); }
";

/// One bundle of `Copy` signals shared with every demo badge via context.
#[derive(Clone, Copy)]
struct Controls {
    active: Signal<bool>,
    with_x: Signal<bool>,
    with_plus: Signal<bool>,
    small: Signal<bool>,
    emit_hover: Signal<bool>,
    click_kind: Signal<BadgeClickKind>,
    override_color: Signal<Option<Rgb>>,
    accent_color: Signal<Option<String>>,
    log: Signal<Vec<String>>,
}

fn push_log(mut log: Signal<Vec<String>>, action: &BadgeAction) {
    let mut entries = log.write();
    entries.insert(0, format!("{action:?}"));
    entries.truncate(50);
}

/// A community-tint swatch dark enough that the derived fg goes light…
const TINT_DARK: Rgb = (30, 64, 175);
/// …and one bright enough that the derived fg goes dark.
const TINT_BRIGHT: Rgb = (250, 204, 21);

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let controls = Controls {
        active: use_signal(|| false),
        with_x: use_signal(|| false),
        with_plus: use_signal(|| false),
        small: use_signal(|| false),
        emit_hover: use_signal(|| false),
        click_kind: use_signal(BadgeClickKind::default),
        override_color: use_signal(|| Option::<Rgb>::None),
        accent_color: use_signal(|| Option::<String>::None),
        log: use_signal(Vec::new),
    };
    use_context_provider(|| controls);

    let log = controls.log;
    let tags = [
        "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta",
    ];

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        div { class: "demo",
            h1 { "panel-kit badge demo" }

            h2 { "props (apply to every badge below)" }
            div { class: "controls",
                FlagButton { label: "active", flag: controls.active }
                FlagButton { label: "with_x", flag: controls.with_x }
                FlagButton { label: "with_plus", flag: controls.with_plus }
                FlagButton { label: "small", flag: controls.small }
                FlagButton { label: "emit_hover", flag: controls.emit_hover }
                span { class: "sep", "│ click_kind:" }
                ClickKindButton { label: "Toggle", value: BadgeClickKind::Toggle }
                ClickKindButton { label: "Clicked", value: BadgeClickKind::Clicked }
                span { class: "sep", "│ override_color:" }
                TintButton { label: "none", value: None }
                TintButton { label: "dark blue", value: Some(TINT_DARK) }
                TintButton { label: "bright yellow", value: Some(TINT_BRIGHT) }
                span { class: "sep", "│ accent_color:" }
                AccentButton { label: "none", value: None }
                AccentButton { label: "var(--yellow)", value: Some("var(--yellow)".to_string()) }
                AccentButton { label: "#ff7edb", value: Some("#ff7edb".to_string()) }
            }

            h2 { "every BadgeKind" }
            div { class: "kind-grid",
                span { class: "k", "Tag" }
                div { class: "badge-row", DemoBadge { field: "tag", value: "rust", kind: BadgeKind::Tag } }
                span { class: "k", "Doctype" }
                div { class: "badge-row", DemoBadge { field: "doctype", value: "note", kind: BadgeKind::Doctype } }
                span { class: "k", "Folder" }
                div { class: "badge-row", DemoBadge { field: "folder", value: "projects/panel-kit", kind: BadgeKind::Folder } }
                span { class: "k", "Author" }
                div { class: "badge-row", DemoBadge { field: "author", value: "olive", kind: BadgeKind::Author } }
                span { class: "k", "Entity (ty: Some / None)" }
                div { class: "badge-row",
                    DemoBadge { field: "entity", value: "Ada Lovelace", kind: BadgeKind::Entity { ty: Some("person".to_string()) } }
                    DemoBadge { field: "entity", value: "untyped thing", kind: BadgeKind::Entity { ty: None } }
                }
                span { class: "k", "Wikilink (resolved / not)" }
                div { class: "badge-row",
                    DemoBadge { field: "link", value: "Home", kind: BadgeKind::Wikilink { resolved: true, target: "Home".to_string() } }
                    DemoBadge { field: "link", value: "Missing Page", kind: BadgeKind::Wikilink { resolved: false, target: "Missing Page".to_string() } }
                    span { class: "note", "body click → Navigate" }
                }
                span { class: "k", "Url (host label / empty-host fallback)" }
                div { class: "badge-row",
                    DemoBadge { field: "url", value: "https://example.com/a/long/path", kind: BadgeKind::Url { href: "https://example.com/a/long/path".to_string(), host: "example.com".to_string() } }
                    DemoBadge { field: "url", value: "https://example.com", kind: BadgeKind::Url { href: "https://example.com".to_string(), host: String::new() } }
                    span { class: "note", "body click → OpenUrl" }
                }
                span { class: "k", "Date" }
                div { class: "badge-row", DemoBadge { field: "date", value: "2026-06-09", kind: BadgeKind::Date } }
                span { class: "k", "Status" }
                div { class: "badge-row", DemoBadge { field: "status", value: "done", kind: BadgeKind::Status } }
                span { class: "k", "Generic" }
                div { class: "badge-row",
                    DemoBadge { field: "priority", value: "high", kind: BadgeKind::Generic }
                    DemoBadge {
                        field: "long",
                        value: "a very long value that truncates with an ellipsis — hover for the full text in the title tooltip",
                        kind: BadgeKind::Generic,
                    }
                }
            }

            h2 { "tag_hue — deterministic FNV-1a hue spread" }
            div { class: "badge-row",
                for t in tags {
                    DemoBadge {
                        field: "tag",
                        value: t,
                        kind: BadgeKind::Tag,
                        hue_accent: true,
                    }
                }
            }
            p { class: "note", style: "color: var(--dim); font-size: .72rem;",
                "each chip's colour is hsl(tag_hue(value)·360, 70%, 60%) — stable across reloads and hosts"
            }

            h2 { "BadgeAction log (newest first)" }
            div { class: "log",
                if log.read().is_empty() {
                    div { class: "none", "— click, hover (with emit_hover), + or × a badge —" }
                }
                for entry in log.read().iter() {
                    div { "{entry}" }
                }
            }
        }
    }
}

/// On/off toggle for one boolean prop.
#[component]
fn FlagButton(label: String, flag: Signal<bool>) -> Element {
    let on = flag();
    let cls = if on { "on" } else { "" };
    let mut flag = flag;
    rsx! {
        button { class: "{cls}", onclick: move |_| flag.set(!on), "{label}" }
    }
}

#[component]
fn ClickKindButton(label: String, value: BadgeClickKind) -> Element {
    let c = use_context::<Controls>();
    let cls = if *c.click_kind.read() == value {
        "on"
    } else {
        ""
    };
    let mut sig = c.click_kind;
    rsx! {
        button { class: "{cls}", onclick: move |_| sig.set(value), "{label}" }
    }
}

#[component]
fn TintButton(label: String, value: Option<Rgb>) -> Element {
    let c = use_context::<Controls>();
    let cls = if *c.override_color.read() == value {
        "on"
    } else {
        ""
    };
    let mut sig = c.override_color;
    rsx! {
        button { class: "{cls}", onclick: move |_| sig.set(value), "{label}" }
    }
}

#[component]
fn AccentButton(label: String, value: Option<String>) -> Element {
    let c = use_context::<Controls>();
    let cls = if *c.accent_color.read() == value {
        "on"
    } else {
        ""
    };
    let mut sig = c.accent_color;
    rsx! {
        button { class: "{cls}", onclick: move |_| sig.set(value.clone()), "{label}" }
    }
}

/// A badge wired to the shared controls and the action log. When
/// `hue_accent` is set, `accent_color` comes from `tag_hue(value)` instead
/// of the global control (the FNV hue-spread row).
#[component]
fn DemoBadge(
    field: String,
    value: String,
    kind: BadgeKind,
    #[props(default)] hue_accent: bool,
) -> Element {
    let c = use_context::<Controls>();
    let accent = if hue_accent {
        Some(format!("hsl({:.0} 70% 60%)", tag_hue(&value) * 360.0))
    } else {
        c.accent_color.read().clone()
    };
    rsx! {
        Badge {
            field,
            value,
            kind,
            active: *c.active.read(),
            with_x: *c.with_x.read(),
            with_plus: *c.with_plus.read(),
            small: *c.small.read(),
            override_color: *c.override_color.read(),
            accent_color: accent,
            click_kind: *c.click_kind.read(),
            emit_hover: *c.emit_hover.read(),
            on_action: move |a: BadgeAction| push_log(c.log, &a),
        }
    }
}
