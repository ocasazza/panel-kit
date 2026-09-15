use serde::{Deserialize, Serialize};

use super::Color;

/// One of the semantic colour slots every palette must fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeColor {
    /// Page background, and the recessed field behind chips and badges.
    Bg,
    /// Every raised surface: panels, topbar, dock, tooltip.
    Panel,
    /// Body and title text.
    Fg,
    /// De-emphasized text.
    Dim,
    /// Structural hairline dividing regions.
    Line,
    /// Object outline.
    Line2,
    /// Inverted background, used by selection.
    InverseBg,
    /// Inverted foreground, used by selection.
    InverseFg,
    /// Live-system signal only.
    Accent,
    /// Errors and unresolved references.
    Red,
    /// Minimize light.
    Yellow,
    /// Verdict: resolved, valid, reachable.
    Green,
    /// Floating/tiling mode light.
    Blue,
    /// Maximize/restore light.
    Pink,
    /// Informational metadata badges, and the resting tint for tags.
    BadgeInfo,
    /// The keyboard-focus indicator.
    FocusRing,
}

impl ThemeColor {
    /// Every slot, in [`crate::theme::ColorTokens`] declaration order.
    ///
    /// Consumers that generate a palette (and the completeness test) iterate
    /// this rather than naming variants, so a slot added here reaches every
    /// consumer without an edit elsewhere — the same rule as the `DARK` seed
    /// slice.
    pub const ALL: &'static [ThemeColor] = &[
        ThemeColor::Bg,
        ThemeColor::Panel,
        ThemeColor::Fg,
        ThemeColor::Dim,
        ThemeColor::Line,
        ThemeColor::Line2,
        ThemeColor::InverseBg,
        ThemeColor::InverseFg,
        ThemeColor::Accent,
        ThemeColor::Red,
        ThemeColor::Yellow,
        ThemeColor::Green,
        ThemeColor::Blue,
        ThemeColor::Pink,
        ThemeColor::BadgeInfo,
        ThemeColor::FocusRing,
    ];

    /// The CSS custom-property key for this slot, without the leading `--`.
    ///
    /// This is the spelling the web `:root` block and the `DARK` seed names
    /// use (`inv-bg`, `badge-info`, `focus-ring`), not the serde spelling.
    pub const fn key(self) -> &'static str {
        match self {
            ThemeColor::Bg => "bg",
            ThemeColor::Panel => "panel",
            ThemeColor::Fg => "fg",
            ThemeColor::Dim => "dim",
            ThemeColor::Line => "line",
            ThemeColor::Line2 => "line2",
            ThemeColor::InverseBg => "inv-bg",
            ThemeColor::InverseFg => "inv-fg",
            ThemeColor::Accent => "accent",
            ThemeColor::Red => "red",
            ThemeColor::Yellow => "yellow",
            ThemeColor::Green => "green",
            ThemeColor::Blue => "blue",
            ThemeColor::Pink => "pink",
            ThemeColor::BadgeInfo => "badge-info",
            ThemeColor::FocusRing => "focus-ring",
        }
    }
}

/// A colour in an authored spec: a literal, or a reference to a semantic
/// slot the resolved theme fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", content = "value", rename_all = "snake_case")]
pub enum ColorRef {
    /// A literal `#rrggbb` colour.
    Literal(Color),
    /// A semantic slot, resolved against the active palette.
    Theme(ThemeColor),
}
