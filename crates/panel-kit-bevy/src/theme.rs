//! Bevy conversion for the shared panel-kit theme tokens.
//!
//! This module adapts semantic colours into `bevy` colours and the typography
//! and density tokens into logical-pixel metrics. It mirrors `panel-kit-tui`'s
//! `ResolvedTuiTheme`: the same [`ThemeColor`] slots resolved once from
//! [`ThemeTokens`], so the web, terminal, and Bevy renderers paint the same
//! roles from the same palette and type scale.

use bevy::prelude::Color;
use bevy::prelude::Resource;
use panel_kit_core::theme::{Color as CoreColor, PanelPaint, ThemeColor, ThemeTokens};

/// The chrome palette resolved for Bevy painters.
///
/// Field names are the snake_case form of the `--color-<role>` CSS variables
/// in `assets/panel-kit.css` (`--color-text-muted`, `--color-focus-ring`, …) so
/// parity checks can walk the same [`ThemeColor`] slots for web, terminal, and
/// Bevy consumers.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct BevyTheme {
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
    /// Type and spacing metrics in logical pixels.
    pub metrics: ThemeMetrics,
    /// Panel chrome geometry in logical pixels.
    pub panel: PanelPaint,
}

/// Typography and density tokens resolved to logical pixels, the units the
/// web stylesheet uses (`--body-size`, `--label-size`, `--space-*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeMetrics {
    /// Root type size; `rem` multiples resolve against it (`--body-size`).
    pub rem: f32,
    /// Body text size (`--body-size`).
    pub body_size: f32,
    /// Body line height as a multiple of the font size.
    pub line_height: f32,
    /// Label and caption size (`--label-size`).
    pub label_size: f32,
    /// Label letter spacing as a multiple of the font size.
    pub label_tracking: f32,
    /// Whether labels use the bold face (`--label-weight` >= 600).
    pub label_bold: bool,
    /// `--space-xs`.
    pub space_xs: f32,
    /// `--space-sm`.
    pub space_sm: f32,
    /// `--space-md`.
    pub space_md: f32,
    /// `--panel-radius`.
    pub panel_radius: f32,
    /// `--badge-radius`.
    pub badge_radius: f32,
}

impl ThemeMetrics {
    /// Resolve the typography and density tokens.
    pub fn from_tokens(tokens: &ThemeTokens) -> Self {
        let typography = &tokens.typography;
        let density = tokens.density;
        let rem = typography.body_size as f32;
        Self {
            rem,
            body_size: rem,
            line_height: typography.body_line_height as f32,
            label_size: typography.label_size as f32 * rem,
            label_tracking: typography.label_tracking as f32,
            label_bold: typography.label_weight >= 600,
            space_xs: density.spacing_xs as f32,
            space_sm: density.spacing_sm as f32,
            space_md: density.spacing_md as f32,
            panel_radius: density.panel_radius as f32,
            badge_radius: density.badge_radius as f32,
        }
    }

    /// `factor` rem in logical pixels.
    pub fn rem(&self, factor: f32) -> f32 {
        factor * self.rem
    }
}

impl BevyTheme {
    /// Convert shared theme tokens into `bevy` colours.
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
            metrics: ThemeMetrics::from_tokens(tokens),
            panel: PanelPaint::css_px(tokens),
        }
    }

    /// Resolve one semantic colour slot to its `bevy` colour.
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
}

impl Default for BevyTheme {
    fn default() -> Self {
        Self::from_tokens(&ThemeTokens::dark())
    }
}

fn rgb(color: CoreColor) -> Color {
    Color::srgb_u8(color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `from_tokens` maps every token colour exactly, and `color` resolves
    /// each [`ThemeColor`] slot to the field of the same role.
    #[test]
    fn from_tokens_maps_every_slot_and_color_resolves_it_back() {
        for tokens in [ThemeTokens::dark(), ThemeTokens::paper()] {
            let theme = BevyTheme::from_tokens(&tokens);
            for slot in ThemeColor::ALL {
                assert_eq!(
                    theme.color(*slot),
                    rgb(tokens.colors.resolve(*slot)),
                    "{slot:?} does not map the token colour exactly"
                );
            }
        }
    }

    /// The two shipped presets produce visibly different themes, so a theme
    /// swap is observable through the resource.
    #[test]
    fn dark_and_paper_presets_differ() {
        assert_ne!(
            BevyTheme::from_tokens(&ThemeTokens::dark()),
            BevyTheme::from_tokens(&ThemeTokens::paper())
        );
    }

    /// The default theme is the dark seed, matching the web and TUI defaults.
    #[test]
    fn default_is_the_dark_seed() {
        assert_eq!(BevyTheme::default(), BevyTheme::from_tokens(&ThemeTokens::dark()));
    }

    /// Every metric follows its typography or density token, and labels
    /// resolve `label_size` in rem of `body_size`, as `--label-size` does.
    #[test]
    fn metrics_follow_typography_and_density_tokens() {
        let mut tokens = ThemeTokens::dark();
        tokens.typography.body_size = 15.0;
        tokens.typography.body_line_height = 1.25;
        tokens.typography.label_size = 0.8;
        tokens.typography.label_tracking = 0.1;
        tokens.typography.label_weight = 500;
        tokens.density.spacing_xs = 3.0;
        tokens.density.spacing_sm = 7.0;
        tokens.density.spacing_md = 11.0;
        tokens.density.panel_radius = 2.0;
        tokens.density.badge_radius = 9.0;
        let metrics = BevyTheme::from_tokens(&tokens).metrics;
        assert_eq!(metrics.rem(1.0), 15.0);
        assert_eq!(metrics.body_size, 15.0);
        assert_eq!(metrics.line_height, 1.25);
        assert_eq!(metrics.label_size, 12.0);
        assert_eq!(metrics.label_tracking, 0.1);
        assert!(!metrics.label_bold);
        assert_eq!(
            (metrics.space_xs, metrics.space_sm, metrics.space_md),
            (3.0, 7.0, 11.0)
        );
        assert_eq!((metrics.panel_radius, metrics.badge_radius), (2.0, 9.0));
        assert_eq!(BevyTheme::from_tokens(&tokens).panel, PanelPaint::css_px(&tokens));
    }
}
