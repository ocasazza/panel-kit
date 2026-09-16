use crate::reducer::{HitTarget, PanelPart};
use crate::{PanelKey, Region};

use super::{FrameStatus, PanelChromeProjection, ProjectedFrame};

/// Resolve a point against dock entries, controls, headers, bodies, and workspace.
pub fn hit_test<K: PanelKey>(
    frame: &ProjectedFrame<'_, K>,
    point: (f64, f64),
) -> Option<HitTarget<K>> {
    if frame.status != FrameStatus::Ready {
        return None;
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
