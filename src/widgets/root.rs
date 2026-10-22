//! Root-level web composition helpers for projected workspaces.

use panel_kit_core::frame::{ProjectedFrame, TileGridProjection};
use panel_kit_core::{PanelKey, SurfaceClass};

/// Class for a workspace root element rendered from a borrowed projected frame.
///
/// The class contract is one surface tier, optional coarse-pointer target
/// sizing, and `dragging` while a projected panel has an active pointer or tile
/// drag marker.
pub fn root_class<K: PanelKey>(frame: &ProjectedFrame<'_, K>) -> &'static str {
    let dragging = frame
        .panels
        .iter()
        .any(|panel| panel.pointer_dragging || panel.tile_dragging);

    match (
        frame.surface.class,
        frame.surface.caps.coarse_pointer,
        dragging,
    ) {
        (SurfaceClass::Compact, false, false) => "ws-root compact",
        (SurfaceClass::Compact, false, true) => "ws-root compact dragging",
        (SurfaceClass::Compact, true, false) => "ws-root compact coarse",
        (SurfaceClass::Compact, true, true) => "ws-root compact coarse dragging",
        (SurfaceClass::Tablet, false, false) => "ws-root tablet",
        (SurfaceClass::Tablet, false, true) => "ws-root tablet dragging",
        (SurfaceClass::Tablet, true, false) => "ws-root tablet coarse",
        (SurfaceClass::Tablet, true, true) => "ws-root tablet coarse dragging",
        (SurfaceClass::Regular, false, false) => "ws-root regular",
        (SurfaceClass::Regular, false, true) => "ws-root regular dragging",
        (SurfaceClass::Regular, true, false) => "ws-root regular coarse",
        (SurfaceClass::Regular, true, true) => "ws-root regular coarse dragging",
    }
}

/// Inline CSS Grid tracks projected by core from one [`TileGridProjection`].
///
/// This conversion deliberately accepts no panel list, surface profile, or CSS
/// policy table. Core owns the grid math; the web backend only serializes the
/// explicit tracks and gaps it receives.
pub fn tile_grid_style(grid: TileGridProjection) -> String {
    // Rows are emitted explicitly, one track per row. `repeat(N, h)` forced
    // every row to the same height in the DOM, so resizing one tile resized
    // its neighbours even once core stopped forcing a shared number. Columns
    // stay uniform — nothing varies them per tile.
    let mut rows = String::new();
    for row in 0..grid.rows {
        if row > 0 {
            rows.push(' ');
        }
        rows.push_str(&css_px(grid.row_heights.get(row)));
    }
    format!(
        "grid-template-columns:repeat({}, {});grid-template-rows:{};gap:{};padding:{};",
        grid.columns,
        css_px(grid.track_w),
        rows,
        css_px(grid.gap),
        css_px(grid.padding),
    )
}

pub(crate) fn css_px(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{}px", value as i64)
    } else {
        format!("{value}px")
    }
}
