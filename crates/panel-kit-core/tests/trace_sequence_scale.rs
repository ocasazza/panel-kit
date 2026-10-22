//! LANES and COLLAB shaping at a million events inside fixed budgets: the
//! sequence diagram, one lifeline's view of it, and the pair and thread tables.
//! Each budget is about three times its release timing on an M-series laptop.
//! Debug builds skip it; run with
//! `cargo test --release -p panel-kit-core --test trace_sequence_scale -- --nocapture`.

use std::time::{Duration, Instant};

use panel_kit_core::widgets::trace::query::{GraphState, LanePick};
use panel_kit_core::widgets::trace::sample::activity_sized;
use panel_kit_core::widgets::trace::swimlane::{
    pair_table, thread_table, view, SeqRow, LANE_CAP,
};

const EVENTS: usize = 1_000_000;

fn timed<T>(budget: Duration, what: &str, run: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let out = run();
    let took = started.elapsed();
    eprintln!("{what}: {took:?} (budget {budget:?})");
    assert!(
        took <= budget,
        "{what} took {took:?}, over its {budget:?} budget"
    );
    out
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timed: run with --release")]
fn a_million_events_shape_into_a_sequence_and_collaboration_tables_inside_their_budgets() {
    let ms = Duration::from_millis;
    let activity = activity_sized(EVENTS);
    let events = &activity.events;
    let state = GraphState {
        window: "30d".to_owned(),
        ..GraphState::default()
    };

    let seq = timed(ms(450), "the sequence of 1M events", || {
        view::sequence(events, &state, activity.now, Some(false))
    });
    assert_eq!(seq.read, EVENTS);
    assert!(seq.lanes.len() <= LANE_CAP, "{} lanes", seq.lanes.len());
    assert!(seq.drawn > 10_000, "{} rows", seq.drawn);
    assert!(seq.rows.windows(2).all(|pair| pair[0].at() <= pair[1].at()));
    assert!(seq
        .rows
        .iter()
        .any(|row| matches!(row, SeqRow::Broadcast { .. })));
    assert!(seq
        .rows
        .iter()
        .any(|row| matches!(row, SeqRow::Edge { .. })));

    timed(ms(450), "the sequence with plain activity drawn", || {
        view::sequence(events, &state, activity.now, Some(true))
    });
    let three = LanePick::Only(seq.lanes.iter().take(3).map(|l| l.agent.clone()).collect());
    let folded = timed(ms(10), "folding all but three lanes", || seq.show(&three));
    assert!(folded.folds() && folded.lanes.len() == 4);
    assert_eq!(folded.rows.len(), seq.rows.len(), "folding moves no row");
    timed(ms(5), "one lifeline's rows", || seq.touching(0));

    let collab = timed(ms(1_200), "the collaboration of 1M events", || {
        view::collaboration(events, &state, activity.now)
    });
    assert!(!collab.pairs.is_empty() && !collab.threads.is_empty());
    let pairs = timed(ms(5), "the pair table", || pair_table(&collab));
    assert_eq!(pairs.len(), collab.pairs.len());
    let threads = timed(ms(15), "the thread table", || thread_table(&collab));
    assert_eq!(threads.len(), collab.threads.len());
}
