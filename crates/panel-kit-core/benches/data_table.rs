//! The table model at scale: loading 10k, 100k and 1M rows, and building the
//! view of them under each query shape a user can ask for.

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use panel_kit_core::widgets::data_table::{
    BuildOptions, ColumnFilter, DataColumnSpec, Ordered, SortDir, SortSpec, TableData,
    TableDataBuilder, TableDefaults, TableState, Value, View,
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

fn data(count: usize) -> TableData {
    let columns = columns();
    let mut builder = TableDataBuilder::with_capacity(&columns, count);
    let mut id = String::new();
    let mut title = String::new();
    for n in 0..count {
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

criterion_group!(
    benches,
    load,
    view_build,
    refine,
    unicode_quick,
    group_unique
);
criterion_main!(benches);
