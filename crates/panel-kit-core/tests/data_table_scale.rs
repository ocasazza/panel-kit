//! A million rows inside fixed budgets, each about four times its release
//! timing on an M-series laptop: tight enough that losing the packed sort keys,
//! the rank cache, the prefix refinement or the append's chunk sharing fails,
//! loose enough for a slower builder. Debug builds skip it; run with
//! `cargo test --release -p panel-kit-core --test data_table_scale`.

use std::ops::Range;
use std::time::{Duration, Instant};

use panel_kit_core::widgets::data_table::{
    BuildOptions, ColumnFilter, DataColumnSpec, Ordered, SortDir, SortSpec, TableData,
    TableDataBuilder, TableDefaults, TableState, Value, View,
};

#[path = "support/demo_table.rs"]
mod demo;

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

/// Rows `range` of the table's shape.
fn push_rows(builder: &mut TableDataBuilder, range: Range<usize>) {
    for n in range {
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
}

fn data_of(rows: usize) -> TableData {
    let mut builder = TableDataBuilder::with_capacity(&columns(), rows);
    push_rows(&mut builder, 0..rows);
    builder.build()
}

fn data() -> TableData {
    data_of(ROWS)
}

/// Unique titles sharing a 36-byte prefix, in an order unrelated to their sort
/// order (7_919 is prime, so `n * 7_919 % ROWS` visits every number once).
fn shared_prefix_data() -> TableData {
    let mut builder = TableDataBuilder::with_capacity(&columns(), ROWS);
    for n in 0..ROWS {
        let m = n * 7_919 % ROWS;
        let id = format!("imp-{n}");
        let title = format!(
            "Trace the ingest path through shard {m:07} replica {}",
            m % 3
        );
        builder.row(&id, [Value::Text(&id), Value::Text(&title)]);
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

#[test]
#[cfg_attr(debug_assertions, ignore = "timed: run with --release")]
fn appending_a_thousand_rows_to_a_million_costs_the_rows_added_not_the_rows_held() {
    let ms = Duration::from_millis;
    let columns = columns();
    let data = data();
    eprintln!("memory held by 1M rows: {} bytes", data.memory_bytes());
    data.warm(1);
    eprintln!("memory with the title ranks: {} bytes", data.memory_bytes());

    let appended = timed(Duration::from_millis(2), "appending 1k rows to 1M", || {
        data.append(|builder| push_rows(builder, ROWS..ROWS + 1_000))
    });
    timed(
        ms(5),
        "carrying the title ranks over the 1k appended rows",
        || appended.warm(1),
    );

    assert_eq!((data.len(), appended.len()), (ROWS, ROWS + 1_000));
    assert!(appended.extends(&data) && !data.extends(&appended));
    let by_title = TableState {
        sort: vec![SortSpec::new("title", SortDir::Asc)],
        ..TableState::new(&columns, TableDefaults::default())
    };
    let view = |data: &TableData| View::build(&columns, data, &by_title, BuildOptions::default());
    let (carried, rebuilt) = (view(&appended), view(&data_of(ROWS + 1_000)));
    assert_eq!(
        carried.rows, rebuilt.rows,
        "carried ranks order the rows as a table built at once does"
    );
    assert_eq!(view(&data).total, ROWS, "the earlier table keeps its rows");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timed: run with --release")]
fn a_million_unique_titles_sharing_a_long_prefix_rank_inside_their_budget() {
    let ms = Duration::from_millis;
    let columns = columns();
    let data = shared_prefix_data();
    timed(ms(200), "the first rank of 1M shared-prefix titles", || {
        data.warm(1)
    });

    let by_title = TableState {
        sort: vec![SortSpec::new("title", SortDir::Asc)],
        ..TableState::new(&columns, TableDefaults::default())
    };
    let view = View::build(&columns, &data, &by_title, BuildOptions::default());
    for (place, &row) in view.rows.iter().enumerate() {
        assert_eq!(
            row * 7_919 % ROWS,
            place,
            "the title at place {place} is shard {place}"
        );
    }
}

/// The demo's table: seven columns, three of them text, one with a unique
/// 55-byte note per row. Typing in the quick filter refines the whole table in
/// sort order, so rows are not read in the order they are stored. Budgets are
/// about four times the release timing on an M-series laptop (the first
/// filter also folds the non-ASCII titles).
#[test]
#[cfg_attr(debug_assertions, ignore = "timed: run with --release")]
fn a_quick_filter_over_the_demos_million_rows_stays_inside_its_budgets() {
    let ms = Duration::from_millis;
    let columns = demo::columns();
    let data = demo::data(ROWS);
    data.warm(0);
    data.warm(1);
    let options = BuildOptions::default();
    let plain = TableState::new(&columns, TableDefaults::default());
    let sorted = TableState {
        sort: vec![
            SortSpec::new("since", SortDir::Asc),
            SortSpec::new("item", SortDir::Asc),
        ],
        ..plain.clone()
    };
    let whole = Ordered::build(&columns, &data, &sorted, options);
    let holds = |row: usize, word: &str| {
        (0..columns.len()).any(|at| data.text(row, at).to_lowercase().contains(word))
    };

    for (text, budget) in [("seat", ms(120)), ("équipe", ms(25)), ("zzqx", ms(60))] {
        let mut state = sorted.clone();
        state.set_quick(text);
        let refined = timed(
            budget,
            &format!("refining 1M sorted rows by {text:?}"),
            || whole.refine(&columns, &data, &state, options),
        );
        let expected: Vec<usize> = whole
            .rows
            .iter()
            .copied()
            .filter(|&row| holds(row, text))
            .collect();
        assert_eq!(refined.rows, expected, "{text:?}");
    }

    let mut seat = plain.clone();
    seat.set_quick("seat");
    let built = timed(ms(60), "building 1M rows filtered by \"seat\"", || {
        Ordered::build(&columns, &data, &seat, options)
    });
    assert_eq!(built.rows.len(), 71_429, "the demo's rows holding \"seat\"");
}
