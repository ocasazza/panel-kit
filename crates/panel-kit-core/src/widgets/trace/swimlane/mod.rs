//! Horizontal swimlane graph model: the activity window across time.
//!
//! Renderer-neutral and pure. Every function returns plain structs with the
//! geometry already resolved (x/y, SVG path strings, tone and kind enums), so
//! a painter only emits DOM. Instants are `i64` microseconds since the Unix
//! epoch (UTC); pixels are `f64` CSS px at the width the host measured.
//!
//! Ported from the board's activity graph (`Activity.Graph.layout/2`,
//! `Activity.Warp`, `Activity.Collaboration` and the lanes split of
//! `ActivityView`); the behaviour and the suites in `tests/` follow it.
//!
//! Entry points:
//! - [`Graph::layout`] with [`Options`] and [`Range`]: the GRAPH mode. Items
//!   packed into rows, dots with bursts merged, fork lanes and elbows, the
//!   per-kind no-item strip, the density-warped axis with its ticks and gaps.
//!   The host filters first and passes the survivors:
//!   `Graph::layout(events.iter().filter(|e| filters.keep(*e)), &opts)`.
//! - [`Graph::layout_slice`] with [`Options::lazy`]: the same layout over a
//!   slice of events for a million of them. Items are not built; the graph
//!   holds each row's [`RowSpan`], [`Graph::span`] finds the rows of a band
//!   of y and [`Graph::item`] builds the ones a painter draws. Events are
//!   grouped by hashing item ids, never by sorting them, so a layout is
//!   linear in the events: about 100 ms at 1M events capped to the newest
//!   rows, 200 ms with every row laid out.
//! - [`content_width`]: the width to lay out at, from a measured container.
//! - [`Warp::scale`]: the time axis alone.
//! - [`sequence`]: the LANES mode, a sequence diagram: one lifeline per agent,
//!   edges where two agents meet, broadcasts for global actions.
//! - [`Collaboration::build`] with [`current_items`]: the COLLAB mode, pairs
//!   and threads.
//! - [`view::graph`], [`view::sequence`], [`view::collaboration`]: the three modes
//!   derived from the events a host holds and the toolbar's `GraphState`.
//! - [`cluster`], [`merge`], [`summarize`], [`caption`], [`time`](super::time) stamps: the
//!   helpers the modes share.
//!
//! Events, lanes and tones are the shared vocabulary of [`super::model`] and
//! [`super::query`]; the strip's toggle state lives in its `GraphState`, whose
//! kind names [`kinds_of`](super::model::kinds_of) turns into
//! [`LooseOpen::Kinds`].

mod collab;
mod derive;
mod dot;
mod group;
mod layout;
pub mod metrics;
mod node;
mod num;
mod pack;
mod sequence;
mod strip;
mod table;
pub mod view;
pub mod warp;

#[cfg(test)]
mod tests;

pub use collab::{
    current_items, Collaboration, Handoff, ItemInfo, Pair, Step, Supplied, Thread, STALE_AFTER_DAYS,
};
pub use derive::{derive_forks, FORK_CAP};
pub use dot::{caption, cluster, merge, title, Dot};
pub use layout::{
    content_width, summarize, EdgePath, Fork, Graph, Item, Options, Range, Rect, RowSpan, Summary,
};
pub use node::{FactBlock, Inspection, ItemHeader, Node, CLUSTER_OPEN_CAP, PAIR_LIST_CAP};
pub use num::{fmt1, round_half_up};
pub use sequence::{
    clock_label, day_label, detail, label, sequence, shows_count, slug, Columns, EdgeKind,
    LaneHead, Options as SequenceOptions, RosterLane, SeqRow, Sequence, ACTIVITY_AUTO_MAX,
    BUSIEST_LANES, COUNT_MIN_W, FOLD_W, GUTTER_W, LANE_CAP, LANE_MIN_W, ROW_PITCH,
};
pub use strip::{strip, Cell, LooseOpen, LooseRow, StripRow};
pub use table::{
    pair_columns, pair_of_row, pair_row, pair_table, thread_columns, thread_of_row, thread_row,
    thread_table,
};
pub use warp::{Gap, Params as WarpParams, Tick, TickKind, Warp};
