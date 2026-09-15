//! Badge color math shared by every renderer.

use crate::badge::Rgb;

/// HSL(hue, 0.55, 0.55) converted to RGB for stable tag tinting.
pub fn hue_color(hue: f32) -> Rgb {
    let (s, l) = (0.55_f32, 0.55_f32);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h6 = hue.rem_euclid(1.0) * 6.0;
    let x = c * (1.0 - (h6 % 2.0 - 1.0).abs());
    let (r, g, b) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;

    (((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8)
}

/// Rec. 709 luma in `[0, 1]` for contrast decisions.
pub fn perceived_brightness((r, g, b): Rgb) -> f32 {
    0.2126 * (r as f32 / 255.0) + 0.7152 * (g as f32 / 255.0) + 0.0722 * (b as f32 / 255.0)
}

/// Opaquely blend `over` at strength `alpha` on top of `base`.
pub fn tint_over(base: Rgb, over: Rgb, alpha: f32) -> Rgb {
    let alpha = alpha.clamp(0.0, 1.0);
    let mix = |base: u8, over: u8| -> u8 { (base as f32 * (1.0 - alpha) + over as f32 * alpha).round() as u8 };

    (mix(base.0, over.0), mix(base.1, over.1), mix(base.2, over.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hue_color_matches_tui_fixed_values() {
        assert_eq!(hue_color(0.0), (203, 77, 77));
        assert_eq!(hue_color(1.0 / 3.0), (77, 203, 77));
        assert_eq!(hue_color(2.0 / 3.0), (77, 77, 203));
        assert_eq!(hue_color(1.25), hue_color(0.25));
    }

    #[test]
    fn contrast_math_matches_web_fixed_values() {
        assert_eq!(perceived_brightness((255, 255, 255)), 1.0);
        assert_eq!(perceived_brightness((0, 0, 0)), 0.0);
        assert!((perceived_brightness((255, 0, 0)) - 0.2126).abs() < f32::EPSILON);
        assert_eq!(tint_over((10, 20, 30), (110, 120, 130), 0.30), (40, 50, 60));
        assert_eq!(tint_over((10, 20, 30), (110, 120, 130), -1.0), (10, 20, 30));
        assert_eq!(tint_over((10, 20, 30), (110, 120, 130), 2.0), (110, 120, 130));
    }
}
