//! The one table component's model: columns, the rows by column, state, the
//! view a state yields, the tag ribbon, and exports. Renderer-neutral: painters
//! read it, a host may own it, and its durable part persists through any serde
//! format.
//!
//! Ported from the board's LiveView data table (`ImpAgentWeb.Components.
//! DataTable` and its `State`, `View`, `Column`, `Tags`); the behaviour and the
//! test suite (`tests.rs`) follow it. `benches/data_table.rs` holds it to
//! a million rows.

mod column;
mod data;
mod export;
mod scan;
mod state;
mod stored;
mod tags;
mod view;

#[cfg(test)]
mod tests;

pub use column::{natural_cmp, Align, ColumnKind, DataColumnSpec, SortDir, SortSpec};
pub use data::{RowWriter, TableData, TableDataBuilder, Value};
pub use export::{csv, tsv, write_csv, write_tsv};
pub use state::{
    ActiveFilter, ColumnFilter, Density, FilterKind, Mounted, MoveTarget, OrderKey, PersistedTable,
    SelectMode, Selectable, TableDefaults, TableEvent, TableKey, TableQuery, TableState,
    TableWindow, ViewMode, DEFAULT_PAGE, MAX_WIDTH, MAX_WINDOW, MIN_WIDTH, NARROW_WIDTH, OVERSCAN,
    QUICK_LIMIT, STATE_VERSION, WINDOW_COUNT,
};
pub use stored::{Field, Numeric, StoredFilter, StoredTable};
pub use tags::{
    canonical_tag, ribbon_label, row_labels, segment_label, tag_hash, tag_letter, tag_slot,
    RibbonMode, TagMode, MAX_TAG_LENGTH, TAG_SLOTS,
};
pub use view::{
    empty_sentence, tally, BuildOptions, Entry, Ordered, QueryMode, View, OPTION_LIMIT,
};
