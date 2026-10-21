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
//! - [`content_width`]: the width to lay out at, from a measured container.
//! - [`Warp::scale`]: the time axis alone.
//! - [`split`]: the LANES mode, one flat lane per agent plus the unattributed
//!   events by source.
//! - [`Collaboration::build`] with [`current_items`]: the COLLAB mode, pairs
//!   and threads.
//! - [`view::graph`], [`view::lanes`], [`view::collaboration`]: the three modes
//!   derived from the events a host holds and the toolbar's `GraphState`.
//! - [`cluster`], [`merge`], [`summarize`], [`caption`], [`time`](super::time) stamps: the
//!   helpers the modes share.
//!
//! Events, lanes and tones are the shared vocabulary of [`super::model`] and
//! [`super::query`]; the strip's toggle state lives in its `GraphState`, whose
//! kind names [`kinds_of`](super::model::kinds_of) turns into
//! [`LooseOpen::Kinds`].

mod collab;
mod dot;
mod lanes;
mod layout;
pub mod metrics;
mod num;
mod pack;
mod strip;
pub mod view;
pub mod warp;

#[cfg(test)]
mod tests;

pub use collab::{
    current_items, Collaboration, Handoff, ItemInfo, Pair, Step, Supplied, Thread, STALE_AFTER_DAYS,
};
pub use dot::{caption, cluster, merge, title, Dot};
pub use lanes::{node, slug, split, Lane as ActivityLane, Node as ActivityNode, Split};
pub use layout::{
    content_width, summarize, EdgePath, Fork, Graph, Item, Options, Range, Rect, Summary,
};
pub use num::{fmt1, round_half_up};
pub use strip::{strip, Cell, LooseOpen, LooseRow, StripRow};
pub use warp::{Gap, Params as WarpParams, Tick, TickKind, Warp};
