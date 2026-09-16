//! TUI painters for the shared chart and gauge models.
//!
//! Renderer-neutral chart data types are imported from
//! `panel_kit_core::widgets::charts`; this module keeps only ratatui painting.

mod boxplot;
mod boxplot_render;
mod flame;
mod gauges;
mod time_series;

pub use boxplot::boxplot;
pub use flame::flame;
pub use gauges::gauges;
pub use time_series::time_series;

use panel_kit_core::badge::Rgb;
use ratatui::style::Color;

use crate::ResolvedTuiTheme;

/// Blend two colors by `t` in `0..=1` (0 = `a`, 1 = `b`). Non-RGB colors
/// fall back to `a`.
pub(super) fn mix(a: Color, b: Color, t: f64) -> Color {
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            let t = t.clamp(0.0, 1.0);
            let lerp =
                |from: u8, to: u8| (from as f64 + (to as f64 - from as f64) * t).round() as u8;
            Color::Rgb(lerp(ar, br), lerp(ag, bg), lerp(ab, bb))
        }
        _ => a,
    }
}

/// The categorical series palette, derived from the theme.
pub(super) fn series_colors(t: &ResolvedTuiTheme) -> [Color; 6] {
    [t.fg, t.blue, t.pink, t.yellow, t.badge_info, t.red]
}

pub(super) fn rgb_color((r, g, b): Rgb) -> Color {
    Color::Rgb(r, g, b)
}

pub(super) fn contrast_ink(bg: Color, t: &ResolvedTuiTheme) -> Color {
    match bg {
        Color::Rgb(r, g, b) => {
            // Rec. 601 luma; light cells get dark ink and vice versa.
            let luma = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
            if luma > 140.0 {
                t.bg
            } else {
                t.fg
            }
        }
        _ => t.fg,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn chart_statistics_live_in_core() {
        let summary = panel_kit_core::widgets::charts::five_num(&[7.0, 1.0, 3.0, 9.0, 5.0])
            .expect("sample set has a summary");

        assert_eq!(summary.median, 5.0);
    }
}
