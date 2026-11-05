//! Projection of an in-flight tile drag onto drawn panel rectangles.
//!
//! Every rectangle is derived from the committed tiling order plus at most
//! two swaps involving the dragged panel, so the projection needs no state
//! beyond [`TileDrag`]: the pre-drop base (the committed order with a
//! settling swap undone), the preview for `target`, and the preview for
//! `previous_target` that displaced tiles are animating away from.

use crate::reducer::Snapshot;
use crate::tile_drag::{ease_out, occupies_slot, swap_tiles, TileDrag, TileDragPhase, TileRole};
use crate::{PanelKey, PanelWin, Region, WinState};

use super::scratch::{TileDragScratch, TilePlacement};
use super::tiling::{pack_tiles, tile_region};
use super::TileLayoutMetrics;

/// The tile drag the projection should draw, if its panel occupies a slot
/// and no maximized panel hides the grid.
pub(super) fn active_tile_drag<K: PanelKey>(snapshot: &Snapshot<K>) -> Option<TileDrag<K>> {
    let drag = snapshot.tile_drag?;
    let maximized = snapshot
        .panels
        .iter()
        .any(|panel| panel.state == WinState::Maximized);
    (!maximized && occupies_slot(&snapshot.panels, &drag.key)).then_some(drag)
}

/// Pack the base, target-preview, and previous-preview layouts into scratch.
pub(super) fn prepare<K: PanelKey>(
    snapshot: &Snapshot<K>,
    drag: &TileDrag<K>,
    workspace: Region,
    metrics: &TileLayoutMetrics,
    scroll: f64,
    scratch: &mut TileDragScratch<K>,
) {
    let settled_swap = match drag.phase {
        TileDragPhase::Settling => drag.target,
        _ => None,
    };
    let TileDragScratch {
        panels,
        rows,
        base,
        to,
        from,
    } = scratch;
    let mut layout = |second: Option<K>, out: &mut Vec<(K, Region)>| {
        LayoutPass {
            source: &snapshot.panels,
            key: drag.key,
            swaps: [settled_swap, second],
            workspace,
            metrics,
            scroll,
        }
        .run(panels, rows, out);
    };
    layout(None, base);
    layout(drag.target, to);
    layout(drag.previous_target, from);
}

struct LayoutPass<'a, K: PanelKey> {
    source: &'a [PanelWin<K>],
    key: K,
    swaps: [Option<K>; 2],
    workspace: Region,
    metrics: &'a TileLayoutMetrics,
    scroll: f64,
}

impl<K: PanelKey> LayoutPass<'_, K> {
    fn run(
        &self,
        panels: &mut Vec<PanelWin<K>>,
        rows: &mut Vec<TilePlacement>,
        out: &mut Vec<(K, Region)>,
    ) {
        panels.clear();
        panels.extend_from_slice(self.source);
        for other in self.swaps.into_iter().flatten() {
            swap_tiles(panels, self.key, other);
        }
        rows.clear();
        let grid = pack_tiles(panels, self.workspace, self.metrics, rows);
        out.clear();
        out.extend(rows.iter().map(|placement| {
            let (region, _) = tile_region(self.workspace, grid, *placement, self.scroll);
            (panels[placement.source_index].kind, region)
        }));
    }
}

/// The drawn rectangle, drag role, and elevation for the panel `key` whose
/// committed slot is `slot`.
pub(super) fn drag_region<K: PanelKey>(
    drag: &TileDrag<K>,
    key: K,
    slot: Region,
    scratch: &TileDragScratch<K>,
) -> (Region, TileRole, f64) {
    let lookup = |layout: &[(K, Region)]| {
        layout
            .iter()
            .find(|(candidate, _)| *candidate == key)
            .map_or(slot, |(_, region)| *region)
    };

    if key == drag.key {
        let (dx, dy) = drag.offset();
        let home = lookup(&scratch.base);
        let lifted = Region::new(home.x + dx, home.y + dy, home.w, home.h);
        return match drag.phase {
            TileDragPhase::Pressed => (slot, TileRole::Held, 0.0),
            TileDragPhase::Lifted => (lifted, TileRole::Lifted, drag.elevation()),
            TileDragPhase::Settling | TileDragPhase::Cancelling => (
                lerp(lifted, slot, ease_out(drag.land)),
                TileRole::Landing,
                drag.elevation(),
            ),
        };
    }

    let region = lerp(
        lookup(&scratch.from),
        lookup(&scratch.to),
        ease_out(drag.retarget),
    );
    let role = if drag.phase == TileDragPhase::Lifted && drag.target == Some(key) {
        TileRole::Target
    } else {
        TileRole::Resting
    };
    (region, role, 0.0)
}

fn lerp(a: Region, b: Region, t: f64) -> Region {
    let mix = |from: f64, to: f64| from + (to - from) * t;
    Region::new(mix(a.x, b.x), mix(a.y, b.y), mix(a.w, b.w), mix(a.h, b.h))
}
