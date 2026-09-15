//! Root shell and workspace-level scrollbar parts.

use panel_kit_core::frame::ProjectedFrame;
use panel_kit_core::{max_scroll, PanelKey};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::widgets::draw_border;
use crate::{rect_from_region, Charset, ResolvedTuiTheme};

/// Draw the workspace root border and return its native inner rect.
pub fn draw_root(
    frame: &mut Frame,
    area: Rect,
    theme: &ResolvedTuiTheme,
    charset: Charset,
) -> Rect {
    draw_border(frame, area, charset, Style::default().fg(theme.line2));
    Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    )
}

/// Draw the workspace-level vertical scrollbar for an overhanging frame.
pub fn draw_workspace_scrollbar<K: PanelKey>(
    frame: &mut Frame,
    projected: &ProjectedFrame<'_, K>,
    theme: &ResolvedTuiTheme,
) {
    let workspace = rect_from_region(projected.chrome.workspace);
    let max = max_scroll(projected.content_extent.h, projected.chrome.workspace.h);
    if max <= 0.0 || workspace.height == 0 {
        return;
    }

    let mut state = ScrollbarState::new(max as usize).position(projected.workspace_scroll as usize);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(Style::default().fg(theme.line2))
            .thumb_style(Style::default().fg(theme.dim)),
        workspace,
        &mut state,
    );
}

