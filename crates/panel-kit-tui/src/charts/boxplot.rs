use panel_kit_core::widgets::charts::BoxItemView;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::ResolvedTuiTheme;

use super::boxplot_render::draw_item;
use super::{rgb_color, series_colors};

#[derive(Clone, Copy)]
pub(super) struct BoxScale {
    lo: f64,
    span: f64,
    plot_h: u16,
    pub(super) plot_bottom: u16,
}

#[derive(Clone, Copy)]
pub(super) struct BoxGeometry {
    pub(super) slot_x: u16,
    pub(super) slot_w: u16,
    pub(super) center_x: u16,
    pub(super) box_x: u16,
    pub(super) box_w: u16,
}

impl BoxScale {
    pub(super) fn row_of(self, val: f64) -> u16 {
        let frac = ((val - self.lo) / self.span).clamp(0.0, 1.0);
        let from_bottom = (frac * self.plot_h.saturating_sub(1) as f64).round() as u16;
        self.plot_bottom
            .saturating_sub(1)
            .saturating_sub(from_bottom)
    }
}

/// Render side-by-side vertical box-and-whisker plots into `area`.
///
/// One box per [`BoxItemView`], laid out across the width. The shared y-axis
/// auto-scales across all boxes with a small headroom. Colors come from the
/// categorical palette unless overridden.
pub fn boxplot(f: &mut Frame, area: Rect, t: &ResolvedTuiTheme, items: &[BoxItemView<'_>]) {
    let Some(scale) = box_scale(area, items) else {
        return;
    };

    let colors = series_colors(t);
    for (index, item) in items.iter().enumerate() {
        let color = item
            .color
            .map(rgb_color)
            .unwrap_or(colors[index % colors.len()]);
        draw_item(
            f,
            area,
            scale,
            box_geometry(area, index, items.len() as u16),
            item,
            color,
            t,
        );
    }
}

fn box_scale(area: Rect, items: &[BoxItemView<'_>]) -> Option<BoxScale> {
    if area.width == 0 || area.height < 2 || items.is_empty() {
        return None;
    }

    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for item in items {
        lo = lo.min(item.summary.min);
        hi = hi.max(item.summary.max);
    }
    if lo > hi {
        (lo, hi) = (0.0, 1.0);
    }
    if (hi - lo).abs() < f64::EPSILON {
        hi = lo + 1.0;
    }

    let plot_h = area.height.saturating_sub(1).max(1);
    Some(BoxScale {
        lo,
        span: hi - lo,
        plot_h,
        plot_bottom: area.y + plot_h,
    })
}

fn box_geometry(area: Rect, index: usize, count: u16) -> BoxGeometry {
    let slot_w = (area.width / count).max(1);
    let slot_x = area.x + index as u16 * slot_w;
    let center_x = slot_x + slot_w / 2;
    let box_w = slot_w.saturating_sub(2).clamp(1, 7);
    let box_x = center_x.saturating_sub(box_w / 2);

    BoxGeometry {
        slot_x,
        slot_w,
        center_x,
        box_x,
        box_w,
    }
}
