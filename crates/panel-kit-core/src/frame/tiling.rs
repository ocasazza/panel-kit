use crate::reducer::Snapshot;
use crate::{
    clamp_scroll, effective_rect, floating_content_height, Mode, PanelKey, Region, WinState,
    TILE_H_MAX, TILE_W_MAX,
};

use super::scratch::{ProjectionBuffer, TilePlacement};
use super::{Placement, ProjectionInput, TileFillOrder, TileGridProjection, TileLayoutMetrics};

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
    let rows = match metrics.fill_order {
        TileFillOrder::RowMajor => project_row_major(snapshot, columns, scratch),
        TileFillOrder::ColumnMajor => project_column_major(snapshot, columns, scratch),
    };
    let usable_w =
        (workspace.w - metrics.padding * 2.0 - metrics.gap * columns.saturating_sub(1) as f64)
            .max(0.0);
    let usable_h =
        (workspace.h - metrics.padding * 2.0 - metrics.gap * rows.saturating_sub(1) as f64)
            .max(0.0);
    let track_w = (usable_w / columns as f64)
        .max(metrics.resize.col_floor)
        .max(0.0);
    let natural_h = metrics.row_min.max(0.0);
    let track_h = if metrics.fill_viewport {
        natural_h.max(usable_h / rows as f64)
    } else {
        natural_h
    };

    TileGridProjection {
        columns,
        rows,
        track_w,
        track_h,
        gap: metrics.gap.max(0.0),
        padding: metrics.padding.max(0.0),
    }
}

fn project_row_major<K: PanelKey>(
    snapshot: &Snapshot<K>,
    columns: u8,
    scratch: &mut ProjectionBuffer<K>,
) -> u16 {
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

        scratch.tile_rows.push(TilePlacement {
            source_index,
            column: used,
            row,
            column_span,
            row_span,
        });
        used += column_span;
        row_h = row_h.max(row_span as u16);
    }

    row.saturating_add(row_h).max(1)
}

fn project_column_major<K: PanelKey>(
    snapshot: &Snapshot<K>,
    columns: u8,
    scratch: &mut ProjectionBuffer<K>,
) -> u16 {
    let mut rows = row_major_rows(snapshot, columns);
    while column_major_columns(snapshot, rows, columns) > columns as u16 && rows < u16::MAX {
        rows += 1;
    }

    let mut column = 0_u16;
    let mut used = 0_u16;
    let mut column_w = 0_u8;
    for (source_index, panel) in snapshot.panels.iter().enumerate() {
        if panel.state != WinState::Floating {
            continue;
        }

        let column_span = panel.tile_w.clamp(1, columns);
        let row_span = panel.tile_h.clamp(1, TILE_H_MAX);
        if used > 0 && used.saturating_add(row_span as u16) > rows {
            column = column.saturating_add(column_w.max(1) as u16);
            used = 0;
            column_w = 0;
        }

        scratch.tile_rows.push(TilePlacement {
            source_index,
            column: column.min(u8::MAX as u16) as u8,
            row: used,
            column_span,
            row_span,
        });
        used = used.saturating_add(row_span as u16);
        column_w = column_w.max(column_span);
    }
    rows
}

fn row_major_rows<K: PanelKey>(snapshot: &Snapshot<K>, columns: u8) -> u16 {
    let mut used = 0_u8;
    let mut row = 0_u16;
    let mut row_h = 0_u16;

    for panel in &snapshot.panels {
        if panel.state != WinState::Floating {
            continue;
        }
        let column_span = panel.tile_w.clamp(1, columns);
        let row_span = panel.tile_h.clamp(1, TILE_H_MAX) as u16;
        if used + column_span > columns {
            row = row.saturating_add(row_h.max(1));
            used = 0;
            row_h = 0;
        }
        used += column_span;
        row_h = row_h.max(row_span);
    }

    row.saturating_add(row_h).max(1)
}

fn column_major_columns<K: PanelKey>(snapshot: &Snapshot<K>, rows: u16, columns: u8) -> u16 {
    let mut column = 0_u16;
    let mut used = 0_u16;
    let mut column_w = 0_u8;

    for panel in &snapshot.panels {
        if panel.state != WinState::Floating {
            continue;
        }
        let column_span = panel.tile_w.clamp(1, columns);
        let row_span = panel.tile_h.clamp(1, TILE_H_MAX) as u16;
        if used > 0 && used.saturating_add(row_span) > rows {
            column = column.saturating_add(column_w.max(1) as u16);
            used = 0;
            column_w = 0;
        }
        used = used.saturating_add(row_span);
        column_w = column_w.max(column_span);
    }

    column.saturating_add(column_w as u16)
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
            .unwrap_or((
                Region::default(),
                Placement::Tiled {
                    column: 0,
                    row: 0,
                    column_span: 1,
                    row_span: 1,
                },
            ));
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
    let content_h =
        projected_content_height(snapshot, mode, workspace, tile_grid, &scratch.panel_order);
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
    let w = placement.column_span as f64 * grid.track_w
        + placement.column_span.saturating_sub(1) as f64 * grid.gap;
    let h = placement.row_span as f64 * grid.track_h
        + placement.row_span.saturating_sub(1) as f64 * grid.gap;

    (
        Region::new(x, y, w.max(0.0), h.max(0.0)),
        Placement::Tiled {
            column: placement.column,
            row: placement.row,
            column_span: placement.column_span,
            row_span: placement.row_span,
        },
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
        (Mode::Tiling, Some(grid)) => {
            grid.padding * 2.0
                + grid.rows as f64 * grid.track_h
                + grid.rows.saturating_sub(1) as f64 * grid.gap
        }
        (Mode::Tiling, None) => workspace.h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reducer::Viewport;
    use crate::{
        LayoutBuilder, PanelKind, SurfaceCapabilities, SurfaceProfile, TileMetrics, Units,
        CELLS_COMPACT_MAX, CELLS_TABLET_MAX,
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    enum TestPanel {
        One,
        Two,
        Three,
        Four,
    }

    impl PanelKind for TestPanel {
        fn title(self) -> &'static str {
            match self {
                Self::One => "One",
                Self::Two => "Two",
                Self::Three => "Three",
                Self::Four => "Four",
            }
        }
    }

    #[test]
    fn column_major_fill_places_panels_down_columns_first() {
        let mut layout = LayoutBuilder::new();
        let panels = vec![
            layout
                .at(TestPanel::One, 0.0, 0.0, 20.0, 10.0)
                .with_tile(1, 1),
            layout
                .at(TestPanel::Two, 0.0, 0.0, 20.0, 10.0)
                .with_tile(1, 1),
            layout
                .at(TestPanel::Three, 0.0, 0.0, 20.0, 10.0)
                .with_tile(1, 1),
            layout
                .at(TestPanel::Four, 0.0, 0.0, 20.0, 10.0)
                .with_tile(1, 1),
        ];
        let snapshot = Snapshot::from_defaults(
            panels,
            Mode::Tiling,
            Viewport {
                width: 80.0,
                height: 24.0,
                units: Units::Cells,
            },
        );
        let surface = SurfaceProfile::from_logical_width(
            80.0,
            CELLS_COMPACT_MAX,
            CELLS_TABLET_MAX,
            SurfaceCapabilities {
                coarse_pointer: false,
                hover: true,
                keyboard: true,
            },
        );
        let mut metrics = TileLayoutMetrics::from_tile_metrics(TileMetrics::CELLS, surface);
        assert_eq!(metrics.fill_order, TileFillOrder::RowMajor);
        metrics.columns = 2;
        metrics.row_min = 1.0;
        metrics.fill_order = TileFillOrder::ColumnMajor;
        let mut scratch = ProjectionBuffer::with_panel_capacity(snapshot.panels.len());
        let workspace = Region::new(0.0, 0.0, 80.0, 24.0);
        let grid = project_tiles(&snapshot, workspace, &metrics, &mut scratch);
        let placements = scratch
            .tile_rows
            .iter()
            .map(|placement| tile_region(workspace, grid, *placement, 0.0).1)
            .collect::<Vec<_>>();

        assert_eq!(grid.rows, 2);
        assert_eq!(
            placements,
            vec![
                Placement::Tiled {
                    column: 0,
                    row: 0,
                    column_span: 1,
                    row_span: 1,
                },
                Placement::Tiled {
                    column: 0,
                    row: 1,
                    column_span: 1,
                    row_span: 1,
                },
                Placement::Tiled {
                    column: 1,
                    row: 0,
                    column_span: 1,
                    row_span: 1,
                },
                Placement::Tiled {
                    column: 1,
                    row: 1,
                    column_span: 1,
                    row_span: 1,
                },
            ]
        );
    }

    #[test]
    fn default_tile_metrics_expand_tracks_to_fill_the_workspace_band() {
        // The pre-1.0 tiling renderer sized rows with `1fr`: tiles always
        // expanded to fill the screen. Hosts build metrics through
        // `TileLayoutMetrics::from_tile_metrics`, so that constructor's
        // default must preserve the expanding behavior — fixed `row_min`
        // tracks leave panels huddled at the top of the viewport.
        let mut layout = LayoutBuilder::new();
        let panels = vec![
            layout
                .at(TestPanel::One, 0.0, 0.0, 20.0, 10.0)
                .with_tile(4, 1),
            layout
                .at(TestPanel::Two, 0.0, 0.0, 20.0, 10.0)
                .with_tile(4, 1),
        ];
        let snapshot = Snapshot::from_defaults(
            panels,
            Mode::Tiling,
            Viewport {
                width: 1000.0,
                height: 600.0,
                units: Units::CssPx,
            },
        );
        let surface = SurfaceProfile::from_logical_width(
            1000.0,
            crate::WEB_COMPACT_MAX,
            crate::WEB_TABLET_MAX,
            SurfaceCapabilities {
                coarse_pointer: false,
                hover: true,
                keyboard: true,
            },
        );
        let metrics = TileLayoutMetrics::from_tile_metrics(TileMetrics::WEB, surface);
        let workspace = Region::new(0.0, 0.0, 1000.0, 600.0);
        let mut scratch = ProjectionBuffer::with_panel_capacity(snapshot.panels.len());
        let grid = project_tiles(&snapshot, workspace, &metrics, &mut scratch);
        assert_eq!(grid.rows, 2);

        let heights = scratch
            .tile_rows
            .iter()
            .map(|placement| tile_region(workspace, grid, *placement, 0.0).0.h)
            .collect::<Vec<_>>();
        assert_eq!(
            heights,
            vec![300.0, 300.0],
            "default metrics must stretch tile tracks to fill the workspace band"
        );
    }
}
