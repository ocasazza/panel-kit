use serde::{Deserialize, Serialize};

use super::Color;

/// One of the semantic colour slots every palette must fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeColor {
    /// Page background.
    Background,
    /// Raised surfaces.
    Surface,
    /// Body and title text.
    Text,
    /// De-emphasized text.
    TextMuted,
    /// Structural hairline dividing regions.
    Divider,
    /// Object outline.
    Border,
    /// Inverted background, used by selection.
    InverseBackground,
    /// Inverted foreground, used by selection.
    InverseText,
    /// Keyboard-focus indicator.
    FocusRing,
    /// Floating/tiling mode light.
    Primary,
    /// Maximize/restore light.
    Secondary,
    /// Minimize light.
    Tertiary,
    /// Live-system signal.
    Highlight,
    /// Verdict: resolved, valid, reachable.
    Success,
    /// Failed checks and verdicts.
    Failure,
    /// Errors and unresolved references.
    Error,
    /// Warnings and thresholds.
    Warning,
    /// Informational metadata badges.
    Info,
}

impl ThemeColor {
    /// Every colour slot, in declaration order.
pub const ALL: &'static [ThemeColor] = &[
        ThemeColor::Background,
        ThemeColor::Surface,
        ThemeColor::Text,
        ThemeColor::TextMuted,
        ThemeColor::Divider,
        ThemeColor::Border,
        ThemeColor::InverseBackground,
        ThemeColor::InverseText,
        ThemeColor::FocusRing,
        ThemeColor::Primary,
        ThemeColor::Secondary,
        ThemeColor::Tertiary,
        ThemeColor::Highlight,
        ThemeColor::Success,
        ThemeColor::Failure,
        ThemeColor::Error,
        ThemeColor::Warning,
        ThemeColor::Info,
    ];

    /// The CSS custom property for this slot, e.g. `"--color-background"`.
    pub const fn css_var(self) -> &'static str {
        match self {
            ThemeColor::Background => "--color-background",
            ThemeColor::Surface => "--color-surface",
            ThemeColor::Text => "--color-text",
            ThemeColor::TextMuted => "--color-text-muted",
            ThemeColor::Divider => "--color-divider",
            ThemeColor::Border => "--color-border",
            ThemeColor::InverseBackground => "--color-inverse-background",
            ThemeColor::InverseText => "--color-inverse-text",
            ThemeColor::FocusRing => "--color-focus-ring",
            ThemeColor::Primary => "--color-primary",
            ThemeColor::Secondary => "--color-secondary",
            ThemeColor::Tertiary => "--color-tertiary",
            ThemeColor::Highlight => "--color-highlight",
            ThemeColor::Success => "--color-success",
            ThemeColor::Failure => "--color-failure",
            ThemeColor::Error => "--color-error",
            ThemeColor::Warning => "--color-warning",
            ThemeColor::Info => "--color-info",
        }
    }

    /// The token key for this slot, e.g. `"color-background"`.
    pub const fn key(self) -> &'static str {
        match self {
            ThemeColor::Background => "color-background",
            ThemeColor::Surface => "color-surface",
            ThemeColor::Text => "color-text",
            ThemeColor::TextMuted => "color-text-muted",
            ThemeColor::Divider => "color-divider",
            ThemeColor::Border => "color-border",
            ThemeColor::InverseBackground => "color-inverse-background",
            ThemeColor::InverseText => "color-inverse-text",
            ThemeColor::FocusRing => "color-focus-ring",
            ThemeColor::Primary => "color-primary",
            ThemeColor::Secondary => "color-secondary",
            ThemeColor::Tertiary => "color-tertiary",
            ThemeColor::Highlight => "color-highlight",
            ThemeColor::Success => "color-success",
            ThemeColor::Failure => "color-failure",
            ThemeColor::Error => "color-error",
            ThemeColor::Warning => "color-warning",
            ThemeColor::Info => "color-info",
        }
    }
}

/// A colour resolved either as a literal or through a [`ThemeColor`] slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", content = "value", rename_all = "snake_case")]
pub enum ColorRef {
    /// An explicit colour value.
    Literal(Color),
    /// A lookup into the active theme.
    Theme(ThemeColor),
}