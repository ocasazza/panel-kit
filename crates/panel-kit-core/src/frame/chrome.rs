use crate::{Region, SurfaceProfile, WinState};

use super::{ChromeProjectionInput, PanelChromeProjection};

#[derive(Clone, Copy)]
pub(super) struct PanelChromeMetrics {
    header_h: f64,
    hit: f64,
    light_gap: f64,
    light_left: f64,
    resize_w: f64,
    resize_h: f64,
}

impl PanelChromeMetrics {
    pub(super) fn for_input(chrome: &ChromeProjectionInput, surface: SurfaceProfile) -> Self {
        if !chrome.panel_frame {
            return Self {
                header_h: 0.0,
                hit: 0.0,
                light_gap: 0.0,
                light_left: 0.0,
                resize_w: 0.0,
                resize_h: 0.0,
            };
        }

        let hit = if surface.caps.coarse_pointer {
            chrome.hit_target_min.max(44.0)
        } else {
            chrome.hit_target_min
        };
        let cells = chrome.metrics == crate::ChromeMetrics::CELLS;
        Self {
            header_h: chrome.panel_header_h,
            hit,
            light_gap: if cells { 1.0 } else { 6.0 },
            light_left: if cells { 2.0 } else { 8.0 },
            resize_w: if cells { 2.0 } else { hit },
            resize_h: if cells { 1.0 } else { hit },
        }
    }
}

pub(super) fn panel_chrome(
    region: Region,
    state: WinState,
    surface: SurfaceProfile,
    chrome: &ChromeProjectionInput,
    metrics: PanelChromeMetrics,
) -> PanelChromeProjection {
    let header_h = metrics.header_h.min(region.h).max(0.0);
    let body = panel_body(region, header_h, chrome);
    let header_hit = Region::new(region.x, region.y, region.w, header_h);
    let restore_only = surface.must_offer_restore(state);
    let window_management = surface.window_management();
    let mode_hit = (chrome.mode_control && window_management).then(|| light_region(region, metrics, 0));
    let minimize_hit = (chrome.minimize_control && window_management).then(|| light_region(region, metrics, 1));
    let maximize_hit = (chrome.maximize_control && (window_management || restore_only)).then(|| {
        if window_management {
            light_region(region, metrics, 2)
        } else {
            light_region(region, metrics, 0)
        }
    });
    let resize_hit = (chrome.resize_grip && window_management).then(|| {
        Region::new(
            (region.x + region.w - metrics.resize_w).max(region.x),
            (region.y + region.h - metrics.resize_h).max(region.y),
            metrics.resize_w.min(region.w).max(0.0),
            metrics.resize_h.min(region.h).max(0.0),
        )
    });

    PanelChromeProjection { outer: region, body, header_hit, mode_hit, minimize_hit, maximize_hit, resize_hit }
}

fn panel_body(region: Region, header_h: f64, chrome: &ChromeProjectionInput) -> Region {
    if !chrome.panel_frame {
        return region;
    }

    if chrome.metrics == crate::ChromeMetrics::CELLS {
        return Region::new(
            region.x + 1.0,
            region.y + 1.0,
            (region.w - 2.0).max(0.0),
            (region.h - 2.0).max(0.0),
        );
    }

    Region::new(
        region.x,
        region.y + header_h,
        region.w,
        (region.h - header_h).max(0.0),
    )
}

fn light_region(region: Region, metrics: PanelChromeMetrics, slot: u8) -> Region {
    let x = region.x + metrics.light_left + slot as f64 * (metrics.hit + metrics.light_gap);
    Region::new(x, region.y, metrics.hit.min(region.w).max(0.0), metrics.header_h.min(region.h).max(0.0))
}
