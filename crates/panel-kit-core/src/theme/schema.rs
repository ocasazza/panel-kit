use serde::{Deserialize, Serialize};

use super::presets::{dark_colors, density, paper_colors, typography};
use super::{Color, ThemeColor};

/// A complete palette: one colour per [`ThemeColor`] slot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ColorTokens {
    /// Page background.
    pub bg: Color,
    /// Raised surfaces.
    pub panel: Color,
    /// Body and title text.
    pub fg: Color,
    /// De-emphasized text.
    pub dim: Color,
    /// Region hairlines.
    pub line: Color,
    /// Object outlines.
    pub line2: Color,
    /// Inverted background (selection).
    pub inverse_bg: Color,
    /// Inverted foreground (selection).
    pub inverse_fg: Color,
    /// Live-system signal.
    pub accent: Color,
    /// Errors and unresolved references.
    pub red: Color,
    /// Minimize light.
    pub yellow: Color,
    /// Verdict: resolved, valid, reachable.
    pub green: Color,
    /// Floating/tiling mode light.
    pub blue: Color,
    /// Maximize/restore light.
    pub pink: Color,
    /// Informational badges and tag tints.
    pub badge_info: Color,
    /// The keyboard-focus indicator.
    pub focus_ring: Color,
}

impl ColorTokens {
    /// The colour filling a semantic slot.
    ///
    /// This is the resolution [`crate::theme::ColorRef::Theme`] performs
    /// against a resolved palette, and the lookup the generated CSS emitter
    /// uses.
    pub const fn resolve(&self, slot: ThemeColor) -> Color {
        match slot {
            ThemeColor::Bg => self.bg,
            ThemeColor::Panel => self.panel,
            ThemeColor::Fg => self.fg,
            ThemeColor::Dim => self.dim,
            ThemeColor::Line => self.line,
            ThemeColor::Line2 => self.line2,
            ThemeColor::InverseBg => self.inverse_bg,
            ThemeColor::InverseFg => self.inverse_fg,
            ThemeColor::Accent => self.accent,
            ThemeColor::Red => self.red,
            ThemeColor::Yellow => self.yellow,
            ThemeColor::Green => self.green,
            ThemeColor::Blue => self.blue,
            ThemeColor::Pink => self.pink,
            ThemeColor::BadgeInfo => self.badge_info,
            ThemeColor::FocusRing => self.focus_ring,
        }
    }
}

/// The type system. One family across every role is a design rule, so the
/// family is a token like any colour; sizes are the documented canonical
/// values (body in px, label in rem, tracking in em).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TypographyTokens {
    /// The one font family, as a CSS `font-family` stack.
    pub family: String,
    /// Body size in px.
    pub body_size: f64,
    /// Body line-height ratio.
    pub body_line_height: f64,
    /// Label (panel-title) size in rem.
    pub label_size: f64,
    /// Label weight.
    pub label_weight: u16,
    /// Label letter-spacing in em.
    pub label_tracking: f64,
}

/// Radii and spacing, in px.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DensityTokens {
    /// Panel corner radius.
    pub panel_radius: f64,
    /// Badge pill radius (`999` is the fully rounded stadium).
    pub badge_radius: f64,
    /// Extra-small spacing.
    pub spacing_xs: f64,
    /// Small spacing.
    pub spacing_sm: f64,
    /// Medium spacing.
    pub spacing_md: f64,
}

/// One complete theme: palette, type system, and density.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ThemeTokens {
    /// The palette.
    pub colors: ColorTokens,
    /// The type system.
    pub typography: TypographyTokens,
    /// Radii and spacing.
    pub density: DensityTokens,
}

impl ThemeTokens {
    /// The canonical dark preset — the palette every shipped surface uses
    /// today, derived from the `Token` seed.
    pub fn dark() -> Self {
        Self {
            colors: dark_colors(),
            typography: typography(),
            density: density(),
        }
    }

    /// The light "paper" preset, seeded in core for the first time.
    pub fn paper() -> Self {
        Self {
            colors: paper_colors(),
            typography: typography(),
            density: density(),
        }
    }
}
