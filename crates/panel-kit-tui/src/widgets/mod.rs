//! Independently callable ratatui composition parts.
//!
//! Hosts project a borrowed frame with `panel-kit-core`, then call whichever
//! pieces they want in their own render order: root shell, panel surface,
//! optional chrome, traffic lights, resize grip, dock, and scrollbar.

mod border;
pub(crate) use border::draw_border;

pub mod dock;
pub mod panel;
pub mod root;

use panel_kit_core::reducer::{HitTarget, PanelPart};
use panel_kit_core::PanelKey;
use ratatui::layout::Rect;

/// Reusable, caller-owned storage for TUI hit regions recorded by parts.
///
/// Painters clear lengths between frames while retaining capacity. Hosts should
/// reserve once for the largest expected frame, then reuse the buffer to avoid
/// per-frame panel/dock scratch vectors.
#[derive(Debug)]
pub struct TuiHitBuffer<K: PanelKey> {
    panels: Vec<TuiPanelHit<K>>,
    docks: Vec<TuiDockHit<K>>,
}

impl<K: PanelKey> TuiHitBuffer<K> {
    /// Create an empty hit buffer with explicit panel and dock capacities.
    pub fn with_capacity(panel_capacity: usize, dock_capacity: usize) -> Self {
        Self {
            panels: Vec::with_capacity(panel_capacity),
            docks: Vec::with_capacity(dock_capacity),
        }
    }

    /// Clear frame-local hit records while keeping allocated capacity.
    pub fn clear(&mut self) {
        self.panels.clear();
        self.docks.clear();
    }

    /// Ensure the reusable storage can hold the next frame without reallocating.
    pub fn reserve(&mut self, panel_capacity: usize, dock_capacity: usize) {
        self.panels
            .reserve(panel_capacity.saturating_sub(self.panels.capacity()));
        self.docks
            .reserve(dock_capacity.saturating_sub(self.docks.capacity()));
    }

    /// Resolve a terminal-cell point against recorded dock, control, header, and body hits.
    pub fn hit_test(&self, point: (f64, f64)) -> Option<HitTarget<K>> {
        for dock in self.docks.iter().rev() {
            if contains(dock.region, point) {
                return Some(HitTarget::Dock { key: dock.key });
            }
        }

        for panel in self.panels.iter().rev() {
            if let Some(part) = panel.hit_test(point) {
                return Some(HitTarget::Panel {
                    key: panel.key,
                    part,
                });
            }
        }

        None
    }

    pub(crate) fn record_panel_surface(
        &mut self,
        key: K,
        source_index: usize,
        outer: Rect,
        body: Rect,
    ) {
        self.panel_hit_mut(key, source_index)
            .record_surface(outer, body);
    }

    pub(crate) fn record_panel_header(&mut self, key: K, source_index: usize, header: Rect) {
        self.panel_hit_mut(key, source_index).header = visible_rect(header);
    }

    pub(crate) fn record_panel_part(
        &mut self,
        key: K,
        source_index: usize,
        part: PanelPart,
        region: Rect,
    ) {
        self.panel_hit_mut(key, source_index)
            .record_part(part, region);
    }

    pub(crate) fn record_dock(&mut self, key: K, source_index: usize, region: Rect) {
        self.docks.push(TuiDockHit {
            key,
            source_index,
            region,
        });
    }

    fn panel_hit_mut(&mut self, key: K, source_index: usize) -> &mut TuiPanelHit<K> {
        if let Some(index) = self
            .panels
            .iter()
            .position(|panel| panel.key == key && panel.source_index == source_index)
        {
            return &mut self.panels[index];
        }

        self.panels.push(TuiPanelHit::new(key, source_index));
        self.panels.last_mut().expect("panel hit was just pushed")
    }
}

#[derive(Clone, Copy, Debug)]
struct TuiPanelHit<K: PanelKey> {
    key: K,
    source_index: usize,
    outer: Option<Rect>,
    body: Option<Rect>,
    header: Option<Rect>,
    mode: Option<Rect>,
    minimize: Option<Rect>,
    maximize: Option<Rect>,
    resize: Option<Rect>,
}

impl<K: PanelKey> TuiPanelHit<K> {
    fn new(key: K, source_index: usize) -> Self {
        Self {
            key,
            source_index,
            outer: None,
            body: None,
            header: None,
            mode: None,
            minimize: None,
            maximize: None,
            resize: None,
        }
    }

    fn record_surface(&mut self, outer: Rect, body: Rect) {
        self.outer = visible_rect(outer);
        self.body = visible_rect(body);
    }

    fn record_part(&mut self, part: PanelPart, region: Rect) {
        let visible = visible_rect(region);
        match part {
            PanelPart::Surface => self.body = visible,
            PanelPart::Header => self.header = visible,
            PanelPart::ModeControl => self.mode = visible,
            PanelPart::MinimizeControl => self.minimize = visible,
            PanelPart::MaximizeControl => self.maximize = visible,
            PanelPart::ResizeGrip => self.resize = visible,
        }
    }

    fn hit_test(&self, point: (f64, f64)) -> Option<PanelPart> {
        if self.resize.is_some_and(|region| contains(region, point)) {
            return Some(PanelPart::ResizeGrip);
        }
        if self.maximize.is_some_and(|region| contains(region, point)) {
            return Some(PanelPart::MaximizeControl);
        }
        if self.minimize.is_some_and(|region| contains(region, point)) {
            return Some(PanelPart::MinimizeControl);
        }
        if self.mode.is_some_and(|region| contains(region, point)) {
            return Some(PanelPart::ModeControl);
        }
        if self.header.is_some_and(|region| contains(region, point)) {
            return Some(PanelPart::Header);
        }
        if self.body.is_some_and(|region| contains(region, point)) {
            return Some(PanelPart::Surface);
        }
        None
    }
}

#[derive(Clone, Copy, Debug)]
struct TuiDockHit<K: PanelKey> {
    key: K,
    #[allow(dead_code)]
    source_index: usize,
    region: Rect,
}

fn visible_rect(rect: Rect) -> Option<Rect> {
    (rect.width > 0 && rect.height > 0).then_some(rect)
}

fn contains(rect: Rect, (x, y): (f64, f64)) -> bool {
    x >= rect.x as f64 && y >= rect.y as f64 && x < rect.right() as f64 && y < rect.bottom() as f64
}
