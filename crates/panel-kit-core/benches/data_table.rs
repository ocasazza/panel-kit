//! The table model at scale: loading 10k, 100k and 1M rows, and building the
//! view of them under each query shape a user can ask for.

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use panel_kit_core::widgets::data_table::{
    BuildOptions, ColumnFilter, DataColumnSpec, Ordered, Selectable, SortDir, SortSpec, TableData,
    TableDataBuilder, TableDefaults, TableKey, TableState, Value, View,
};

const STATES: [&str; 6] = [
    "open",
    "claimed",
    "in_progress",
    "review",
    "blocked",
    "done",
];
const AGENTS: [&str; 4] = ["FHKDV644W6-MBP", "selfdev@GN9", "L96N-mini", ""];
const TAGS: [&[&str]; 3] = [&[], &["raft"], &["ui", "ops"]];

fn columns() -> Vec<DataColumnSpec> {
    vec![
        DataColumnSpec::new("item", "item"),
        DataColumnSpec::new("title", "title"),
        DataColumnSpec::new("agent", "agent").enumerated(),
        DataColumnSpec::new("state", "state").enumerated(),
        DataColumnSpec::new("since", "since").number(),
        DataColumnSpec::new("type", "type").enumerated(),
    ]
}

/// Rows `range` of the benchmark table's shape.
fn push_rows(builder: &mut TableDataBuilder, range: std::ops::Range<usize>) {
    let mut id = String::new();
    let mut title = String::new();
    for n in range {
        use std::fmt::Write;
        id.clear();
        title.clear();
        let _ = write!(id, "imp-{n}");
        let _ = write!(title, "Trace the ingest path {} end to end", n % 977);
        let agent = AGENTS[n % AGENTS.len()];
        builder
            .row(
                id.as_str(),
                [
                    Value::Text(&id),
                    Value::Text(&title),
                    if agent.is_empty() {
                        Value::Missing
                    } else {
                        Value::Text(agent)
                    },
                    Value::Text(STATES[n % STATES.len()]),
                    Value::Number(((n * 7_919) % 1_000_000) as f64),
                    Value::Text("T"),
                ],
            )
            .tags(TAGS[n % TAGS.len()]);
    }
}

fn data(count: usize) -> TableData {
    let mut builder = TableDataBuilder::with_capacity(&columns(), count);
    push_rows(&mut builder, 0..count);
    builder.build()
}

/// Unique titles sharing a long prefix, in an order unrelated to their sort
/// order (7_919 is prime, so `n * 7_919 % count` visits every number once).
fn shared_prefix(count: usize) -> TableData {
    let mut builder = TableDataBuilder::with_capacity(&columns(), count);
    for n in 0..count {
        let m = n * 7_919 % count;
        let id = format!("imp-{n}");
        let title = format!(
            "Trace the ingest path through shard {m:07} replica {}",
            m % 3
        );
        builder.row(&id, [Value::Text(&id), Value::Text(&title)]);
    }
    builder.build()
}

/// Each query shape, as a state.
fn queries() -> Vec<(&'static str, TableState)> {
    let base = || TableState::new(&columns(), TableDefaults::default());
    let sorted = |column: &str, dir| TableState {
        sort: vec![SortSpec::new(column, dir)],
        ..base()
    };
    let mut quick = base();
    quick.set_quick("ingest 97");
    let mut values = base();
    values.set_filter("state", ColumnFilter::values(["open", "review"]));
    let mut tags = base();
    tags.toggle_tag("raft", false);
    let grouped = TableState {
        group_by: Some("state".into()),
        sort: vec![SortSpec::new("since", SortDir::Desc)],
        ..base()
    };
    let mut everything = grouped.clone();
    everything.set_quick("ingest");
    everything.set_filter("since", ColumnFilter::range(Some(1_000.0), None));
    vec![
        ("unsorted", base()),
        ("sort_text", sorted("title", SortDir::Asc)),
        ("sort_natural_id", sorted("item", SortDir::Desc)),
        ("sort_number", sorted("since", SortDir::Asc)),
        ("quick_filter", quick),
        ("enum_filter", values),
        ("tag_filter", tags),
        ("group_and_sort", grouped),
        ("everything", everything),
    ]
}

fn load(c: &mut Criterion) {
    let mut group = c.benchmark_group("load");
    group.sample_size(10);
    for count in [10_000, 100_000, 1_000_000] {
        group.bench_function(BenchmarkId::from_parameter(count), |b| {
            b.iter(|| data(count))
        });
    }
    group.finish();
}

fn view_build(c: &mut Criterion) {
    let columns = columns();
    for count in [10_000, 100_000, 1_000_000] {
        let mut group = c.benchmark_group(format!("view/{count}"));
        group.sample_size(10);
        for (name, state) in queries() {
            // Warm: ranks already derived, as for every sort after the first.
            let warm = data(count);
            group.bench_with_input(BenchmarkId::new("warm", name), &state, |b, state| {
                b.iter(|| View::build(&columns, &warm, state, BuildOptions::default()))
            });
        }
        // Cold: the first sort of a column pays for its ranks.
        let cold = TableState {
            sort: vec![SortSpec::new("title", SortDir::Asc)],
            ..TableState::new(&columns, TableDefaults::default())
        };
        group.bench_function(BenchmarkId::new("cold", "sort_text"), |b| {
            b.iter_batched(
                || data(count),
                |fresh| View::build(&columns, &fresh, &cold, BuildOptions::default()),
                BatchSize::LargeInput,
            )
        });
        group.finish();
    }
}

/// What the painter pays per render and per keystroke at 1M rows: selecting
/// every row, cloning the state that holds them, and one arrow key.
fn state_ops(c: &mut Criterion) {
    let columns = columns();
    let data = data(1_000_000);
    let multi = TableState {
        selectable: Selectable::Multi,
        ..TableState::new(&columns, TableDefaults::default())
    };
    let view = View::build(&columns, &data, &multi, BuildOptions::default());
    let mut group = c.benchmark_group("state/1000000");
    group.sample_size(10);
    group.bench_function("select_all", |b| {
        b.iter_batched(
            || multi.clone(),
            |mut s| {
                s.select_all(&view, &data);
                s
            },
            BatchSize::LargeInput,
        )
    });
    let mut all = multi.clone();
    all.select_all(&view, &data);
    group.bench_function("clone_after_select_all", |b| b.iter(|| all.clone()));
    group.bench_function("select_all_again_deselects", |b| {
        b.iter_batched(
            || all.clone(),
            |mut s| {
                s.select_all(&view, &data);
                s
            },
            BatchSize::LargeInput,
        )
    });
    let mut parked = multi.clone();
    parked.cursor = Some(view.id(&data, 900_000).to_string());
    // Cursor set by a click: no remembered position, so the first key scans.
    group.bench_function("key_down_cold", |b| {
        b.iter_batched(
            || parked.clone(),
            |mut s| {
                s.key(TableKey::Down, false, &view, &data, 20);
                s
            },
            BatchSize::LargeInput,
        )
    });
    // Cursor already moved by a key: its position is remembered.
    parked.key(TableKey::Down, false, &view, &data, 20);
    group.bench_function("key_down", |b| {
        b.iter_batched(
            || parked.clone(),
            |mut s| {
                s.key(TableKey::Down, false, &view, &data, 20);
                s
            },
            BatchSize::LargeInput,
        )
    });
    group.finish();
}

/// A title of mostly-ASCII text with accented letters, so a query that names
/// them cannot take the ASCII path.
fn data_unicode(count: usize) -> TableData {
    let columns = columns();
    let mut builder = TableDataBuilder::with_capacity(&columns, count);
    let mut id = String::new();
    let mut title = String::new();
    for n in 0..count {
        use std::fmt::Write;
        id.clear();
        title.clear();
        let _ = write!(id, "imp-{n}");
        let _ = write!(title, "Tracé the ingést path {} ünd to end", n % 977);
        builder.row(id.as_str(), [Value::Text(&id), Value::Text(&title)]);
    }
    builder.build()
}

fn quick(text: &str) -> TableState {
    let mut state = TableState::new(&columns(), TableDefaults::default());
    state.set_quick(text);
    state
}

/// Typing one more letter into the quick filter: build again, or refine.
fn refine(c: &mut Criterion) {
    let columns = columns();
    let data = data(1_000_000);
    let (first, second) = (quick("s"), quick("se"));
    let options = BuildOptions::default();
    let earlier = Ordered::build(&columns, &data, &first, options);
    let mut group = c.benchmark_group("refine/1000000");
    group.sample_size(10);
    group.bench_function("s_to_se/build", |b| {
        b.iter(|| Ordered::build(&columns, &data, &second, options))
    });
    group.bench_function("s_to_se/refine", |b| {
        b.iter(|| earlier.refine(&columns, &data, &second, options))
    });
    // A query that already cut the rows down: the refine reads few of them.
    let (first, second) = (quick("ingest 97"), quick("ingest 977"));
    let earlier = Ordered::build(&columns, &data, &first, options);
    group.bench_function("ingest_97_to_977/build", |b| {
        b.iter(|| Ordered::build(&columns, &data, &second, options))
    });
    group.bench_function("ingest_97_to_977/refine", |b| {
        b.iter(|| earlier.refine(&columns, &data, &second, options))
    });
    group.finish();
}

fn unicode_quick(c: &mut Criterion) {
    let columns = columns();
    let data = data_unicode(1_000_000);
    let state = quick("ingést 97");
    let mut group = c.benchmark_group("unicode_quick/1000000");
    group.sample_size(10);
    group.bench_function("build", |b| {
        b.iter(|| Ordered::build(&columns, &data, &state, BuildOptions::default()))
    });
    group.finish();
}

/// Grouping where nearly every row is its own group: the cost is the run
/// detection and the labels, not the filter or the sort.
fn group_unique(c: &mut Criterion) {
    let columns = columns();
    let data = data(1_000_000);
    let mut group = c.benchmark_group("group_unique/1000000");
    group.sample_size(10);
    for column in ["item", "since"] {
        let state = TableState {
            group_by: Some(column.into()),
            ..TableState::new(&columns, TableDefaults::default())
        };
        let ordered = Ordered::build(&columns, &data, &state, BuildOptions::default());
        group.bench_function(column, |b| {
            b.iter(|| View::group(&ordered, &columns, &data, &state))
        });
    }
    group.finish();
}

/// Appending to a million rows costs the rows added (plus carrying the title
/// ranks), not the rows held.
fn append(c: &mut Criterion) {
    let mut group = c.benchmark_group("append");
    group.sample_size(10);
    let held = 1_000_000;
    let base = data(held);
    base.warm(1);
    for count in [1_000, 100_000] {
        group.bench_function(BenchmarkId::new("rows", count), |b| {
            b.iter(|| base.append(|builder| push_rows(builder, held..held + count)))
        });
        group.bench_function(BenchmarkId::new("rows_and_title_ranks", count), |b| {
            b.iter(|| {
                let next = base.append(|builder| push_rows(builder, held..held + count));
                next.warm(1);
                next
            })
        });
    }
    group.finish();
}

/// The first rank of a column whose strings share long prefixes.
fn cold_rank(c: &mut Criterion) {
    let mut group = c.benchmark_group("rank");
    group.sample_size(10);
    for count in [100_000, 1_000_000] {
        group.bench_function(BenchmarkId::new("cold_shared_prefix", count), |b| {
            b.iter_batched(
                || shared_prefix(count),
                |fresh| {
                    fresh.warm(1);
                    fresh
                },
                BatchSize::LargeInput,
            )
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    load,
    view_build,
    state_ops,
    refine,
    unicode_quick,
    group_unique,
    append,
    cold_rank
);
criterion_main!(benches);
