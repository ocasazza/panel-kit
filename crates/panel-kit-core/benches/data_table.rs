//! The table model at scale: loading 10k, 100k and 1M rows, and building the
//! view of them under each query shape a user can ask for.

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use panel_kit_core::widgets::data_table::{
    BuildOptions, ColumnFilter, DataColumnSpec, SortDir, SortSpec, TableData, TableDataBuilder,
    TableDefaults, TableState, Value, View,
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

criterion_group!(benches, load, view_build, append, cold_rank);
criterion_main!(benches);
