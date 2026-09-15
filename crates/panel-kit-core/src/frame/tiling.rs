use crate::reducer::Snapshot;
use crate::{clamp_scroll, effective_rect, floating_content_height, Mode, PanelKey, Region, WinState, TILE_H_MAX, TILE_W_MAX};

use super::scratch::{ProjectionBuffer, TilePlacement};
use super::{Placement, ProjectionInput, TileGridProjection, TileLayoutMetrics};

pub(super) struct PanelRegionContext<'a, 'b, K: PanelKey> {
    pub(super) workspace: Region,
    pub(super) mode: Mode,
    pub(super) tile_grid: Option<TileGridProjection>,
    pub(super) scroll: f64,
    pub(super) input: ProjectionInput<'a, K>,
    pub(super) tile_rows: &'b [TilePlacement],
}

pub(super) fn project_tiles<K: PanelKey>(
    snapshot: &Snapshot<K>,
    workspace: Region,
    metrics: &TileLayoutMetrics,
    scratch: &mut ProjectionBuffer<K>,
) -> TileGridProjection {
    let columns = metrics.columns.clamp(1, TILE_W_MAX);
    let mut used = 0_u8;
    let mut row = 0_u16;
    let mut row_h = 0_u16;

    for (source_index, panel) in snapshot.panels.iter().enumerate() {
        if panel.state != WinState::Floating {
            continue;
        }

        let column_span = panel.tile_w.clamp(1, columns);
        let row_span = panel.tile_h.clamp(1, TILE_H_MAX);
        if used + column_span > columns {
            row = row.saturating_add(row_h.max(1));
            used = 0;
            row_h = 0;
        }

        scratch.tile_rows.push(TilePlacement { source_index, column: used, row, column_span, row_span });
        used += column_span;
        row_h = row_h.max(row_span as u16);
    }

    let rows = row.saturating_add(row_h).max(1);
    let usable_w = (workspace.w - metrics.padding * 2.0 - metrics.gap * (columns.saturating_sub(1) as f64)).max(0.0);
    let usable_h = (workspace.h - metrics.padding * 2.0 - metrics.gap * (rows.saturating_sub(1) as f64)).max(0.0);
    let track_w = (usable_w / columns as f64).max(metrics.resize.col_floor).max(0.0);
    let natural_h = metrics.row_min.max(0.0);
    let track_h = if metrics.fill_viewport { natural_h.max(usable_h / rows as f64) } else { natural_h };

    TileGridProjection { columns, rows, track_w, track_h, gap: metrics.gap.max(0.0), padding: metrics.padding.max(0.0) }
}

pub(super) fn panel_region<K: PanelKey>(
    panel: crate::PanelWin<K>,
    source_index: usize,
    context: PanelRegionContext<'_, '_, K>,
) -> (Region, Placement) {
    if panel.state == WinState::Maximized {
        return (context.workspace, Placement::Maximized);
    }

    if context.mode == Mode::Tiling {
        return context
            .tile_rows
            .iter()
            .find(|placement| placement.source_index == source_index)
            .and_then(|placement| {
                context
                    .tile_grid
                    .map(|grid| tile_region(context.workspace, grid, *placement, context.scroll))
            })
            .unwrap_or((Region::default(), Placement::Tiled { column: 0, row: 0, column_span: 1, row_span: 1 }));
    }

    let (x, y, w, h) = effective_rect(
        &panel,
        context.workspace.w,
        context.workspace.h,
        context.input.clamp,
    );
    (
        Region::new(
            context.workspace.x + x,
            context.workspace.y + y - context.scroll,
            w.min(context.workspace.w),
            h.min(context.workspace.h),
        ),
        Placement::Floating,
    )
}

pub(super) fn projected_scroll<K: PanelKey>(
    snapshot: &Snapshot<K>,
    mode: Mode,
    workspace: Region,
    tile_grid: Option<TileGridProjection>,
    scratch: &ProjectionBuffer<K>,
) -> f64 {
    let content_h = projected_content_height(snapshot, mode, workspace, tile_grid, &scratch.panel_order);
    clamp_scroll(snapshot.workspace_scroll, content_h, workspace.h)
}

pub(super) fn content_extent<K: PanelKey>(
    snapshot: &Snapshot<K>,
    mode: Mode,
    workspace: Region,
    tile_grid: Option<TileGridProjection>,
    order: &[usize],
) -> Region {
    let h = projected_content_height(snapshot, mode, workspace, tile_grid, order);
    Region::new(workspace.x, workspace.y, workspace.w, h.max(0.0))
}

pub(super) fn tile_region(
    workspace: Region,
    grid: TileGridProjection,
    placement: TilePlacement,
    scroll: f64,
) -> (Region, Placement) {
    let x = workspace.x + grid.padding + placement.column as f64 * (grid.track_w + grid.gap);
    let y = workspace.y + grid.padding + placement.row as f64 * (grid.track_h + grid.gap) - scroll;
    let w = placement.column_span as f64 * grid.track_w + placement.column_span.saturating_sub(1) as f64 * grid.gap;
    let h = placement.row_span as f64 * grid.track_h + placement.row_span.saturating_sub(1) as f64 * grid.gap;

    (
        Region::new(x, y, w.max(0.0), h.max(0.0)),
        Placement::Tiled { column: placement.column, row: placement.row, column_span: placement.column_span, row_span: placement.row_span },
    )
}

fn projected_content_height<K: PanelKey>(
    snapshot: &Snapshot<K>,
    mode: Mode,
    workspace: Region,
    tile_grid: Option<TileGridProjection>,
    order: &[usize],
) -> f64 {
    match (mode, tile_grid) {
        (Mode::Floating, _) => floating_content_height(&snapshot.panels, order),
        (Mode::Tiling, Some(grid)) => grid.padding * 2.0 + grid.rows as f64 * grid.track_h + grid.rows.saturating_sub(1) as f64 * grid.gap,
        (Mode::Tiling, None) => workspace.h,
    }
}
