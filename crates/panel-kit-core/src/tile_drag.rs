//! Tiling-mode header drag: press, lift, follow, swap.
//!
//! A press on a tiled panel's header starts a [`TileDrag`] in
//! [`TileDragPhase::Pressed`]. Motion past [`lift_threshold`] lifts it: the
//! panel follows the pointer offset, and the tile under the pointer becomes
//! the swap target. Release commits the swap to the panel order and settles
//! the panel into its new slot; a cancel (Escape, pointer cancel, leaving the
//! window) flies it back home without touching the order.
//!
//! The drag is gesture state only. It lives in
//! [`Snapshot::tile_drag`](crate::reducer::Snapshot::tile_drag), is never
//! persisted, and its animation advances only on host-supplied
//! [`WorkspaceEvent::Tick`](crate::reducer::WorkspaceEvent::Tick) time.
//!
//! Drop semantics are a swap: the dragged panel and the target exchange
//! positions in the tiling order and every other panel keeps its index. A
//! swap is its own inverse, which is what lets the projection rebuild the
//! pre-drop layout while a committed drop is still settling.

use crate::{PanelWin, Units, WinState};

/// Pointer travel, in CSS pixels, before a pressed tile lifts.
pub const LIFT_THRESHOLD_PX: f64 = 4.0;
/// Pointer travel, in character cells, before a pressed tile lifts.
pub const LIFT_THRESHOLD_CELLS: f64 = 1.0;
/// Duration of the rise once a tile lifts, in milliseconds.
pub const LIFT_MS: f64 = 140.0;
/// Duration of the flight into a slot after release or cancel, in milliseconds.
pub const LAND_MS: f64 = 200.0;
/// Duration of the displaced tiles' move toward a new preview layout, in
/// milliseconds.
pub const RETARGET_MS: f64 = 160.0;
/// Extra scale a fully lifted tile is drawn at (`1.0 + LIFT_SCALE`).
pub const LIFT_SCALE: f64 = 0.03;
/// Opacity a fully lifted tile loses (`1.0 - LIFT_FADE`).
pub const LIFT_FADE: f64 = 0.12;

/// Pointer travel before a pressed tile lifts, in `units`.
pub fn lift_threshold(units: Units) -> f64 {
    match units {
        Units::CssPx => LIFT_THRESHOLD_PX,
        Units::Cells => LIFT_THRESHOLD_CELLS,
    }
}

/// Cubic ease-out on `t` clamped to `[0, 1]`.
pub fn ease_out(t: f64) -> f64 {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    1.0 - (1.0 - t).powi(3)
}

/// Draw scale for a tile at `elevation` (`0` resting, `1` fully lifted).
pub fn lift_scale(elevation: f64) -> f64 {
    1.0 + LIFT_SCALE * elevation.clamp(0.0, 1.0)
}

/// Draw opacity for a tile at `elevation` (`0` resting, `1` fully lifted).
pub fn lift_opacity(elevation: f64) -> f64 {
    1.0 - LIFT_FADE * elevation.clamp(0.0, 1.0)
}

/// Where a tile drag is in its gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileDragPhase {
    /// Pressed on the header; the pointer has not yet travelled past
    /// [`lift_threshold`]. Releasing here is a click.
    Pressed,
    /// Lifted and following the pointer; `target` is the live swap target.
    Lifted,
    /// Released: the swap with `target` (if any) is already committed to the
    /// panel order and the tile is flying into its slot.
    Settling,
    /// Cancelled: the order is unchanged and the tile is flying home.
    Cancelling,
}

/// One in-flight tiling header drag.
///
/// Coordinates are in whatever consistent space the host's pointer events
/// use; only the difference `pointer - press` is applied to geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileDrag<K> {
    /// The dragged panel.
    pub key: K,
    /// Gesture phase.
    pub phase: TileDragPhase,
    /// Pointer position at the press.
    pub press: (f64, f64),
    /// Latest pointer position.
    pub pointer: (f64, f64),
    /// Swap target: the panel whose slot the dragged tile will take.
    /// `None` is the dragged tile's own slot. While settling this is the
    /// panel the committed swap exchanged with.
    pub target: Option<K>,
    /// The target before the most recent retarget; displaced tiles animate
    /// from its preview layout toward `target`'s.
    pub previous_target: Option<K>,
    /// Progress of the displaced tiles' move from `previous_target`'s
    /// preview toward `target`'s, `0..=1`.
    pub retarget: f64,
    /// Progress of the rise, `0..=1`. Frozen once the tile is released.
    pub lift: f64,
    /// Progress of the flight into the slot while settling or cancelling,
    /// `0..=1`.
    pub land: f64,
}

impl<K: Copy + PartialEq> TileDrag<K> {
    /// A fresh press on `key`'s header at `at`.
    pub fn pressed(key: K, at: (f64, f64)) -> Self {
        Self {
            key,
            phase: TileDragPhase::Pressed,
            press: at,
            pointer: at,
            target: None,
            previous_target: None,
            retarget: 1.0,
            lift: 0.0,
            land: 0.0,
        }
    }

    /// Whether pointer motion still steers the drag (pressed or lifted).
    pub fn tracks_pointer(&self) -> bool {
        matches!(self.phase, TileDragPhase::Pressed | TileDragPhase::Lifted)
    }

    /// Whether another [`Tick`](crate::reducer::WorkspaceEvent::Tick) would
    /// change what is drawn.
    pub fn animating(&self) -> bool {
        match self.phase {
            TileDragPhase::Pressed => false,
            TileDragPhase::Lifted => self.lift < 1.0 || self.retarget < 1.0,
            TileDragPhase::Settling | TileDragPhase::Cancelling => true,
        }
    }

    /// Pointer travel since the press.
    pub fn offset(&self) -> (f64, f64) {
        (self.pointer.0 - self.press.0, self.pointer.1 - self.press.1)
    }

    /// Eased height above the workspace plane, `0..=1`: rises while lifted,
    /// falls while settling or cancelling.
    pub fn elevation(&self) -> f64 {
        match self.phase {
            TileDragPhase::Pressed => 0.0,
            TileDragPhase::Lifted => ease_out(self.lift),
            TileDragPhase::Settling | TileDragPhase::Cancelling => {
                ease_out(self.lift) * (1.0 - ease_out(self.land))
            }
        }
    }

    /// Whether pointer travel since the press has crossed the lift threshold.
    pub(crate) fn past_threshold(&self, units: Units) -> bool {
        let (dx, dy) = self.offset();
        dx.abs().max(dy.abs()) >= lift_threshold(units)
    }

    /// Point the drag at `target`, restarting the displaced-tile animation
    /// when it changes.
    pub(crate) fn retarget(&mut self, target: Option<K>) {
        if self.target == target {
            return;
        }
        self.previous_target = self.target;
        self.target = target;
        self.retarget = 0.0;
    }

    /// Turn a lifted drag into a flight home with the order untouched.
    pub(crate) fn cancel(&mut self) {
        self.retarget(None);
        self.phase = TileDragPhase::Cancelling;
        self.land = 0.0;
    }

    /// Advance the animation by `dt_ms`. Returns `true` once a landing
    /// finished and the drag should be dropped.
    pub(crate) fn advance(&mut self, dt_ms: f64) -> bool {
        let step = |value: &mut f64, duration: f64| *value = (*value + dt_ms / duration).min(1.0);
        match self.phase {
            TileDragPhase::Pressed => false,
            TileDragPhase::Lifted => {
                step(&mut self.lift, LIFT_MS);
                step(&mut self.retarget, RETARGET_MS);
                false
            }
            TileDragPhase::Settling | TileDragPhase::Cancelling => {
                step(&mut self.land, LAND_MS);
                step(&mut self.retarget, RETARGET_MS);
                self.land >= 1.0
            }
        }
    }
}

/// Exchange the tiling-order positions of panels `a` and `b`. Every other
/// panel keeps its index. Returns whether anything moved.
pub fn swap_tiles<K: PartialEq + Copy>(panels: &mut [PanelWin<K>], a: K, b: K) -> bool {
    let Some(from) = panels.iter().position(|panel| panel.kind == a) else {
        return false;
    };
    let Some(to) = panels.iter().position(|panel| panel.kind == b) else {
        return false;
    };
    if from == to {
        return false;
    }
    panels.swap(from, to);
    true
}

/// Whether `key` names a panel that currently occupies a tile slot.
pub(crate) fn occupies_slot<K: PartialEq>(panels: &[PanelWin<K>], key: &K) -> bool {
    panels
        .iter()
        .any(|panel| panel.kind == *key && panel.state == WinState::Floating)
}

/// A panel's part in an in-flight tile drag, as projected for painters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TileRole {
    /// Not involved, or displaced and moving with the preview layout.
    #[default]
    Resting,
    /// Pressed, still under the lift threshold.
    Held,
    /// Lifted and following the pointer.
    Lifted,
    /// Released or cancelled, flying into its slot.
    Landing,
    /// The live swap target of a lifted tile.
    Target,
}

impl TileRole {
    /// Whether pointer motion resolves against resting slots rather than
    /// drawn geometry: true while a tile is held or lifted.
    pub fn tracks_pointer(self) -> bool {
        matches!(self, TileRole::Held | TileRole::Lifted)
    }

    /// Whether the panel is the dragged one in any phase.
    pub fn is_dragged(self) -> bool {
        matches!(self, TileRole::Held | TileRole::Lifted | TileRole::Landing)
    }

    /// Whether the panel is drawn above the plane: lifted or landing.
    pub fn is_elevated(self) -> bool {
        matches!(self, TileRole::Lifted | TileRole::Landing)
    }
}
