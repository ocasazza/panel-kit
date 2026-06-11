//! Renderer-agnostic badge model: what kind of metadata a badge shows,
//! what a click on it means, and the stable per-tag hue derivation.
//!
//! Rendering lives in the shells: the Dioxus `panel-kit` crate draws
//! pill-shaped DOM chips, `panel-kit-tui` draws styled terminal spans —
//! both over these types, so a host app's action-routing `match` is
//! identical in the browser and the terminal.

/// RGB triple for the community-tint override. Kept numeric (not a CSS
/// string) so shells can derive contrast foregrounds / brightened borders
/// in Rust.
pub type Rgb = (u8, u8, u8);

/// What kind of metadata a badge shows — selects its colour and its
/// body-click behaviour.
///
/// Most kinds only differ visually; [`Wikilink`](BadgeKind::Wikilink) and
/// [`Url`](BadgeKind::Url) also change the click action to
/// [`BadgeAction::Navigate`] / [`BadgeAction::OpenUrl`] and decorate the
/// label (see [`display_label`]).
#[derive(Debug, Clone, PartialEq)]
pub enum BadgeKind {
    /// A `#tag`-style label. Pair with [`tag_hue`] for a stable per-value
    /// colour via the shells' `override_color`.
    Tag,
    /// A document type (e.g. "note", "meeting").
    Doctype,
    /// A folder / path segment.
    Folder,
    /// An author / person.
    Author,
    /// A named entity, optionally typed (e.g. `ty: Some("org")`); the type
    /// only affects how the host routes actions, not the rendering.
    Entity {
        /// Optional entity type, carried for the host's benefit.
        ty: Option<String>,
    },
    /// An internal `[[wikilink]]`. The label is drawn with a `⟶` prefix;
    /// body clicks emit [`BadgeAction::Navigate`] with `target`.
    Wikilink {
        /// Whether the link target exists.
        resolved: bool,
        /// Navigation target emitted on click.
        target: String,
    },
    /// An external URL. The label shows `host` (falling back to the badge
    /// value when empty); body clicks emit [`BadgeAction::OpenUrl`] with
    /// `href`.
    Url {
        /// Full URL emitted on click.
        href: String,
        /// Display host (e.g. `example.com`); empty → the value is shown.
        host: String,
    },
    /// A date value.
    Date,
    /// A status value (e.g. "draft", "done").
    Status,
    /// Anything else — the default kind.
    Generic,
}

/// What the user did to a badge. The host app matches on this and routes:
/// toggle a filter, navigate, open a URL, etc. There is no `None` variant —
/// shells only deliver it when something happens.
#[derive(Debug, Clone, PartialEq)]
pub enum BadgeAction {
    /// Body click under [`BadgeClickKind::Toggle`], and every `×` click.
    Toggle {
        /// The badge's `field` prop.
        field: String,
        /// The badge's `value` prop.
        value: String,
    },
    /// Body click when the caller opted into raw-click semantics via
    /// [`BadgeClickKind::Clicked`].
    Clicked {
        /// The badge's `field` prop.
        field: String,
        /// The badge's `value` prop.
        value: String,
    },
    /// The explicit `+` affordance. Distinct from `Toggle` so call sites
    /// can route body-clicks to "focus this" and reserve `+` for "add this
    /// attribute to the filter set".
    AddFilter {
        /// The badge's `field` prop.
        field: String,
        /// The badge's `value` prop.
        value: String,
    },
    /// Body click on a [`BadgeKind::Wikilink`] badge.
    Navigate {
        /// The wikilink's `target`.
        target: String,
    },
    /// Body click on a [`BadgeKind::Url`] badge.
    OpenUrl {
        /// The URL's `href`.
        href: String,
    },
    /// Pointer entered the badge and the caller opted in via `emit_hover`.
    Hovered {
        /// The badge's `field` prop.
        field: String,
        /// The badge's `value` prop.
        value: String,
    },
}

/// Selects the click semantics for a non-Wikilink/Url body click.
/// Default is `Toggle` (back-compat with filter-toggle call sites).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BadgeClickKind {
    /// Body clicks emit [`BadgeAction::Toggle`] — for badges that drive a
    /// filter set. This is the default.
    #[default]
    Toggle,
    /// Body clicks emit [`BadgeAction::Clicked`] — raw click semantics for
    /// hosts that want to decide themselves.
    Clicked,
}

/// Stable hue derivation (0.0..1.0) for tag-like values. FNV-1a 32-bit —
/// small, deterministic, no extra deps — so hosts share one colour mapping
/// and tests can assert determinism without rendering.
///
/// # Examples
///
/// ```
/// let h = panel_kit_core::badge::tag_hue("project/alpha");
/// assert!((0.0..1.0).contains(&h));
/// assert_eq!(h, panel_kit_core::badge::tag_hue("project/alpha"));
/// ```
pub fn tag_hue(value: &str) -> f32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in value.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    (h % 360) as f32 / 360.0
}

/// The label a shell should draw for a badge: wikilinks get the `⟶`
/// prefix, URLs show their host (falling back to the value), everything
/// else shows the value as-is.
pub fn display_label(kind: &BadgeKind, value: &str) -> String {
    match kind {
        BadgeKind::Wikilink { .. } => format!("\u{27F6} {value}"),
        BadgeKind::Url { host, .. } => {
            if host.is_empty() {
                value.to_string()
            } else {
                host.clone()
            }
        }
        _ => value.to_string(),
    }
}
