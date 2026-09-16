//! Terminal spinner — the twin of the web shell's `Spinner` component: a
//! small animated ring with an optional label.

pub use panel_kit_core::widgets::spinner::{spinner_frame, FRAMES};

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::ResolvedTuiTheme;

/// One spinner line: the ring (accent) plus an optional label (dim).
/// Call with a frame/tick counter from your draw loop; empty label renders
/// the ring alone, matching the web component.
pub fn spinner(tick: u64, label: &str, t: &ResolvedTuiTheme) -> Line<'static> {
    let ring = Span::styled(spinner_frame(tick), Style::default().fg(t.accent));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_uses_core_frame_selection() {
        assert_eq!(
            spinner_frame(0),
            panel_kit_core::widgets::spinner::spinner_frame(0)
        );
        assert_eq!(FRAMES, panel_kit_core::widgets::spinner::FRAMES);
    }
}
