use super::tokens::{
    BACKGROUND, BORDER, DIVIDER, ERROR, FAILURE, HIGHLIGHT, INFO, INVERSE_BACKGROUND,
    INVERSE_TEXT, FOCUS_RING, MONO, PRIMARY, SECONDARY, SUCCESS, SURFACE, TERTIARY, TEXT, TEXT_MUTED,
    WARNING,
};
use super::{Color, ColorTokens, DensityTokens, TypographyTokens};

/// The dark palette, derived from the `Token` seed so the flat constants
/// and the structured token set cannot drift apart.
pub(super) fn dark_colors() -> ColorTokens {
    ColorTokens {
        background: BACKGROUND.color(),
        surface: SURFACE.color(),
        text: TEXT.color(),
        text_muted: TEXT_MUTED.color(),
        divider: DIVIDER.color(),
        border: BORDER.color(),
        inverse_background: INVERSE_BACKGROUND.color(),
        inverse_text: INVERSE_TEXT.color(),
        focus_ring: FOCUS_RING.color(),
        primary: PRIMARY.color(),
        secondary: SECONDARY.color(),
        tertiary: TERTIARY.color(),
        highlight: HIGHLIGHT.color(),
        success: SUCCESS.color(),
        failure: FAILURE.color(),
        error: ERROR.color(),
        warning: WARNING.color(),
        info: INFO.color(),
    }
}

/// The light "paper" palette that used to live only in the terminal preset.
pub(super) fn paper_colors() -> ColorTokens {
    ColorTokens {
        background: Color::new(0xf4, 0xf1, 0xea),
        surface: Color::new(0xfb, 0xf9, 0xf4),
        text: Color::new(0x1a, 0x1a, 0x1a),
        text_muted: Color::new(0x6e, 0x66, 0x5c),
        divider: Color::new(0xd8, 0xd2, 0xc6),
        border: Color::new(0x8c, 0x85, 0x78),
        inverse_background: Color::new(0x1a, 0x1a, 0x1a),
        inverse_text: Color::new(0xf4, 0xf1, 0xea),
        focus_ring: Color::new(0x1a, 0x1a, 0x1a),
        primary: Color::new(0x1f, 0x5e, 0xc2),
        secondary: Color::new(0xc2, 0x1f, 0x8e),
        tertiary: Color::new(0xb8, 0x86, 0x0b),
        highlight: Color::new(0x0c, 0x7a, 0x3d),
        success: Color::new(0x1d, 0x7a, 0x33),
        failure: Color::new(0xc6, 0x28, 0x28),
        error: Color::new(0xc6, 0x28, 0x28),
        warning: Color::new(0xb8, 0x86, 0x0b),
        info: Color::new(0x2e, 0x6e, 0x8c),
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
        panel_radius: 0.0,
        badge_radius: 999.0,
        spacing_xs: 4.0,
        spacing_sm: 6.0,
        spacing_md: 8.0,
    }
}