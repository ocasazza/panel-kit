//! Shared builders of the swimlane suites: the clock of `graph_test.exs` and
//! `warp_test.exs`, and event constructors.

use std::sync::atomic::{AtomicU64, Ordering};

use super::super::{Graph, Options, Range};
use crate::widgets::event_graph::model::{Event, EventKind, Micros};
use crate::widgets::event_graph::time;

/// Microseconds in one second.
pub const SEC: i64 = 1_000_000;
/// Microseconds in one hour.
pub const HOUR: i64 = 3_600 * SEC;

/// `@since`: 2026-10-01T00:00:00Z, the window start of the graph suites.
pub fn since() -> Micros {
    time::from_utc(2026, 10, 1, 0, 0, 0)
}

/// `@now`: 2026-10-08T00:00:00Z, the window end of the graph suites.
pub fn now() -> Micros {
    time::from_utc(2026, 10, 8, 0, 0, 0)
}

/// `since + round(hours * 3600)` seconds: the instant `hours` after `base`.
pub fn after(base: Micros, hours: f64) -> Micros {
    base + (hours * 3600.0).round() as i64 * SEC
}

/// The instant `hours` after [`since`].
pub fn at(hours: f64) -> Micros {
    after(since(), hours)
}

/// A process-unique event id.
pub fn uid() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("t:{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// `ev(item, hour)` of `graph_test.exs`: a note by `worker` on `item`,
/// `hour` hours after [`since`]. The tests build every other kind by struct
/// update: `Event { kind: EventKind::Commit, ..ev("imp-1", 5.0) }`.
pub fn ev(item: &str, hour: f64) -> Event {
    Event::new(uid(), at(hour), EventKind::Note)
        .agent("worker")
        .item(item)
}

/// A no-item event of `kind` by `worker`, `hour` hours after [`since`].
pub fn loose_ev(kind: EventKind, hour: f64) -> Event {
    Event::new(uid(), at(hour), kind).agent("worker")
}

/// The options of `layout/2` in `graph_test.exs`: 1400px over `@since..@now`.
pub fn opts() -> Options<'static> {
    Options::new(1400, Range::new(since(), now()))
}

/// `layout(events)` of `graph_test.exs`: [`opts`], no cap, nothing open.
pub fn layout(events: &[Event]) -> Graph {
    Graph::layout(events, &opts())
}
