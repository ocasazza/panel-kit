use serde::{Deserialize, Serialize};

use super::presets::{dark_colors, density, paper_colors, typography};
use super::{Color, ThemeColor};

/// A complete palette: one colour per [`ThemeColor`] slot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ColorTokens {
    /// Page background.
    pub background: Color,
    /// Raised surfaces.
    pub surface: Color,
    /// Body and title text.
    pub text: Color,
    /// De-emphasized text.
    pub text_muted: Color,
    /// Structural hairline dividing regions.
    pub divider: Color,
    /// Object outline.
    pub border: Color,
    /// Inverted background, used by selection.
    pub inverse_background: Color,
    /// Inverted foreground, used by selection.
    pub inverse_text: Color,
    /// Keyboard-focus indicator.
    pub focus_ring: Color,
    /// Primary brand colour / mode light.
    pub primary: Color,
    /// Secondary colour / maximize light.
    pub secondary: Color,
    /// Tertiary colour / minimize light.
    pub tertiary: Color,
    /// Live-system signal.
    pub highlight: Color,
    /// Verdict: resolved, valid, reachable.
    pub success: Color,
    /// Failed checks and verdicts.
    pub failure: Color,
    /// Errors and unresolved references.
    pub error: Color,
    /// Warnings and thresholds.
    pub warning: Color,
    /// Informational metadata badges, and the resting tint for tags.
    pub info: Color,
}

impl ColorTokens {
    /// The colour filling a semantic slot.
    pub const fn resolve(&self, slot: ThemeColor) -> Color {
        match slot {
            ThemeColor::Background => self.background,
            ThemeColor::Surface => self.surface,
            ThemeColor::Text => self.text,
            ThemeColor::TextMuted => self.text_muted,
            ThemeColor::Divider => self.divider,
            ThemeColor::Border => self.border,
            ThemeColor::InverseBackground => self.inverse_background,
            ThemeColor::InverseText => self.inverse_text,
            ThemeColor::FocusRing => self.focus_ring,
            ThemeColor::Primary => self.primary,
            ThemeColor::Secondary => self.secondary,
            ThemeColor::Tertiary => self.tertiary,
            ThemeColor::Highlight => self.highlight,
            ThemeColor::Success => self.success,
            ThemeColor::Failure => self.failure,
            ThemeColor::Error => self.error,
            ThemeColor::Warning => self.warning,
            ThemeColor::Info => self.info,
        }
    }
}
/// The type system.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TypographyTokens {
    pub family: String,
    pub body_size: f64,
    pub body_line_height: f64,
    pub label_size: f64,
    pub label_weight: u16,
    pub label_tracking: f64,
}

/// Radii and spacing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DensityTokens {
    pub panel_radius: f64,
    pub badge_radius: f64,
    pub spacing_xs: f64,
    pub spacing_sm: f64,
    pub spacing_md: f64,
}

/// One complete theme.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ThemeTokens {
    pub colors: ColorTokens,
    pub typography: TypographyTokens,
    pub density: DensityTokens,
}

impl ThemeTokens {
    pub fn dark() -> Self {
        Self { colors: dark_colors(), typography: typography(), density: density() }
    }
    pub fn paper() -> Self {
        Self { colors: paper_colors(), typography: typography(), density: density() }
    }
}
