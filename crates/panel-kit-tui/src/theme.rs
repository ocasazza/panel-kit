//! Terminal palette mirroring the web shell's `:root` CSS variables, so
//! the TUI looks like the same product. Override fields (or swap whole
//! presets) to retheme — the terminal twin of overriding the CSS vars.

use ratatui::style::Color;

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
    /// The default dark palette — the exact hex values from
    /// `panel-kit.css`.
    pub const DARK: Theme = Theme {
        bg: Color::Rgb(0x0a, 0x0a, 0x0a),
        panel: Color::Rgb(0x0d, 0x0d, 0x0d),
        fg: Color::Rgb(0xed, 0xed, 0xed),
        dim: Color::Rgb(0x7a, 0x7a, 0x7a),
        line: Color::Rgb(0x26, 0x26, 0x26),
        line2: Color::Rgb(0x3a, 0x3a, 0x3a),
        accent: Color::Rgb(0x5e, 0xf3, 0x8c),
        red: Color::Rgb(0xff, 0x5f, 0x56),
        yellow: Color::Rgb(0xff, 0xbd, 0x2e),
        green: Color::Rgb(0x27, 0xc9, 0x3f),
        blue: Color::Rgb(0x3b, 0x9b, 0xff),
        pink: Color::Rgb(0xff, 0x5f, 0xc3),
        badge_info: Color::Rgb(0x83, 0xb7, 0xcc),
    };

    /// A light "paper" preset — the terminal twin of the theming example's
    /// CSS-variable override path.
    pub const PAPER: Theme = Theme {
        bg: Color::Rgb(0xf4, 0xf1, 0xea),
        panel: Color::Rgb(0xfb, 0xf9, 0xf4),
        fg: Color::Rgb(0x1a, 0x1a, 0x1a),
        dim: Color::Rgb(0x6e, 0x66, 0x5c),
        line: Color::Rgb(0xd8, 0xd2, 0xc6),
        line2: Color::Rgb(0xbf, 0xb8, 0xa8),
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
