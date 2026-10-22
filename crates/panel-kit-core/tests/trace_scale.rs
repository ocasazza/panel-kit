//! The swimlane graph at a million events inside fixed budgets, each about
//! twice its release timing on an M-series laptop: tight enough that going back
//! to sorting events by id strings or building every item's dots fails, loose
//! enough for a slower builder. The 100k test runs everywhere with generous
//! bounds; the 1M and 3M ones skip debug builds. The tree toggle is on in the
//! timed tests, so every item without lanes derives them from its events, and
//! off in their twins. Run the timed ones with
//! `cargo test --release -p panel-kit-core --test trace_scale`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use panel_kit_core::widgets::trace::query::GraphState;
use panel_kit_core::widgets::trace::sample::{self, Activity};
use panel_kit_core::widgets::trace::swimlane::{view, Graph, Options, Range};
use panel_kit_core::widgets::trace::{Event, EventKind};

/// Counts live and peak heap bytes of the whole test binary.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(by: usize) {
    let live = LIVE.fetch_add(by, Relaxed) + by;
    PEAK.fetch_max(live, Relaxed);
}

// SAFETY: every call forwards to the system allocator unchanged and only
// counts the sizes it was asked for.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            grow(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(ptr, layout, new_size) };
        if !moved.is_null() {
            LIVE.fetch_sub(layout.size(), Relaxed);
            grow(new_size);
        }
        moved
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

const DAY_MS: i64 = 86_400_000;

/// The counters above are the whole binary's: a measured test holds this so
/// no other one allocates while it runs.
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn state(show_all: bool, tree: bool) -> GraphState {
    GraphState {
        window: "30d".to_owned(),
        tree,
        show_all,
        graph_width: 1_400,
        ..GraphState::default()
    }
}

fn graph(a: &Activity, show_all: bool, tree: bool) -> Graph {
    view::graph(&a.events, &state(show_all, tree), Some(&a.tree), a.now)
}

/// Runs `run`, returning its value, its time and the heap it peaked at above
/// where it started and still holds when it returns.
fn measured<T>(run: impl FnOnce() -> T) -> (T, Duration, usize, usize) {
    let before = LIVE.load(Relaxed);
    PEAK.store(before, Relaxed);
    let started = Instant::now();
    let out = run();
    let took = started.elapsed();
    let held = LIVE.load(Relaxed).saturating_sub(before);
    (out, took, PEAK.load(Relaxed).saturating_sub(before), held)
}

fn within(what: &str, took: Duration, budget: Duration) {
    eprintln!("{what}: {took:?} (budget {budget:?})");
    assert!(
        took <= budget,
        "{what} took {took:?}, over its {budget:?} budget"
    );
}

fn distinct_items(events: &[Event]) -> usize {
    events
        .iter()
        .filter_map(|e| e.item_id.as_deref())
        .collect::<HashSet<_>>()
        .len()
}

fn loose_of(events: &[Event], kind: EventKind) -> usize {
    events
        .iter()
        .filter(|e| e.item_id.is_none() && e.kind == kind)
        .count()
}

/// What a laid-out graph of `a` must say whatever the size. With the tree on
/// the items without lanes have derived them, so the rows hold fork stacks.
fn check_shape(a: &Activity, capped: &Graph, all: &Graph, tree: bool) {
    let items = distinct_items(&a.events);
    assert_eq!(capped.total_items, items);
    assert_eq!(capped.items.len(), 20);
    assert_eq!(capped.hidden, items - 20);
    assert_eq!(capped.item_count(), 20);
    assert!(
        capped.gaps.iter().any(|gap| gap.ms >= 2 * DAY_MS),
        "the idle days stay a gap"
    );
    for kind in [
        EventKind::BranchHead,
        EventKind::Commit,
        EventKind::PrMerged,
    ] {
        let drawn = capped
            .strip
            .iter()
            .find(|row| row.kind == kind)
            .map(|row| row.count);
        assert_eq!(
            drawn,
            Some(loose_of(&a.events, kind)),
            "{kind:?} strip count"
        );
    }

    assert_eq!((all.total_items, all.hidden), (items, 0));
    assert_eq!(all.item_count(), items);
    assert!(all.items.is_empty(), "a lazy graph builds no item up front");
    assert_eq!(all.strip, capped.strip);
    assert_eq!(all.ticks, capped.ticks);
    assert!(all.rows >= all.lanes && all.height > capped.height);
    assert_eq!(all.row_spans().len(), items);
    eprintln!("{} rows over {} packed rows, tree {tree}", all.rows, all.lanes);
    if tree {
        assert!(
            all.rows > 2 * all.lanes,
            "derived forks keep rows under their items: {} rows over {}",
            all.rows,
            all.lanes
        );
    } else {
        assert_eq!(all.rows, all.lanes, "no tree, no fork rows");
    }
}

/// Items drawn from a lazy graph agree with a built one over the same window.
fn check_window(a: &Activity, all: &Graph, tree: bool) {
    let band = all.span(0.0, 600.0);
    assert!(
        !band.is_empty() && band.len() < 1_000,
        "{} rows in 600px",
        band.len()
    );
    let rows = &all.row_spans()[band];
    assert!(rows.windows(2).all(|w| w[0].y <= w[1].y));
    let forked = rows
        .iter()
        .filter(|span| !all.item(&a.events, span.item).forks.is_empty())
        .count();
    assert_eq!(forked > rows.len() / 4, tree, "{forked} of {} rows fork", rows.len());
    let top = a.events.len().min(30_000);
    let small = &a.events[a.events.len() - top..];
    let opts = Options::new(1_400, Range::from_secs(0, None, a.now))
        .tree(&a.tree)
        .lazy(true);
    let lazy = Graph::layout_slice(small, |_| true, &opts);
    let built = Graph::layout_slice(small, |_| true, &opts.lazy(false));
    for span in lazy.row_spans().iter().take(40) {
        let item = lazy.item(small, span.item);
        assert_eq!(&*item, &built.items[span.item as usize]);
        assert_eq!(item.y, span.y);
    }
}

#[test]
fn a_hundred_thousand_events_lay_out_correctly_inside_a_generous_budget() {
    let _alone = serial();
    let slack = if cfg!(debug_assertions) { 40 } else { 1 };
    let a = sample::activity_sized(100_000);
    assert_eq!(a.events.len(), 100_000);

    for tree in [true, false] {
        let (capped, took, ..) = measured(|| graph(&a, false, tree));
        within(
            "100k capped layout",
            took,
            Duration::from_millis(100 * slack),
        );
        let (all, took, ..) = measured(|| graph(&a, true, tree));
        within("100k lazy layout", took, Duration::from_millis(150 * slack));
        check_shape(&a, &capped, &all, tree);
        check_window(&a, &all, tree);

        let (_, took, ..) = measured(|| {
            (0..all.item_count().min(60)).fold(0, |dots, at| {
                dots + all.item(&a.events, all.row_spans()[at].item).dots.len()
            })
        });
        within(
            "100k sixty items built on demand",
            took,
            Duration::from_millis(20 * slack),
        );
    }
}

/// The 1M budgets with the tree toggle `tree`.
fn a_million_events_lay_out_inside_their_budgets_with(tree: bool) {
    let _alone = serial();
    let ms = Duration::from_millis;
    let (a, took, peak, held) = measured(|| sample::activity_sized(1_000_000));
    eprintln!(
        "1M events: {} bytes each held, {} peak",
        held / 1_000_000,
        peak / 1_000_000
    );
    within("generating 1M events", took, ms(1_500));

    let (capped, took, peak, held) = measured(|| graph(&a, false, tree));
    eprintln!(
        "capped: {} B/event peak, {} B/event held",
        peak / 1_000_000,
        held / 1_000_000
    );
    within("1M capped layout", took, ms(250));
    assert!(
        peak <= 56 * 1_000_000,
        "capped layout peaked at {peak} bytes"
    );

    let (all, took, peak, held) = measured(|| graph(&a, true, tree));
    eprintln!(
        "lazy: {} B/event peak, {} B/event held",
        peak / 1_000_000,
        held / 1_000_000
    );
    within("1M lazy layout", took, ms(450));
    assert!(peak <= 80 * 1_000_000, "lazy layout peaked at {peak} bytes");
    assert!(held <= 24 * 1_000_000, "lazy layout holds {held} bytes");
    check_shape(&a, &capped, &all, tree);
    check_window(&a, &all, tree);

    let (_, took, ..) = measured(|| {
        (0..2_000).fold(0usize, |rows, n| {
            let y = n as f64 * all.height as f64 / 2_000.0;
            rows + all.span(y, y + 600.0).len()
        })
    });
    within("2000 window lookups over 290k rows", took, ms(10));
    let (_, took, ..) = measured(|| {
        let band = all.span(2_000.0, 2_600.0);
        all.row_spans()[band]
            .iter()
            .map(|span| all.item(&a.events, span.item).dots.len())
            .sum::<usize>()
    });
    within("building the items of a 600px window", took, ms(20));
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timed: run with --release")]
fn a_million_events_lay_out_inside_their_budgets() {
    a_million_events_lay_out_inside_their_budgets_with(true);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timed: run with --release")]
fn a_million_events_lay_out_inside_their_budgets_with_the_tree_off() {
    a_million_events_lay_out_inside_their_budgets_with(false);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timed: run with --release")]
fn three_million_events_still_lay_out_in_a_second() {
    let _alone = serial();
    let ms = Duration::from_millis;
    let a = sample::activity_sized(3_000_000);
    let (capped, took, ..) = measured(|| graph(&a, false, true));
    within("3M capped layout", took, ms(700));
    let (all, took, ..) = measured(|| graph(&a, true, true));
    within("3M lazy layout", took, ms(1_400));
    check_shape(&a, &capped, &all, true);
}
