use crate::reducer::Snapshot;
use crate::{effective_mode, front_z, Clamp, Mode, PanelKey, Region, WinState};

use super::chrome::{panel_chrome, PanelChromeMetrics};
use super::scratch::ProjectionBuffer;
use super::tiling::{content_extent, panel_region, project_tiles, projected_scroll, PanelRegionContext};
use super::{FrameStatus, PanelProjection, ProjectedFrame, ProjectionInput};

///
/// The returned frame borrows `scratch`, so a host must drop the frame before
/// projecting into the same buffer again. Rust rejects retaining both frames
/// because that would require a second mutable borrow while the first borrowed
/// projection is still alive:
///
/// ```compile_fail,E0499
/// use panel_kit_core::frame::{
///     project_into, ChromeProjectionInput, ProjectionBuffer, ProjectionInput,
///     TileLayoutMetrics,
/// };
/// use panel_kit_core::reducer::{Snapshot, Viewport};
/// use panel_kit_core::{
///     ChromeMetrics, Clamp, LayoutBuilder, Mode, PanelKey, SurfaceCapabilities,
///     SurfaceProfile, TileMetrics, Units,
/// };
///
/// #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// struct Panel;
///
/// impl PanelKey for Panel {}
///
/// let mut layout = LayoutBuilder::new();
/// let snapshot = Snapshot {
///     panels: vec![layout.at(Panel, 0.0, 0.0, 220.0, 140.0)],
///     preferred_mode: Mode::Floating,
///     viewport: Viewport { width: 800.0, height: 600.0, units: Units::CssPx },
///     focused: Some(Panel),
///     drag: None,
///     tile_drag: None,
///     workspace_scroll: 0.0,
/// };
/// let surface = SurfaceProfile::from_logical_width(
///     800.0,
///     panel_kit_core::WEB_COMPACT_MAX,
///     panel_kit_core::WEB_TABLET_MAX,
///     SurfaceCapabilities { coarse_pointer: false, hover: true, keyboard: true },
/// );
/// let tile = TileLayoutMetrics::from_tile_metrics(TileMetrics::WEB, surface);
/// let chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
/// let input = ProjectionInput {
///     snapshot: &snapshot,
///     surface,
///     chrome: &chrome,
///     clamp: &Clamp::WEB,
///     tile: &tile,
/// };
/// let mut buffer = ProjectionBuffer::with_panel_capacity(snapshot.panels.len());
///
/// let frame = project_into(input, &mut buffer);
/// let next_frame = project_into(input, &mut buffer);
/// drop((frame, next_frame));
/// ```
///
/// Project a full borrowed frame into caller-owned scratch.
pub fn project_into<'frame, K: PanelKey>(
    input: ProjectionInput<'_, K>,
    scratch: &'frame mut ProjectionBuffer<K>,
) -> ProjectedFrame<'frame, K> {
    scratch.clear();

    let snapshot = input.snapshot;
    let chrome = super::project_chrome(snapshot.viewport, input.chrome);
    let mode = effective_mode(snapshot.preferred_mode, &input.surface);
    let status = frame_status(snapshot, input.clamp, chrome.workspace);
    if status == FrameStatus::TooSmall {
        return ProjectedFrame {
            status,
            chrome,
            mode,
            surface: input.surface,
            tile_grid: None,
            content_extent: Region::default(),
            workspace_scroll: 0.0,
            panels: &scratch.panels,
            dock: &scratch.dock,
        };
    }

    if input.chrome.dock {
        super::project_dock_into(&snapshot.panels, chrome.dock, &mut scratch.dock);
    }
    let tile_grid = (mode == Mode::Tiling).then(|| project_tiles(snapshot, chrome.workspace, input.tile, scratch));
    let scroll = projected_scroll(snapshot, mode, chrome.workspace, tile_grid, scratch);
    project_panels(input, chrome.workspace, mode, tile_grid, scroll, scratch);
    let content_extent = content_extent(snapshot, mode, chrome.workspace, tile_grid, scratch.panel_order.as_slice());

    ProjectedFrame {
        status,
        chrome,
        mode,
        surface: input.surface,
        tile_grid,
        content_extent,
        workspace_scroll: scroll,
        panels: &scratch.panels,
        dock: &scratch.dock,
    }
}

fn frame_status<K: PanelKey>(snapshot: &Snapshot<K>, clamp: &Clamp, workspace: Region) -> FrameStatus {
    let finite = snapshot.viewport.width.is_finite()
        && snapshot.viewport.height.is_finite()
        && workspace.x.is_finite()
        && workspace.y.is_finite()
        && workspace.w.is_finite()
        && workspace.h.is_finite();
    if !finite || snapshot.viewport.width <= 0.0 || snapshot.viewport.height <= 0.0 {
        return FrameStatus::TooSmall;
    }
    if workspace.w <= clamp.min_w || workspace.h <= clamp.min_h {
        return FrameStatus::TooSmall;
    }
    FrameStatus::Ready
}

fn project_panels<K: PanelKey>(
    input: ProjectionInput<'_, K>,
    workspace: Region,
    mode: Mode,
    tile_grid: Option<super::TileGridProjection>,
    scroll: f64,
    scratch: &mut ProjectionBuffer<K>,
) {
    fill_panel_order(input.snapshot, mode, &mut scratch.panel_order);
    let frontmost_z = front_z(&input.snapshot.panels) - 1;
    for &source_index in &scratch.panel_order {
        let panel = input.snapshot.panels[source_index];
        let (region, placement) = panel_region(
            panel,
            source_index,
            PanelRegionContext { workspace, mode, tile_grid, scroll, input, tile_rows: &scratch.tile_rows },
        );
        if region.w <= 0.0 || region.h <= 0.0 {
            continue;
        }

        let chrome = panel_chrome(region, panel.state, input.surface, input.chrome, PanelChromeMetrics::for_input(input.chrome, input.surface));
        scratch.panels.push(PanelProjection {
            source_index,
            key: panel.kind,
            region,
            placement,
            z: panel.z.min(frontmost_z),
            state: panel.state,
            focused: input.snapshot.focused == Some(panel.kind),
            pointer_dragging: input.snapshot.drag.map(|drag| drag.idx) == Some(source_index),
            tile_dragging: input.snapshot.tile_drag == Some(panel.kind),
            chrome,
        });
    }
}

fn fill_panel_order<K: PanelKey>(snapshot: &Snapshot<K>, mode: Mode, out: &mut Vec<usize>) {
    if let Some(index) = frontmost_maximized(&snapshot.panels) {
        out.push(index);
        return;
    }

    out.extend(snapshot.panels.iter().enumerate().filter_map(|(index, panel)| {
        (panel.state != WinState::Minimized).then_some(index)
    }));
    if mode == Mode::Floating {
        out.sort_by_key(|&index| (snapshot.panels[index].z, index));
    }
}

fn frontmost_maximized<K: PanelKey>(panels: &[crate::PanelWin<K>]) -> Option<usize> {
    panels
        .iter()
        .enumerate()
        .filter(|(_, panel)| panel.state == WinState::Maximized)
        .max_by_key(|(index, panel)| (panel.z, *index))
        .map(|(index, _)| index)
}
