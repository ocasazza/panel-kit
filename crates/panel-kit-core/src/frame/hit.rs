use crate::reducer::{HitTarget, PanelPart};
use crate::{PanelKey, Region};

use super::{FrameStatus, PanelChromeProjection, Placement, ProjectedFrame};

/// Resolve a point against dock entries, controls, headers, bodies, and workspace.
///
/// While a tile is held or lifted, every point resolves to a resting tile
/// slot instead ([`nearest_tile_slot`]): the drawn geometry is moving under
/// the pointer, the slots are not, so the swap target cannot oscillate.
pub fn hit_test<K: PanelKey>(
    frame: &ProjectedFrame<'_, K>,
    point: (f64, f64),
) -> Option<HitTarget<K>> {
    if frame.status != FrameStatus::Ready {
        return None;
    }

    if frame
        .panels
        .iter()
        .any(|panel| panel.tile_role.tracks_pointer())
    {
        let slots = frame
            .panels
            .iter()
            .filter(|panel| matches!(panel.placement, Placement::Tiled { .. }))
            .map(|panel| (panel.key, panel.slot));
        return nearest_tile_slot(slots, point).map(|key| HitTarget::Panel {
            key,
            part: PanelPart::Surface,
        });
    }

    for dock in frame.dock.iter().rev() {
        if contains(dock.region, point) {
            return Some(HitTarget::Dock { key: dock.key });
        }
    }

    for panel in frame.panels.iter().rev() {
        if let Some(part) = hit_panel(panel.chrome, point) {
            return Some(HitTarget::Panel {
                key: panel.key,
                part,
            });
        }
    }

    contains(frame.chrome.workspace, point).then_some(HitTarget::Workspace)
}

/// The slot containing `point`, else the slot nearest to it; the first slot
/// wins ties. Gaps, padding, and points past the grid's edges therefore
/// still name a slot, so a drag toward an edge targets the edge tile.
pub fn nearest_tile_slot<K>(
    slots: impl IntoIterator<Item = (K, Region)>,
    point: (f64, f64),
) -> Option<K> {
    let mut best: Option<(K, f64)> = None;
    for (key, slot) in slots {
        let distance = distance_sq(slot, point);
        if best.as_ref().is_none_or(|(_, nearest)| distance < *nearest) {
            best = Some((key, distance));
        }
    }
    best.map(|(key, _)| key)
}

fn distance_sq(region: Region, (x, y): (f64, f64)) -> f64 {
    let dx = (region.x - x).max(x - (region.x + region.w)).max(0.0);
    let dy = (region.y - y).max(y - (region.y + region.h)).max(0.0);
    dx * dx + dy * dy
}

fn hit_panel(chrome: PanelChromeProjection, point: (f64, f64)) -> Option<PanelPart> {
    if chrome
        .resize_hit
        .is_some_and(|region| contains(region, point))
    {
        return Some(PanelPart::ResizeGrip);
    }
    if chrome
        .maximize_hit
        .is_some_and(|region| contains(region, point))
    {
        return Some(PanelPart::MaximizeControl);
    }
    if chrome
        .minimize_hit
        .is_some_and(|region| contains(region, point))
    {
        return Some(PanelPart::MinimizeControl);
    }
    if chrome
        .mode_hit
        .is_some_and(|region| contains(region, point))
    {
        return Some(PanelPart::ModeControl);
    }
    if contains(chrome.header_hit, point) {
        return Some(PanelPart::Header);
    }
    if contains(chrome.body, point) {
        return Some(PanelPart::Surface);
    }
    None
}

fn contains(region: Region, (x, y): (f64, f64)) -> bool {
    x >= region.x && y >= region.y && x < region.x + region.w && y < region.y + region.h
}
