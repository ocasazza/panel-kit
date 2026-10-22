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
/// Field names are the snake_case form of the `--color-<role>` CSS variables
/// in `assets/panel-kit.css` (`--color-text-muted`, `--color-focus-ring`, …) so
/// parity checks can walk the same [`ThemeColor`] slots for web and terminal
/// consumers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedTuiTheme {
    /// Page background (`--color-background`).
    pub background: Color,
    /// Panel background (`--color-surface`).
    pub surface: Color,
    /// Primary text (`--color-text`).
    pub text: Color,
    /// De-emphasized text (`--color-text-muted`).
    pub text_muted: Color,
    /// Hairline dividers (`--color-divider`).
    pub divider: Color,
    /// Stronger borders (`--color-border`).
    pub border: Color,
    /// Inverted background (`--color-inverse-background`).
    pub inverse_background: Color,
    /// Inverted text (`--color-inverse-text`).
    pub inverse_text: Color,
    /// Keyboard focus indicator (`--color-focus-ring`).
    pub focus_ring: Color,
    /// Primary brand colour / mode light (`--color-primary`).
    pub primary: Color,
    /// Secondary colour / maximize light (`--color-secondary`).
    pub secondary: Color,
    /// Tertiary colour / minimize light (`--color-tertiary`).
    pub tertiary: Color,
    /// Live-state highlight (`--color-highlight`).
    pub highlight: Color,
    /// Success / URLs (`--color-success`).
    pub success: Color,
    /// Failed checks and verdicts (`--color-failure`).
    pub failure: Color,
    /// Errors / unresolved (`--color-error`).
    pub error: Color,
    /// Warnings and thresholds (`--color-warning`).
    pub warning: Color,
    /// Informational badges (`--color-info`).
    pub info: Color,
    /// How terminal rendering handles each typography token.
    pub typography: TuiTypographyTheme,
    /// How terminal rendering handles each density token.
    pub density: TuiDensityTheme,
}

impl ResolvedTuiTheme {
    /// Convert shared theme tokens into ratatui colours and token dispositions.
    pub fn from_tokens(tokens: &ThemeTokens) -> Self {
        Self {
            background: rgb(tokens.colors.background),
            surface: rgb(tokens.colors.surface),
            text: rgb(tokens.colors.text),
            text_muted: rgb(tokens.colors.text_muted),
            divider: rgb(tokens.colors.divider),
            border: rgb(tokens.colors.border),
            inverse_background: rgb(tokens.colors.inverse_background),
            inverse_text: rgb(tokens.colors.inverse_text),
            focus_ring: rgb(tokens.colors.focus_ring),
            primary: rgb(tokens.colors.primary),
            secondary: rgb(tokens.colors.secondary),
            tertiary: rgb(tokens.colors.tertiary),
            highlight: rgb(tokens.colors.highlight),
            success: rgb(tokens.colors.success),
            failure: rgb(tokens.colors.failure),
            error: rgb(tokens.colors.error),
            warning: rgb(tokens.colors.warning),
            info: rgb(tokens.colors.info),
            typography: TuiTypographyTheme::approximated(),
            density: TuiDensityTheme::approximated(),
        }
    }

    /// Resolve one semantic colour slot to its ratatui colour.
    pub fn color(&self, slot: ThemeColor) -> Color {
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
