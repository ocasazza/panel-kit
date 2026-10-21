//! A million rows inside fixed budgets, each about four times its release
//! timing on an M-series laptop: tight enough that losing the packed sort keys
//! or the rank cache fails, loose enough for a slower builder. Debug builds
//! skip it; run with `cargo test --release -p panel-kit-core --test data_table_scale`.

use std::time::{Duration, Instant};

use panel_kit_core::widgets::data_table::{
    BuildOptions, ColumnFilter, DataColumnSpec, SortDir, SortSpec, TableData, TableDataBuilder,
    TableDefaults, TableState, Value, View,
};

const ROWS: usize = 1_000_000;
const STATES: [&str; 6] = [
    "open",
    "claimed",
    "in_progress",
    "review",
    "blocked",
    "done",
];
const TAGS: [&[&str]; 3] = [&[], &["raft"], &["ui", "ops"]];

fn columns() -> Vec<DataColumnSpec> {
    vec![
        DataColumnSpec::new("item", "item"),
        DataColumnSpec::new("title", "title"),
        DataColumnSpec::new("state", "state").enumerated(),
        DataColumnSpec::new("since", "since").number(),
    ]
}

fn data() -> TableData {
    let columns = columns();
    let mut builder = TableDataBuilder::with_capacity(&columns, ROWS);
    for n in 0..ROWS {
        let id = format!("imp-{n}");
        let title = format!("Trace the ingest path {} end to end", n % 977);
        builder
            .row(
                &id,
                [
                    Value::Text(&id),
                    Value::Text(&title),
                    Value::Text(STATES[n % STATES.len()]),
                    Value::Number(((n * 7_919) % 1_000_000) as f64),
                ],
            )
            .tags(TAGS[n % TAGS.len()]);
    }
    builder.build()
}

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
fn a_million_rows_load_sort_filter_and_group_inside_their_budgets() {
    let ms = Duration::from_millis;
    let columns = columns();
    let data = timed(ms(600), "loading 1M rows", data);
    let view = |state: &TableState| View::build(&columns, &data, state, BuildOptions::default());
    let base = || TableState::new(&columns, TableDefaults::default());
    let sorted = |column: &str, dir| TableState {
        sort: vec![SortSpec::new(column, dir)],
        ..base()
    };

    let all = timed(ms(40), "the unsorted view", || view(&base()));
    assert_eq!(all.total, ROWS);

    let by_title = timed(ms(500), "the first title sort (ranks)", || {
        view(&sorted("title", SortDir::Asc))
    });
    assert_eq!(by_title.id(&data, 0), "imp-0");
    timed(ms(100), "a warm title sort", || {
        view(&sorted("title", SortDir::Desc))
    });
    timed(ms(100), "a number sort", || {
        view(&sorted("since", SortDir::Asc))
    });
    let by_id = timed(ms(150), "the first sort of unique ids (ranks)", || {
        view(&sorted("item", SortDir::Desc))
    });
    assert_eq!(by_id.id(&data, 0), format!("imp-{}", ROWS - 1));

    let mut quick = base();
    quick.set_quick("ingest 97");
    let narrowed = timed(ms(300), "a quick filter", || view(&quick));
    assert!(narrowed.total < ROWS);

    let mut everything = TableState {
        group_by: Some("state".into()),
        sort: vec![SortSpec::new("since", SortDir::Desc)],
        ..base()
    };
    everything.set_filter("since", ColumnFilter::range(Some(1_000.0), None));
    everything.toggle_tag("raft", false);
    let grouped = timed(ms(80), "group, sort, range and tag filter together", || {
        view(&everything)
    });
    assert!(
        grouped.entries.len() > grouped.rows.len(),
        "groups head the rows"
    );
}
