//! A million-row CSV: the streaming writer against the `Vec<Vec<String>>`
//! path it replaced, by output, time and peak heap. Debug builds skip it; run
//! with `cargo test --release -p panel-kit-core --test data_table_export -- --nocapture`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

use panel_kit_core::widgets::data_table::{
    write_csv, DataColumnSpec, TableData, TableDataBuilder, Value,
};

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        CALLS.fetch_add(1, Relaxed);
        let live = LIVE.fetch_add(layout.size(), Relaxed) + layout.size();
        PEAK.fetch_max(live, Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        let live = LIVE.fetch_add(new, Relaxed) + new - layout.size();
        LIVE.fetch_sub(layout.size(), Relaxed);
        PEAK.fetch_max(live, Relaxed);
        unsafe { System.realloc(ptr, layout, new) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

const ROWS: usize = 1_000_000;

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
        let title = format!("Trace, \"the\" ingest path {} end to end", n % 977);
        builder.row(
            &id,
            [
                Value::Text(&id),
                Value::Text(&title),
                Value::Text(["open", "done", "in progress"][n % 3]),
                Value::Number(((n * 7_919) % 1_000_000) as f64),
            ],
        );
    }
    builder.build()
}

/// The allocating CSV the painter used to build: a `String` per cell, a `Vec`
/// per row, a joined `String` per line.
fn old_csv(headers: &[String], rows: &[Vec<String>]) -> String {
    let cell = |text: &str| {
        if text.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", text.replace('"', "\"\""))
        } else {
            text.to_string()
        }
    };
    let line = |cells: &[String]| cells.iter().map(|c| cell(c)).collect::<Vec<_>>().join(",");
    std::iter::once(line(headers))
        .chain(rows.iter().map(|row| line(row)))
        .map(|line| line + "\r\n")
        .collect()
}

/// Run `work`, returning its result, its time, the heap it peaked above what
/// was live before it started, and how many allocations it made.
fn measured<T>(work: impl FnOnce() -> T) -> (T, std::time::Duration, usize, usize) {
    let base = LIVE.load(Relaxed);
    PEAK.store(base, Relaxed);
    let calls = CALLS.load(Relaxed);
    let started = Instant::now();
    let out = work();
    let took = started.elapsed();
    (
        out,
        took,
        PEAK.load(Relaxed).saturating_sub(base),
        CALLS.load(Relaxed) - calls,
    )
}

#[test]
fn a_million_row_csv_streams_in_one_buffer() {
    if cfg!(debug_assertions) {
        return;
    }
    let data = data();
    let headers: Vec<String> = ["item", "title", "state", "since"].map(String::from).to_vec();
    let columns = [0, 1, 2, 3];
    let rows: Vec<usize> = (0..data.len()).collect();

    let (old, old_took, old_peak, old_calls) = measured(|| {
        let body: Vec<Vec<String>> = rows
            .iter()
            .map(|&row| {
                columns
                    .iter()
                    .map(|&c| data.text(row, c).into_owned())
                    .collect()
            })
            .collect();
        old_csv(&headers, &body)
    });
    let (new, new_took, new_peak, new_calls) = measured(|| {
        let mut out = String::new();
        write_csv(&mut out, &data, &headers, rows.iter().copied(), &columns);
        out
    });
    eprintln!(
        "csv of {ROWS} rows, {} MB: old {old_took:?} peak {} MB {old_calls} allocs; \
         streamed {new_took:?} peak {} MB {new_calls} allocs",
        new.len() >> 20,
        old_peak >> 20,
        new_peak >> 20,
    );
    assert!(old == new, "streamed CSV differs from the old path's");
    // One growing buffer: at most the output plus its doubling slack.
    assert!(
        new_peak <= new.len() * 2 + (1 << 20),
        "streaming peaked at {new_peak} bytes for {} bytes of CSV",
        new.len()
    );
    // Nothing per cell or per row: the buffer's growth is the only allocation.
    assert!(new_calls < 100, "streaming made {new_calls} allocations");
    assert!(new_peak < old_peak, "peak {new_peak} vs old {old_peak}");
}
