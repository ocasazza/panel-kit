//! Fill-bar / meter component — the `████░░░░` capacity bar used for
//! utilization, queue depth, and progress readouts. The string builder is the
//! reusable core; [`span`] wraps it in a themed color for inline use.

use panel_kit_core::badge::Rgb;
use ratatui::style::{Color, Style};
use ratatui::text::Span;

/// A horizontal fill bar `width` cells wide: `frac` (clamped to `0.0..=1.0`)
/// rendered with `█`, the remainder with `░`.
pub fn bar(frac: f64, width: usize) -> String {
    let filled = (frac.clamp(0.0, 1.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    "█".repeat(filled) + &"░".repeat(width - filled)
}

/// The [`bar`] as a colored [`Span`], for placing inline in a [`Line`].
///
/// `color` uses the renderer-neutral RGB shape shared with other backends;
/// conversion to ratatui's [`Color`] happens here at the draw boundary.
///
/// [`Line`]: ratatui::text::Line
pub fn span(frac: f64, width: usize, color: Rgb) -> Span<'static> {
    let (r, g, b) = color;
    Span::styled(bar(frac, width), Style::default().fg(Color::Rgb(r, g, b)))
}
