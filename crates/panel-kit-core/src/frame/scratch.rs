use crate::{PanelKey, PanelWin, Region};

use super::{DockProjection, PanelProjection};

/// Caller-owned reusable projection scratch.
pub struct ProjectionBuffer<K: PanelKey> {
    pub(super) panel_order: Vec<usize>,
    pub(super) panels: Vec<PanelProjection<K>>,
    pub(super) dock: Vec<DockProjection<K>>,
    pub(super) tile_rows: Vec<TilePlacement>,
    pub(super) drag: TileDragScratch<K>,
}

/// Alternate tile layouts a tile drag projects against: the pre-drop base,
/// the preview for the current target, and the preview the displaced tiles
/// are leaving.
pub(super) struct TileDragScratch<K: PanelKey> {
    pub(super) panels: Vec<PanelWin<K>>,
    pub(super) rows: Vec<TilePlacement>,
    pub(super) base: Vec<(K, Region)>,
    pub(super) to: Vec<(K, Region)>,
    pub(super) from: Vec<(K, Region)>,
}

impl<K: PanelKey> TileDragScratch<K> {
    fn with_capacity(panels: usize) -> Self {
        Self {
            panels: Vec::with_capacity(panels),
            rows: Vec::with_capacity(panels),
            base: Vec::with_capacity(panels),
            to: Vec::with_capacity(panels),
            from: Vec::with_capacity(panels),
        }
    }

    fn reserve(&mut self, additional: usize) {
        self.panels.reserve(additional);
        self.rows.reserve(additional);
        self.base.reserve(additional);
        self.to.reserve(additional);
        self.from.reserve(additional);
    }

    fn clear(&mut self) {
        self.panels.clear();
        self.rows.clear();
        self.base.clear();
        self.to.clear();
        self.from.clear();
    }
}

impl<K: PanelKey> ProjectionBuffer<K> {
    /// Allocate projection scratch sized for `panels` source records.
    pub fn with_panel_capacity(panels: usize) -> Self {
        Self {
            panel_order: Vec::with_capacity(panels),
            panels: Vec::with_capacity(panels),
            dock: Vec::with_capacity(panels),
            tile_rows: Vec::with_capacity(panels),
            drag: TileDragScratch::with_capacity(panels),
        }
    }

    /// Reserve capacity for additional panel, dock, and tile records.
    pub fn reserve_panels(&mut self, additional: usize) {
        self.panel_order.reserve(additional);
        self.panels.reserve(additional);
        self.dock.reserve(additional);
        self.tile_rows.reserve(additional);
        self.drag.reserve(additional);
    }

    pub(super) fn clear(&mut self) {
        self.panel_order.clear();
        self.panels.clear();
        self.dock.clear();
        self.tile_rows.clear();
        self.drag.clear();
    }
}

#[derive(Clone, Copy)]
pub(super) struct TilePlacement {
    pub(super) source_index: usize,
    pub(super) column: u8,
    pub(super) row: u16,
    pub(super) column_span: u8,
    pub(super) row_span: u8,
}
