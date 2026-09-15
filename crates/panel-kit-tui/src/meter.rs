//! Fill-bar / meter painter — the `████░░░░` capacity bar used for
//! utilization, queue depth, and progress readouts. Fill math and semantic
//! meter data live in [`panel_kit_core::widgets::meter`]; this module only
//! adapts those models into ratatui spans.

use panel_kit_core::widgets::meter::{bar, MeterModel};

use panel_kit_core::badge::Rgb;
use ratatui::style::{Color, Style};
use ratatui::text::Span;

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

/// Render a shared core [`MeterModel`] as an inline colored fill span.
///
/// When the model carries no color, the span uses the surrounding cell style.
pub fn span_model(model: &MeterModel, width: usize) -> Span<'static> {
    match model.color {
        Some(color) => span(model.ratio, width, color),
        None => Span::raw(bar(model.ratio, width)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_bar_reuses_core_fill_math() {
        assert_eq!(bar(0.76, 4), panel_kit_core::widgets::meter::bar(0.76, 4));
    }
}
