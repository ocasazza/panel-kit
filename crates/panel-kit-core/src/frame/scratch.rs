use crate::PanelKey;

use super::{DockProjection, PanelProjection};

/// Caller-owned reusable projection scratch.
pub struct ProjectionBuffer<K: PanelKey> {
    pub(super) panel_order: Vec<usize>,
    pub(super) panels: Vec<PanelProjection<K>>,
    pub(super) dock: Vec<DockProjection<K>>,
    pub(super) tile_rows: Vec<TilePlacement>,
}

impl<K: PanelKey> ProjectionBuffer<K> {
    /// Allocate projection scratch sized for `panels` source records.
    pub fn with_panel_capacity(panels: usize) -> Self {
        Self {
            panel_order: Vec::with_capacity(panels),
            panels: Vec::with_capacity(panels),
            dock: Vec::with_capacity(panels),
            tile_rows: Vec::with_capacity(panels),
        }
    }

    /// Reserve capacity for additional panel, dock, and tile records.
    pub fn reserve_panels(&mut self, additional: usize) {
        self.panel_order.reserve(additional);
        self.panels.reserve(additional);
        self.dock.reserve(additional);
        self.tile_rows.reserve(additional);
    }

    pub(super) fn clear(&mut self) {
        self.panel_order.clear();
        self.panels.clear();
        self.dock.clear();
        self.tile_rows.clear();
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
