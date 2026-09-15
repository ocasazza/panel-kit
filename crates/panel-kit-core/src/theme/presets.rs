use super::tokens::{ACCENT, BADGE_INFO, BG, BLUE, DIM, FG, GREEN, INV_BG, INV_FG, LINE, LINE2, MONO, PANEL, PINK, RED, YELLOW};
use super::{Color, ColorTokens, DensityTokens, TypographyTokens};

/// The dark palette, derived from the `Token` seed so the flat constants
/// and the structured token set cannot drift apart.
pub(super) fn dark_colors() -> ColorTokens {
    ColorTokens {
        bg: BG.color(),
        panel: PANEL.color(),
        fg: FG.color(),
        dim: DIM.color(),
        line: LINE.color(),
        line2: LINE2.color(),
        inverse_bg: INV_BG.color(),
        inverse_fg: INV_FG.color(),
        accent: ACCENT.color(),
        red: RED.color(),
        yellow: YELLOW.color(),
        green: GREEN.color(),
        blue: BLUE.color(),
        pink: PINK.color(),
        badge_info: BADGE_INFO.color(),
        // `--focus-ring: var(--fg)` in the web sheet: a distinct semantic
        // whose dark default happens to alias the foreground.
        focus_ring: FG.color(),
    }
}

/// The light "paper" palette that used to live only in the terminal preset.
pub(super) fn paper_colors() -> ColorTokens {
    ColorTokens {
        bg: Color::new(0xf4, 0xf1, 0xea),
        panel: Color::new(0xfb, 0xf9, 0xf4),
        fg: Color::new(0x1a, 0x1a, 0x1a),
        dim: Color::new(0x6e, 0x66, 0x5c),
        line: Color::new(0xd8, 0xd2, 0xc6),
        line2: Color::new(0x8c, 0x85, 0x78),
        // The terminal preset carries no inverse pair; the web paper preset
        // does, as the same fg/bg swap the dark palette inverts with.
        inverse_bg: Color::new(0x1a, 0x1a, 0x1a),
        inverse_fg: Color::new(0xf4, 0xf1, 0xea),
        accent: Color::new(0x0c, 0x7a, 0x3d),
        red: Color::new(0xc6, 0x28, 0x28),
        yellow: Color::new(0xb8, 0x86, 0x0b),
        green: Color::new(0x1d, 0x7a, 0x33),
        blue: Color::new(0x1f, 0x5e, 0xc2),
        pink: Color::new(0xc2, 0x1f, 0x8e),
        badge_info: Color::new(0x2e, 0x6e, 0x8c),
        focus_ring: Color::new(0x1a, 0x1a, 0x1a),
    }
}

/// The documented canonical typography tokens.
pub(super) fn typography() -> TypographyTokens {
    TypographyTokens {
        family: MONO.to_string(),
        body_size: 13.0,
        body_line_height: 1.5,
        label_size: 0.72,
        label_weight: 700,
        label_tracking: 0.06,
    }
}

/// The documented canonical density tokens.
pub(super) fn density() -> DensityTokens {
    DensityTokens {
        panel_radius: 4.0,
        badge_radius: 999.0,
        spacing_xs: 4.0,
        spacing_sm: 6.0,
        spacing_md: 8.0,
    }
}
