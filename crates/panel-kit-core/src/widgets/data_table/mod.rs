//! The one table component's model: columns, state, the view a state yields,
//! the tag ribbon, and exports. Renderer-neutral: painters read it, a host may
//! own it, and its durable part persists through any serde format.
//!
//! Ported from the board's LiveView data table (`ImpAgentWeb.Components.
//! DataTable` and its `State`, `View`, `Column`, `Tags`); the behaviour and the
//! test suite (`tests.rs`) follow it.

mod column;
mod export;
mod state;
mod stored;
mod tags;
mod view;

#[cfg(test)]
mod tests;

pub use column::{natural_cmp, Align, ColumnKind, DataColumnSpec, SortDir, SortKey, SortSpec};
pub use export::{csv, tsv};
pub use state::{
    ColumnFilter, Density, Mounted, MoveTarget, PersistedTable, SelectMode, Selectable,
    TableDefaults, TableEvent, TableKey, TableQuery, TableState, TableWindow, ViewMode,
    DEFAULT_PAGE, MAX_WIDTH, MAX_WINDOW, MIN_WIDTH, NARROW_WIDTH, OVERSCAN, QUICK_LIMIT,
    STATE_VERSION, WINDOW_COUNT,
};
pub use stored::{Field, Numeric, StoredFilter, StoredTable};
pub use tags::{
    canonical_tag, ribbon_label, row_labels, segment_label, tag_hash, tag_letter, tag_slot,
    RibbonMode, TagMode, MAX_TAG_LENGTH, TAG_SLOTS,
};
pub use view::{empty_sentence, tally, Entry, QueryMode, RowKeys, View, OPTION_LIMIT};
