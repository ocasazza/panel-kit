//! Terminal palettes for the web shell's design language. The dark preset is
//! derived from [`panel_kit_core::tokens`], while only the light preset is
//! hand-authored. Override fields (or swap whole presets) to retheme — the
//! terminal twin of overriding the CSS variables.

use ratatui::style::Color;

use panel_kit_core::tokens::{self, Token};

const fn rgb(token: Token) -> Color {
    Color::Rgb(token.rgb.0, token.rgb.1, token.rgb.2)
}

/// The chrome palette. Field names match the CSS variables in
/// `assets/panel-kit.css` (`--bg`, `--accent`, …).
#[derive(Clone, Copy)]
pub struct Theme {
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
}

impl Theme {
    /// The default dark palette, derived from the canonical core tokens.
    pub const DARK: Theme = Theme {
        bg: rgb(tokens::BG),
        panel: rgb(tokens::PANEL),
        fg: rgb(tokens::FG),
        dim: rgb(tokens::DIM),
        line: rgb(tokens::LINE),
        line2: rgb(tokens::LINE2),
        accent: rgb(tokens::ACCENT),
        red: rgb(tokens::RED),
        yellow: rgb(tokens::YELLOW),
        green: rgb(tokens::GREEN),
        blue: rgb(tokens::BLUE),
        pink: rgb(tokens::PINK),
        badge_info: rgb(tokens::BADGE_INFO),
    };

    /// A light "paper" preset — the terminal twin of the theming example's
    /// CSS-variable override path.
    pub const PAPER: Theme = Theme {
        // Script-computed WCAG ratios: line2/panel 3.4763:1,
        // dim/bg 5.0071:1, and dim/panel 5.3679:1.
        bg: Color::Rgb(0xf4, 0xf1, 0xea),
        panel: Color::Rgb(0xfb, 0xf9, 0xf4),
        fg: Color::Rgb(0x1a, 0x1a, 0x1a),
        dim: Color::Rgb(0x6e, 0x66, 0x5c),
        line: Color::Rgb(0xd8, 0xd2, 0xc6),
        line2: Color::Rgb(0x8c, 0x85, 0x78),
        accent: Color::Rgb(0x0c, 0x7a, 0x3d),
        red: Color::Rgb(0xc6, 0x28, 0x28),
        yellow: Color::Rgb(0xb8, 0x86, 0x0b),
        green: Color::Rgb(0x1d, 0x7a, 0x33),
        blue: Color::Rgb(0x1f, 0x5e, 0xc2),
        pink: Color::Rgb(0xc2, 0x1f, 0x8e),
        badge_info: Color::Rgb(0x2e, 0x6e, 0x8c),
    };
}

impl Default for Theme {
    fn default() -> Self {
        Self::DARK
    }
}
