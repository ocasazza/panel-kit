use serde::{Deserialize, Serialize};

use super::Color;

/// One of the semantic colour slots every palette must fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeColor {
    Background,
    Surface,
    Text,
    TextMuted,
    Divider,
    Border,
    InverseBackground,
    InverseText,
    FocusRing,
    Primary,
    Secondary,
    Tertiary,
    Highlight,
    Success,
    Failure,
    Error,
    Warning,
    Info,
}

impl ThemeColor {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", content = "value", rename_all = "snake_case")]
pub enum ColorRef {
    Literal(Color),
    Theme(ThemeColor),
}