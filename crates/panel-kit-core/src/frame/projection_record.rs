use crate::reducer::Snapshot;
use crate::{ChromeMetrics, Clamp, Mode, PanelKey, Region, SurfaceProfile, WinState};

use super::{PanelChromeProjection, TileGridProjection, TileLayoutMetrics};

/// One projected panel record, copied into caller-owned scratch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelProjection<K: PanelKey> {
    /// Source index in [`Snapshot::panels`].
    pub source_index: usize,
    /// Panel key copied from the source window record.
    pub key: K,
    /// Full panel rectangle in viewport coordinates.
    pub region: Region,
    /// Placement mode and tile spans.
    pub placement: super::Placement,
    /// Floating z-order copied from the source panel.
    pub z: i32,
    /// Window state copied from the source panel.
    pub state: WinState,
    /// Whether this panel owns window-management focus.
    pub focused: bool,
    /// Whether an active floating drag targets this panel.
    pub pointer_dragging: bool,
    /// Whether an active tile reorder targets this panel.
    pub tile_dragging: bool,
    /// Semantic chrome/hit regions for this panel.
    pub chrome: PanelChromeProjection,
}

/// One projected minimized-panel dock entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockProjection<K: PanelKey> {
    /// Source index in [`Snapshot::panels`].
    pub source_index: usize,
    /// Panel key copied from the source window record.
    pub key: K,
    /// Dock chip hit region.
    pub region: Region,
}

/// Borrowed semantic frame projected into reusable scratch.
pub struct ProjectedFrame<'a, K: PanelKey> {
    /// Overall frame readiness.
    pub status: super::FrameStatus,
    /// Shared workspace chrome regions.
    pub chrome: crate::WorkspaceChrome,
    /// Effective layout mode on this surface.
    pub mode: Mode,
    /// Surface facts projection used.
    pub surface: SurfaceProfile,
    /// Tile grid, present only for ready tiling frames.
    pub tile_grid: Option<TileGridProjection>,
    /// Virtual content extent used to clamp workspace scrolling.
    pub content_extent: Region,
    /// Workspace scroll after projection-time clamping.
    pub workspace_scroll: f64,
    /// Ordered visible panel projections.
    pub panels: &'a [PanelProjection<K>],
    /// Minimized panel dock entries.
    pub dock: &'a [DockProjection<K>],
}

/// Chrome options used by renderer-neutral frame projection.
///
/// This is the frame-level shape that authored `ChromeSpec` values lower onto:
/// metrics size the workspace, booleans decide which optional chrome parts are
/// present, and surface-only mode disables every panel chrome and dock part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChromeProjectionInput {
    /// Workspace and dock metrics in renderer units.
    pub metrics: ChromeMetrics,
    /// Minimum pointer hit target for window-management controls.
    pub hit_target_min: f64,
    /// Panel header/title band height.
    pub panel_header_h: f64,
    /// Whether panel frame chrome is enabled.
    pub panel_frame: bool,
    /// Whether backend painters should place the title in the frame/border.
    pub title_in_border: bool,
    /// Whether mode-toggle controls are projected.
    pub mode_control: bool,
    /// Whether minimize controls are projected.
    pub minimize_control: bool,
    /// Whether maximize/restore controls are projected.
    pub maximize_control: bool,
    /// Whether resize grips are projected.
    pub resize_grip: bool,
    /// Whether workspace dock projection is enabled.
    pub dock: bool,
}

impl ChromeProjectionInput {
    /// Enable the standard panel chrome and dock for the given renderer units.
    pub fn full(metrics: ChromeMetrics) -> Self {
        let cells = metrics == ChromeMetrics::CELLS;
        Self {
            metrics,
            hit_target_min: if cells { 1.0 } else { 24.0 },
            panel_header_h: if cells { 1.0 } else { 22.0 },
            panel_frame: true,
            title_in_border: cells,
            mode_control: true,
            minimize_control: true,
            maximize_control: true,
            resize_grip: true,
            dock: true,
        }
    }

    /// Enable only the panel surface/body; all chrome and dock parts are off.
    pub fn surface_only(metrics: ChromeMetrics) -> Self {
        Self {
            metrics,
            hit_target_min: 0.0,
            panel_header_h: 0.0,
            panel_frame: false,
            title_in_border: false,
            mode_control: false,
            minimize_control: false,
            maximize_control: false,
            resize_grip: false,
            dock: false,
        }
    }
}

/// Inputs required to project one panel without a snapshot or catalog.
#[derive(Clone, Copy)]
pub struct PanelProjectionInput<'a> {
    /// Region the panel is projected within.
    pub viewport: Region,
    /// Host-preferred mode before surface constraints are applied.
    pub preferred_mode: Mode,
    /// Current surface profile.
    pub surface: SurfaceProfile,
    /// Whether this panel owns focus.
    pub focused: bool,
    /// Whether an active floating drag targets this panel.
    pub pointer_dragging: bool,
    /// Whether an active tile drag targets this panel.
    pub tile_dragging: bool,
    /// Workspace scroll offset to subtract from projected panel y coordinates.
    pub workspace_scroll: f64,
    /// Floating viewport clamp metrics.
    pub clamp: &'a Clamp,
    /// Chrome options for panel controls and body/header geometry.
    pub chrome: &'a ChromeProjectionInput,
    /// Tile grid values to use when tiling is effective.
    pub tile_grid: Option<&'a TileGridProjection>,
    /// Tiled origin for independently projected panels; `None` means `(0, 0)`.
    pub tiled_origin: Option<(u8, u16)>,
}

/// Inputs required to project a borrowed frame.
#[derive(Clone, Copy)]
pub struct ProjectionInput<'a, K: PanelKey> {
    /// Host-owned snapshot to read from without retaining.
    pub snapshot: &'a Snapshot<K>,
    /// Current surface profile.
    pub surface: SurfaceProfile,
    /// Workspace chrome options in renderer units.
    pub chrome: &'a ChromeProjectionInput,
    /// Floating viewport clamp metrics.
    pub clamp: &'a Clamp,
    /// Tile grid metrics.
    pub tile: &'a TileLayoutMetrics,
}
