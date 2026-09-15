use crate::reducer::Viewport;
use crate::{
    effective_mode, effective_rect, PanelKey, PanelWin, Region, TILE_H_MAX, TILE_W_MAX, WinState,
};

use super::chrome::{panel_chrome, PanelChromeMetrics};
use super::scratch::TilePlacement;
use super::tiling::tile_region;
use super::{DockProjection, PanelProjection, PanelProjectionInput, Placement};

/// Project shared workspace chrome from a viewport and chrome configuration.
///
/// When `chrome.dock` is disabled the workspace occupies the full root region
/// and the dock region is empty, so standalone surfaces get no implicit dock.
pub fn project_chrome(viewport: Viewport, chrome: &super::ChromeProjectionInput) -> crate::WorkspaceChrome {
    let mut metrics = chrome.metrics;
    if !chrome.dock {
        metrics.dock_h = 0.0;
    }

    crate::workspace_chrome(viewport.width, viewport.height, &metrics)
}

/// Project a single panel from explicit geometry inputs.
///
/// This API deliberately accepts only one [`PanelWin`] plus renderer-neutral
/// geometry/configuration values. It does not need a snapshot, catalog, store,
/// or dock state, so hosts can render a lone panel surface anywhere.
pub fn project_panel<K: PanelKey>(
    panel: &PanelWin<K>,
    source_index: usize,
    input: PanelProjectionInput<'_>,
) -> Option<PanelProjection<K>> {
    if panel.state == WinState::Minimized || !viewport_is_projectable(input.viewport) {
        return None;
    }

    let mode = effective_mode(input.preferred_mode, &input.surface);
    let (region, placement) = project_panel_region(*panel, source_index, mode, input)?;
    if region.w <= 0.0 || region.h <= 0.0 {
        return None;
    }

    Some(PanelProjection {
        source_index,
        key: panel.kind,
        region,
        placement,
        z: panel.z,
        state: panel.state,
        focused: input.focused,
        pointer_dragging: input.pointer_dragging,
        tile_dragging: input.tile_dragging,
        chrome: panel_chrome(
            region,
            panel.state,
            input.surface,
            input.chrome,
            PanelChromeMetrics::for_input(input.chrome, input.surface),
        ),
    })
}

/// Project minimized panels into caller-owned dock storage.
///
/// The output vector is cleared but keeps its capacity. Passing an empty dock
/// region (as produced by surface-only chrome) disables every dock entry.
pub fn project_dock_into<K: PanelKey>(
    panels: &[PanelWin<K>],
    dock_region: Region,
    out: &mut Vec<DockProjection<K>>,
) {
    out.clear();
    if dock_region.w <= 0.0 || dock_region.h <= 0.0 {
        return;
    }

    let chip_w = dock_chip_width(dock_region);
    let gap = dock_chip_gap(dock_region);
    let mut x = dock_region.x;
    for (source_index, panel) in panels.iter().enumerate() {
        if panel.state != WinState::Minimized {
            continue;
        }

        let remaining = (dock_region.x + dock_region.w - x).max(0.0);
        if remaining <= 0.0 {
            break;
        }

        let w = chip_w.min(remaining).max(0.0);
        out.push(DockProjection {
            source_index,
            key: panel.kind,
            region: Region::new(x, dock_region.y, w, dock_region.h.max(0.0)),
        });
        x += w + gap;
    }
}

fn project_panel_region<K: PanelKey>(
    panel: PanelWin<K>,
    source_index: usize,
    mode: crate::Mode,
    input: PanelProjectionInput<'_>,
) -> Option<(Region, Placement)> {
    if panel.state == WinState::Maximized {
        return Some((input.viewport, Placement::Maximized));
    }

    if mode == crate::Mode::Tiling {
        return Some(project_tiled_panel(panel, source_index, input));
    }

    let (x, y, w, h) = effective_rect(&panel, input.viewport.w, input.viewport.h, input.clamp);
    Some((
        Region::new(
            input.viewport.x + x,
            input.viewport.y + y - input.workspace_scroll,
            w.min(input.viewport.w),
            h.min(input.viewport.h),
        ),
        Placement::Floating,
    ))
}

fn project_tiled_panel<K: PanelKey>(
    panel: PanelWin<K>,
    source_index: usize,
    input: PanelProjectionInput<'_>,
) -> (Region, Placement) {
    let (column, row) = input.tiled_origin.unwrap_or((0, 0));
    let placement = TilePlacement {
        source_index,
        column,
        row,
        column_span: panel.tile_w.clamp(1, TILE_W_MAX),
        row_span: panel.tile_h.clamp(1, TILE_H_MAX),
    };

    if let Some(grid) = input.tile_grid.copied() {
        return tile_region(input.viewport, grid, placement, input.workspace_scroll);
    }

    (
        input.viewport,
        Placement::Tiled {
            column,
            row,
            column_span: placement.column_span,
            row_span: placement.row_span,
        },
    )
}

fn viewport_is_projectable(viewport: Region) -> bool {
    viewport.x.is_finite()
        && viewport.y.is_finite()
        && viewport.w.is_finite()
        && viewport.h.is_finite()
        && viewport.w > 0.0
        && viewport.h > 0.0
}

fn dock_chip_width(dock_region: Region) -> f64 {
    if dock_region.h <= 3.0 {
        dock_region.h.max(1.0)
    } else {
        24.0_f64.min(dock_region.w)
    }
}

fn dock_chip_gap(dock_region: Region) -> f64 {
    if dock_region.h <= 3.0 {
        1.0
    } else {
        6.0
    }
}
