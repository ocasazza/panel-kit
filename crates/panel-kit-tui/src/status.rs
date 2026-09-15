//! Status-dot painter for table cells and summaries.
//!
//! Core owns the semantic [`StatusModel`]. The terminal backend converts that
//! model into borrowed ratatui spans so table painters can consume shared data
//! without cloning labels.

use panel_kit_core::badge::Rgb;
use panel_kit_core::widgets::status::StatusModel;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

fn color((r, g, b): Rgb) -> Color {
    Color::Rgb(r, g, b)
}

/// A colored status dot `●`.
pub fn dot(rgb: Rgb) -> Span<'static> {
    Span::styled("●", Style::default().fg(color(rgb)))
}

/// A `Style` whose foreground is `color` — convenience for styling a table
/// cell to match its [`dot`].
pub fn style(rgb: Rgb) -> Style {
    Style::default().fg(color(rgb))
}

/// A borrowed `● label` line from a shared core [`StatusModel`].
pub fn line(model: &StatusModel) -> Line<'_> {
    labeled_borrowed(model.color, &model.label)
}

/// A borrowed `● label` line for table painters that already hold cell parts.
pub fn labeled_borrowed(rgb: Rgb, label: &str) -> Line<'_> {
    Line::from(vec![
        dot(rgb),
        Span::raw(" "),
        Span::styled(label, Style::default().fg(color(rgb))),
    ])
}

/// An owned `● label` line for standalone status readouts.
pub fn labeled(rgb: Rgb, label: impl Into<String>) -> Line<'static> {
    let label = label.into();
    Line::from(vec![
        dot(rgb),
        Span::raw(" "),
        Span::styled(label, Style::default().fg(color(rgb))),
    ])
}
