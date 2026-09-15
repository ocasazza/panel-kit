use crate::{Region, SurfaceProfile, TileMetrics};

/// Whether a projected frame has enough finite space to draw normal content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameStatus {
    /// The viewport is finite and large enough for normal panel projection.
    Ready,
    /// The viewport is invalid or smaller than the panel minimums.
    TooSmall,
}

/// Where one panel is placed in the projected frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    /// Free placement using the panel's clamped floating rectangle.
    Floating,
    /// CSS-grid-style tile placement shared by web and TUI backends.
    Tiled {
        /// Zero-based tile column.
        column: u8,
        /// Zero-based tile row.
        row: u16,
        /// Number of tile columns occupied.
        column_span: u8,
        /// Number of tile rows occupied.
        row_span: u8,
    },
    /// Full workspace placement for the frontmost maximized panel.
    Maximized,
}

/// Semantic chrome and hit regions for a projected panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelChromeProjection {
    /// The full panel rectangle.
    pub outer: Region,
    /// The content/body rectangle after header chrome.
    pub body: Region,
    /// Header band used for move/reorder gestures.
    pub header_hit: Region,
    /// Mode-toggle light hit region, when offered.
    pub mode_hit: Option<Region>,
    /// Minimize light hit region, when offered.
    pub minimize_hit: Option<Region>,
    /// Maximize/restore light hit region, when offered.
    pub maximize_hit: Option<Region>,
    /// Resize grip hit region, when offered.
    pub resize_hit: Option<Region>,
}

/// Tiling metrics used by projection and future backend painters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileLayoutMetrics {
    /// Existing span-resize metrics shared with pointer reducers.
    pub resize: TileMetrics,
    /// Number of columns available on the current surface.
    pub columns: u8,
    /// Minimum height of one tile row.
    pub row_min: f64,
    /// Gap between projected tile tracks.
    pub gap: f64,
    /// Padding around the tile grid.
    pub padding: f64,
    /// Whether tracks expand to fill the workspace band.
    pub fill_viewport: bool,
}

impl TileLayoutMetrics {
    /// Build layout metrics from the existing resize metrics and surface tier.
    pub fn from_tile_metrics(resize: TileMetrics, surface: SurfaceProfile) -> Self {
        Self {
            resize,
            columns: surface.tile_columns(),
            row_min: resize.row,
            gap: 0.0,
            padding: 0.0,
            fill_viewport: false,
        }
    }
}

/// Shared tile grid values consumed by every backend painter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileGridProjection {
    /// Number of columns in the projected grid.
    pub columns: u8,
    /// Number of rows needed by the projected tiles.
    pub rows: u16,
    /// Width of one column track.
    pub track_w: f64,
    /// Height of one row track.
    pub track_h: f64,
    /// Gap between tracks.
    pub gap: f64,
    /// Padding around the grid.
    pub padding: f64,
}
