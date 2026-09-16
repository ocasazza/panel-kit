//! Ratatui backend for panel-kit's composable workspace parts.
//!
//! The same renderer-neutral state machine that the Dioxus crate paints to the
//! DOM is drawn here in terminal cells. Hosts own snapshots, input priority,
//! persistence timing, projection scratch, and render order; this crate exposes
//! terminal input adapters, JSON-file storage, and independently callable
//! ratatui painters for root chrome, panel surfaces, panel chrome, controls,
//! resize grips, docks, and scrollbars.
//!
//! ```no_run
//! use panel_kit_core::frame::{project_into, ChromeProjectionInput, ProjectionBuffer, ProjectionInput, TileLayoutMetrics};
//! use panel_kit_core::reducer::{Snapshot, Viewport};
//! use panel_kit_core::{ChromeMetrics, Clamp, LayoutBuilder, Mode, PanelKey, SurfaceCapabilities, SurfaceProfile, TileMetrics, Units};
//! use panel_kit_tui::widgets::TuiHitBuffer;
//!
//! #[derive(Clone, Copy, PartialEq, Eq, Hash)]
//! enum Panel { Logs, Stats }
//! impl PanelKey for Panel {}
//!
//! let mut layout = LayoutBuilder::new();
//! let snapshot = Snapshot {
//!     panels: vec![layout.at(Panel::Logs, 2.0, 1.0, 50.0, 14.0), layout.at(Panel::Stats, 54.0, 1.0, 36.0, 10.0)],
//!     preferred_mode: Mode::Floating,
//!     viewport: Viewport { width: 100.0, height: 30.0, units: Units::Cells },
//!     focused: Some(Panel::Logs),
//!     drag: None,
//!     tile_drag: None,
//!     workspace_scroll: 0.0,
//! };
//! let surface = SurfaceProfile::from_logical_width(100.0, panel_kit_core::CELLS_COMPACT_MAX, panel_kit_core::CELLS_TABLET_MAX, SurfaceCapabilities {
//!     coarse_pointer: false,
//!     hover: true,
//!     keyboard: true,
//! });
//! let chrome = ChromeProjectionInput::full(ChromeMetrics::CELLS);
//! let tile = TileLayoutMetrics::from_tile_metrics(TileMetrics::CELLS, surface);
//! let mut projection = ProjectionBuffer::with_panel_capacity(snapshot.panels.len());
//! let mut hits: TuiHitBuffer<Panel> = TuiHitBuffer::with_capacity(snapshot.panels.len(), snapshot.panels.len());
//! let frame = project_into(ProjectionInput { snapshot: &snapshot, surface, chrome: &chrome, clamp: &Clamp::CELLS, tile: &tile }, &mut projection);
//! hits.clear();
//! assert_eq!(frame.panels.len(), 2);
//! ```

#![warn(missing_docs)]

pub mod badge;
pub mod charts;
pub mod input;
pub mod meter;
pub mod scroll;
#[cfg(feature = "spec-plan")]
pub mod spec_plan;
pub mod spinner;
pub mod status;
pub mod store;
pub mod table;
pub mod theme;
pub mod widgets;

use panel_kit_core::Region;
pub use theme::{ResolvedTuiTheme, TuiThemeDisposition};

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols::border;
use ratatui::Frame;

/// WebGL-safe chrome glyphs. Ratzilla's WebGL font atlas does not reliably
/// include box-drawing symbols, so the shared TUI chrome sticks to ASCII.
const ASCII_BORDER: border::Set<'static> = border::Set {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};

/// Which glyph set the chrome draws with. A native terminal renders
/// box-drawing and geometric glyphs (rounded borders, ● / ◉ lights); the
/// ratzilla WebGL backend drops non-atlas glyphs, so it opts into
/// [`Charset::Ascii`]. Defaults to [`Charset::Unicode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Charset {
    /// Rounded borders and ● / ◉ traffic lights (native terminal).
    #[default]
    Unicode,
    /// `+ - |` borders and `o` / `O` lights (WebGL-safe).
    Ascii,
}

impl Charset {
    /// The panel border glyph set for this charset.
    fn border(self) -> border::Set<'static> {
        match self {
            Charset::Unicode => border::ROUNDED,
            Charset::Ascii => ASCII_BORDER,
        }
    }

    /// The traffic-light glyph; `hovered` rings the light.
    fn light(self, hovered: bool) -> char {
        match (self, hovered) {
            (Charset::Unicode, true) => '◉',
            (Charset::Unicode, false) => '●',
            (Charset::Ascii, true) => 'O',
            (Charset::Ascii, false) => 'o',
        }
    }
}

fn rect_from_region(r: Region) -> Rect {
    Rect::new(r.x as u16, r.y as u16, r.w as u16, r.h as u16)
}

fn write_header_text(f: &mut Frame, x: u16, y: u16, max_width: u16, text: &str, style: Style) {
    let area = f.area();
    if y >= area.bottom() || x >= area.right() {
        return;
    }
    let max_width = max_width.min(area.right().saturating_sub(x));
    for (offset, ch) in text.chars().take(max_width as usize).enumerate() {
        f.buffer_mut()[(x + offset as u16, y)]
            .set_char(ch)
            .set_style(style);
    }
}
