//! Terminal conversion for the shared panel-kit theme tokens.
//!
//! This module adapts semantic colours into ratatui colours. Typography and
//! density tokens are represented as explicit dispositions because terminal
//! hosts, not ratatui, own font selection, glyph metrics, and pixel radii.

mod disposition;

use panel_kit_core::theme::{Color as CoreColor, ThemeColor, ThemeTokens};
use ratatui::style::Color;

pub use disposition::{TuiDensityTheme, TuiThemeDisposition, TuiTypographyTheme};

/// The chrome palette resolved for ratatui painters.
///
/// Field names match the CSS variables in `assets/panel-kit.css`
/// (`--bg`, `--focus-ring`, …) so parity checks can walk the same
/// [`ThemeColor`] slots for web and terminal consumers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedTuiTheme {
    /// Page background (`--bg`).
    pub bg: Color,
    /// Panel background (`--panel`).
    pub panel: Color,
    /// Foreground text (`--fg`).
    pub fg: Color,
    /// De-emphasized text (`--dim`).
    pub dim: Color,
    /// Hairline borders (`--line`).
    pub line: Color,
    /// Stronger borders (`--line2`).
    pub line2: Color,
    /// Inverted background (`--inv-bg`).
    pub inverse_bg: Color,
    /// Inverted foreground (`--inv-fg`).
    pub inverse_fg: Color,
    /// Accent (`--accent`).
    pub accent: Color,
    /// Errors / unresolved (`--red`).
    pub red: Color,
    /// Minimize light (`--yellow`).
    pub yellow: Color,
    /// Success / URLs (`--green`).
    pub green: Color,
    /// Mode light (`--blue`).
    pub blue: Color,
    /// Maximize light (`--pink`).
    pub pink: Color,
    /// Informational badges (`--badge-info`).
    pub badge_info: Color,
    /// Keyboard focus indicator (`--focus-ring`).
    pub focus_ring: Color,
    /// How terminal rendering handles each typography token.
    pub typography: TuiTypographyTheme,
    /// How terminal rendering handles each density token.
    pub density: TuiDensityTheme,
}

impl ResolvedTuiTheme {
    /// Convert shared theme tokens into ratatui colours and token dispositions.
    pub fn from_tokens(tokens: &ThemeTokens) -> Self {
        Self {
            bg: rgb(tokens.colors.bg),
            panel: rgb(tokens.colors.panel),
            fg: rgb(tokens.colors.fg),
            dim: rgb(tokens.colors.dim),
            line: rgb(tokens.colors.line),
            line2: rgb(tokens.colors.line2),
            inverse_bg: rgb(tokens.colors.inverse_bg),
            inverse_fg: rgb(tokens.colors.inverse_fg),
            accent: rgb(tokens.colors.accent),
            red: rgb(tokens.colors.red),
            yellow: rgb(tokens.colors.yellow),
            green: rgb(tokens.colors.green),
            blue: rgb(tokens.colors.blue),
            pink: rgb(tokens.colors.pink),
            badge_info: rgb(tokens.colors.badge_info),
            focus_ring: rgb(tokens.colors.focus_ring),
            typography: TuiTypographyTheme::approximated(),
            density: TuiDensityTheme::approximated(),
        }
    }

    /// Resolve one semantic colour slot to its ratatui colour.
    pub fn color(&self, slot: ThemeColor) -> Color {
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

    /// Resolve one semantic colour slot back to core RGB channels.
    pub fn resolve(&self, slot: ThemeColor) -> CoreColor {
        match self.color(slot) {
            Color::Rgb(r, g, b) => CoreColor::new(r, g, b),
            other => panic!("resolved TUI theme used non-RGB color {other:?}"),
        }
    }
}

impl From<&ThemeTokens> for ResolvedTuiTheme {
    fn from(tokens: &ThemeTokens) -> Self {
        Self::from_tokens(tokens)
    }
}

impl From<ThemeTokens> for ResolvedTuiTheme {
    fn from(tokens: ThemeTokens) -> Self {
        Self::from_tokens(&tokens)
    }
}

impl Default for ResolvedTuiTheme {
    fn default() -> Self {
        Self::from_tokens(&ThemeTokens::dark())
    }
}

fn rgb(color: CoreColor) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}
