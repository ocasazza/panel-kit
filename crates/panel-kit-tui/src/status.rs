//! Status-dot component — a colored `●` for state readouts in tables and
//! lists. Callers map their own domain state to a renderer-neutral [`Rgb`]
//! value and use these helpers to render it consistently.

use panel_kit_core::badge::Rgb;
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

/// A `● label` line: the dot and the label both in `color`.
pub fn labeled(rgb: Rgb, label: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        dot(rgb),
        Span::raw(" "),
        Span::styled(label.into(), Style::default().fg(color(rgb))),
    ])
}
