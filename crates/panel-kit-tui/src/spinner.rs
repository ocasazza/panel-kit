//! Terminal spinner — the twin of the web shell's `Spinner` component: a
//! small animated ring with an optional label.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::Theme;

/// Braille ring frames; index with any monotonically increasing tick.
pub const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// One spinner line: the ring (accent) plus an optional label (dim).
/// Call with a frame/tick counter from your draw loop; empty label renders
/// the ring alone, matching the web component.
pub fn spinner(tick: u64, label: &str, t: &Theme) -> Line<'static> {
    let ring = Span::styled(
        FRAMES[(tick as usize) % FRAMES.len()],
        Style::default().fg(t.accent),
    );
    if label.is_empty() {
        Line::from(ring)
    } else {
        Line::from(vec![
            ring,
            Span::raw(" "),
            Span::styled(label.to_string(), Style::default().fg(t.dim)),
        ])
    }
}
