//! Renderer-agnostic core of panel-kit: the panel-workspace state machine.
//!
//! Everything here is pure data and math — no DOM, no terminal, no async.
//! A renderer shell (the Dioxus `panel-kit` crate, the ratatui
//! `panel-kit-tui` crate, or anything else that can draw rectangles and
//! deliver pointer coordinates) owns a `Vec<PanelWin<K>>` plus a `Mode` and
//! an `Option<Drag>`, and calls into these functions:
//!
//! - [`begin_drag`] / [`begin_tile_resize`] on pointer-down,
//! - [`apply_drag`] on pointer-move,
//! - [`reorder_tile`], [`front_z`], [`restore`] for tiling reorder and
//!   z-order management,
//! - [`effective_rect`] to project stored geometry through the viewport
//!   clamp at render time,
//! - [`SavedLayout`] + [`merge_defaults`] for persistence (the shell
//!   supplies the actual storage: localStorage, a JSON file, a KV bucket).
//!
//! Units are deliberately abstract: the web shell feeds CSS pixels, the TUI
//! shell feeds character cells. All unit-dependent constants live in
//! [`Clamp`] and [`TileMetrics`]; [`Clamp::WEB`]/[`TileMetrics::WEB`]
//! preserve the original panel-kit pixel behavior exactly.

#![warn(missing_docs)]

pub mod badge;

use serde::{Deserialize, Serialize};

/// The app's panel identifier — typically a fieldless enum.
///
/// One variant per panel plus a [`title`](PanelKind::title). The serde
/// bounds exist because layouts (including each panel's kind) persist; the
/// `Copy + Eq + Hash` bounds let workspaces use kinds as cheap, stable panel
/// identities across reorders and reloads.
pub trait PanelKind:
    Copy + PartialEq + Eq + std::hash::Hash + Serialize + serde::de::DeserializeOwned + 'static
{
    /// Human-readable panel title, shown in the panel header (and slugified
    /// into CSS classes by the web shell), so it should be stable.
    fn title(self) -> &'static str;
}

/// Per-panel window state, cycled by the traffic lights.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum WinState {
    /// Normal: shown at its stored geometry (floating mode) or in its grid
    /// slot (tiling mode).
    Floating,
    /// Collapsed into a dock chip; restoring brings it back.
    Minimized,
    /// Fills the whole workspace area, hiding every other panel.
    Maximized,
}

/// Workspace layout mode.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    /// Free placement: panels are absolutely positioned, draggable,
    /// resizable, and overlap by z-order.
    Floating,
    /// Auto grid: panels flow in `Vec` order; dragging a panel header over
    /// another panel reorders them.
    Tiling,
}

/// What an in-flight floating-mode drag is doing.
#[derive(Clone, Copy, PartialEq)]
pub enum DragKind {
    /// Dragging the panel header: the panel follows the pointer.
    Move,
    /// Dragging the resize handle: the panel grows/shrinks.
    Resize,
}

/// An in-flight drag: which panel, what kind, and the pointer + panel
/// geometry captured at pointer-down (deltas are applied against these).
///
/// For a tiling resize started with [`begin_tile_resize`], `start_w`/
/// `start_h` hold the *tile spans*, not length units — [`apply_drag`]
/// branches on `tiling`.
#[derive(Clone, Copy, PartialEq)]
pub struct Drag {
    /// Index of the dragged panel in the workspace `Vec`.
    pub idx: usize,
    /// Move or resize.
    pub kind: DragKind,
    /// Pointer x at pointer-down.
    pub start_mx: f64,
    /// Pointer y at pointer-down.
    pub start_my: f64,
    /// Panel x at pointer-down.
    pub start_x: f64,
    /// Panel y at pointer-down.
    pub start_y: f64,
    /// Panel width (or tiling width span) at pointer-down.
    pub start_w: f64,
    /// Panel height (or tiling height span) at pointer-down.
    pub start_h: f64,
}

/// Tiling-mode row height unit in the web shell's px — kept here so both
/// shells agree on the persisted meaning of [`PanelWin::tile_h`].
pub const TILE_ROW_PX: f64 = 150.0;
/// Width span ceiling (quarter-row units).
pub const TILE_W_MAX: u8 = 4;
/// Height span ceiling (rows).
pub const TILE_H_MAX: u8 = 6;

fn default_tile_w() -> u8 {
    1
}
fn default_tile_h() -> u8 {
    2
}

/// One panel's geometry + window state. `z` is the floating-mode stacking
/// order.
///
/// Stored geometry is the user's *intent*: when the viewport shrinks,
/// panels are clamped on screen at render time ([`effective_rect`]) but the
/// stored rect is left untouched, so they spring back when the viewport
/// grows. Build defaults with [`LayoutBuilder`].
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PanelWin<K> {
    /// Which panel this is (the app's [`PanelKind`]).
    pub kind: K,
    /// Left edge, relative to the workspace area (px on the web, cells in
    /// a terminal).
    pub x: f64,
    /// Top edge, relative to the workspace area.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
    /// Window state (normal / minimized to dock / maximized).
    pub state: WinState,
    /// Floating-mode stacking order; higher is in front.
    pub z: i32,
    /// Tiling-mode width span in quarter-row units (1–4): 1 ≈ a quarter of
    /// the row, 4 = the full row. Layouts saved before this field existed
    /// deserialize to 1.
    #[serde(default = "default_tile_w")]
    pub tile_w: u8,
    /// Tiling-mode height in rows (1–6). Defaults to 2.
    #[serde(default = "default_tile_h")]
    pub tile_h: u8,
}

impl<K> PanelWin<K> {
    /// Builder-style override of the tiling spans, clamped to their valid
    /// ranges (width 1–4 quarter-row units, height 1–6 rows).
    pub fn with_tile(mut self, w: u8, h: u8) -> Self {
        self.tile_w = w.clamp(1, TILE_W_MAX);
        self.tile_h = h.clamp(1, TILE_H_MAX);
        self
    }
}

/// Convenience builder for default layouts: hands out incrementing z values
/// so later panels stack in front of earlier ones.
pub struct LayoutBuilder {
    z: i32,
}

impl LayoutBuilder {
    /// Start a builder; the first panel gets `z = 1`.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { z: 0 }
    }

    /// A [`PanelWin`] at the given rect, in [`WinState::Floating`], with the
    /// next z value.
    pub fn at<K>(&mut self, kind: K, x: f64, y: f64, w: f64, h: f64) -> PanelWin<K> {
        self.z += 1;
        PanelWin {
            kind,
            x,
            y,
            w,
            h,
            state: WinState::Floating,
            z: self.z,
            tile_w: default_tile_w(),
            tile_h: default_tile_h(),
        }
    }
}

/// Viewport-clamp parameters for [`effective_rect`] — the unit-dependent
/// knobs that turn a raw viewport into a workspace area and keep panels
/// visible inside it.
pub struct Clamp {
    /// Subtracted from viewport width to get the workspace width.
    pub outer_w: f64,
    /// Subtracted from viewport height (top bar + dock chrome).
    pub outer_h: f64,
    /// Workspace width floor.
    pub floor_w: f64,
    /// Workspace height floor.
    pub floor_h: f64,
    /// A panel must leave this much workspace visible when sized.
    pub inner: f64,
    /// Edge padding when pulling a panel back on screen.
    pub edge: f64,
    /// Minimum panel width.
    pub min_w: f64,
    /// Minimum panel height.
    pub min_h: f64,
}

impl Clamp {
    /// The original panel-kit web (CSS px) behavior.
    pub const WEB: Clamp = Clamp {
        outer_w: 4.0,
        outer_h: 66.0, // topbar (~36) + dock (~30)
        floor_w: 220.0,
        floor_h: 180.0,
        inner: 12.0,
        edge: 6.0,
        min_w: 180.0,
        min_h: 110.0,
    };

    /// Character-cell defaults for terminal shells.
    pub const CELLS: Clamp = Clamp {
        outer_w: 0.0,
        outer_h: 0.0,
        floor_w: 24.0,
        floor_h: 8.0,
        inner: 2.0,
        edge: 0.0,
        min_w: 20.0,
        min_h: 5.0,
    };
}

/// Tiling-mode metrics: how pointer deltas snap to tile spans.
pub struct TileMetrics {
    /// Height of one tile row.
    pub row: f64,
    /// Floor for the width of one quarter-row column.
    pub col_floor: f64,
    /// Horizontal chrome subtracted from the viewport before computing the
    /// column width.
    pub outer: f64,
}

impl TileMetrics {
    /// The original panel-kit web (CSS px) behavior.
    pub const WEB: TileMetrics = TileMetrics {
        row: TILE_ROW_PX,
        col_floor: 80.0,
        outer: 16.0,
    };

    /// Character-cell defaults for terminal shells.
    pub const CELLS: TileMetrics = TileMetrics {
        row: 6.0,
        col_floor: 12.0,
        outer: 0.0,
    };
}

/// Next z value that stacks in front of every panel.
pub fn front_z<K>(ps: &[PanelWin<K>]) -> i32 {
    ps.iter().map(|p| p.z).max().unwrap_or(0) + 1
}

/// The on-screen geometry for a floating panel: shrunk if larger than the
/// viewport, pulled back so the whole panel stays visible.
///
/// This is a render-time projection — the stored [`PanelWin`] keeps the
/// user's intended geometry untouched. Mutating state instead would make
/// clamping one-way: shrink the window once and every panel stays crushed
/// after it grows back.
pub fn effective_rect<K>(p: &PanelWin<K>, vw: f64, vh: f64, c: &Clamp) -> (f64, f64, f64, f64) {
    let ws_w = (vw - c.outer_w).max(c.floor_w);
    let ws_h = (vh - c.outer_h).max(c.floor_h);
    let w = p.w.min(ws_w - c.inner).max(c.min_w);
    let h = p.h.min(ws_h - c.inner).max(c.min_h);
    let x = p.x.min(ws_w - w - c.edge).max(0.0);
    let y = p.y.min(ws_h - h - c.edge).max(0.0);
    (x, y, w, h)
}

/// Start a floating-mode move/resize drag from a pointer-down at
/// `(mx, my)`, capturing panel geometry into a [`Drag`].
///
/// Normalize-on-grab: what the user grabbed is the *clamped* on-screen rect
/// ([`effective_rect`]), which can differ from the stored geometry after a
/// viewport shrink. Writing it back on grab keeps the drag math anchored to
/// what's visible — no jump on the first pointer-move.
pub fn begin_drag<K>(
    panels: &mut [PanelWin<K>],
    idx: usize,
    kind: DragKind,
    mx: f64,
    my: f64,
    vw: f64,
    vh: f64,
    c: &Clamp,
) -> Option<Drag> {
    let p = panels.get_mut(idx)?;
    let (x, y, w, h) = effective_rect(p, vw, vh, c);
    (p.x, p.y, p.w, p.h) = (x, y, w, h);
    Some(Drag {
        idx,
        kind,
        start_mx: mx,
        start_my: my,
        start_x: p.x,
        start_y: p.y,
        start_w: p.w,
        start_h: p.h,
    })
}

/// Start a tiling-mode resize drag from the corner grip. Unlike
/// [`begin_drag`]'s free resize, pointer deltas snap the panel's tile spans
/// so tiles always land on fit sizes. `start_w`/`start_h` on the captured
/// [`Drag`] hold the *spans*, not lengths; [`apply_drag`] branches on
/// `tiling`.
pub fn begin_tile_resize<K>(
    panels: &[PanelWin<K>],
    idx: usize,
    mx: f64,
    my: f64,
) -> Option<Drag> {
    let p = panels.get(idx)?;
    Some(Drag {
        idx,
        kind: DragKind::Resize,
        start_mx: mx,
        start_my: my,
        start_x: 0.0,
        start_y: 0.0,
        start_w: p.tile_w as f64,
        start_h: p.tile_h as f64,
    })
}

/// Apply the in-flight [`Drag`] for a pointer now at `(mx, my)`.
///
/// `tiling` selects span-snapped resize (see [`begin_tile_resize`]); moves
/// follow the pointer and floating resizes clamp to [`Clamp::min_w`]/
/// [`Clamp::min_h`].
pub fn apply_drag<K>(
    panels: &mut [PanelWin<K>],
    d: &Drag,
    mx: f64,
    my: f64,
    tiling: bool,
    vw: f64,
    c: &Clamp,
    t: &TileMetrics,
) {
    let Some(p) = panels.get_mut(d.idx) else {
        return;
    };
    match d.kind {
        DragKind::Move => {
            p.x = (d.start_x + (mx - d.start_mx)).max(0.0);
            p.y = (d.start_y + (my - d.start_my)).max(0.0);
        }
        DragKind::Resize if tiling => {
            let col = ((vw - t.outer) / TILE_W_MAX as f64).max(t.col_floor);
            let dw = ((mx - d.start_mx) / col).round();
            let dh = ((my - d.start_my) / t.row).round();
            p.tile_w = (d.start_w + dw).clamp(1.0, TILE_W_MAX as f64) as u8;
            p.tile_h = (d.start_h + dh).clamp(1.0, TILE_H_MAX as f64) as u8;
        }
        DragKind::Resize => {
            p.w = (d.start_w + (mx - d.start_mx)).max(c.min_w);
            p.h = (d.start_h + (my - d.start_my)).max(c.min_h);
        }
    }
}

/// Tiling-mode reorder: move `dragged` into `target`'s slot. Moving down
/// the flow inserts after the target, moving up inserts before — the
/// classic sortable-list shuffle, so the dragged panel snaps into whichever
/// slot the pointer is over. `Vec` order is the tiling order.
pub fn reorder_tile<K: PartialEq + Copy>(panels: &mut Vec<PanelWin<K>>, dragged: K, target: K) {
    let Some(from) = panels.iter().position(|p| p.kind == dragged) else {
        return;
    };
    let Some(to) = panels.iter().position(|p| p.kind == target) else {
        return;
    };
    if from == to {
        return;
    }
    let p = panels.remove(from);
    let after_removal = if from < to { to - 1 } else { to };
    let insert_at = (if from < to { after_removal + 1 } else { after_removal }).min(panels.len());
    panels.insert(insert_at, p);
}

/// Restore and raise the panel of `kind`: un-minimizes it and brings it to
/// the front. No-op when the layout holds no panel of that kind.
pub fn restore<K: PartialEq + Copy>(panels: &mut [PanelWin<K>], kind: K) {
    let z = front_z(panels);
    if let Some(p) = panels.iter_mut().find(|p| p.kind == kind) {
        if p.state == WinState::Minimized {
            p.state = WinState::Floating;
        }
        p.z = z;
    }
}

/// Indices of the panels a renderer should draw, honoring minimize and
/// maximize: a maximized panel hides all others. Returns
/// `(visible, maximized_index)`.
pub fn visible_panels<K>(panels: &[PanelWin<K>]) -> (Vec<usize>, Option<usize>) {
    let maximized = panels.iter().position(|p| p.state == WinState::Maximized);
    let visible = match maximized {
        Some(mi) => vec![mi],
        None => panels
            .iter()
            .enumerate()
            .filter(|(_, p)| p.state != WinState::Minimized)
            .map(|(i, _)| i)
            .collect(),
    };
    (visible, maximized)
}

/// The persisted layout: panel geometry plus the layout mode. Shells decide
/// where this lives (localStorage, a JSON file, a KV bucket) — core only
/// defines the shape and the reconcile step.
#[derive(Serialize, Deserialize)]
pub struct SavedLayout<K> {
    /// All panels with their geometry and window state, in tiling order.
    pub panels: Vec<PanelWin<K>>,
    /// Whether tiling mode was active when saved.
    pub tiling: bool,
}

/// Reconcile a loaded layout against the current panel set: panels added to
/// the app since the layout was saved are appended with their default
/// placement, so new features still show up for existing users.
pub fn merge_defaults<K: PartialEq + Copy>(panels: &mut Vec<PanelWin<K>>, defaults: &[PanelWin<K>]) {
    for d in defaults {
        if !panels.iter().any(|p| p.kind == d.kind) {
            panels.push(*d);
        }
    }
}

/// CSS-safe slug of a panel title ("Filter Strip" -> "filter-strip").
pub fn kind_slug(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}
