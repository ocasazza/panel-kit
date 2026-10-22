//! COLLAB mode as tables: the pairs and the threads of a [`Collaboration`],
//! one row each, stored by column in the shared table component's data.
//!
//! A list cell holds its whole list (capped at [`PAIR_LIST_CAP`]), which is
//! what filters match, a tooltip shows and a copy or CSV carries; a painter
//! draws the first few names and a `+N` tail from the model. Rows are in the
//! model's order and named by their index, so a row id leads back to its pair
//! or thread.

use std::fmt::Write;

use super::super::time::format_iso8601;
use super::collab::Collaboration;
use super::node::PAIR_LIST_CAP;
use crate::widgets::data_table::{Align, DataColumnSpec, TableData, TableDataBuilder, Value};

const PAIR_ROW: &str = "pair-";
const THREAD_ROW: &str = "thread-";

/// The pair columns, in the order every row holds its values.
pub fn pair_columns() -> Vec<DataColumnSpec> {
    let number =
        |id: &str, title: &str| DataColumnSpec::new(id, title).number().align(Align::Right);
    vec![
        DataColumnSpec::new("pair", "pair").pinned().sticky(),
        number("shared", "shared"),
        DataColumnSpec::new("items", "items").narrow(),
        number("notes", "notes"),
        number("reviews", "reviews"),
        number("handoffs", "handoffs"),
        number("branches", "branches"),
        DataColumnSpec::new("names", "names").narrow(),
        number("co_commits", "co-commits"),
    ]
}

/// The thread columns, in the order every row holds its values.
pub fn thread_columns() -> Vec<DataColumnSpec> {
    vec![
        DataColumnSpec::new("item", "item").pinned().sticky(),
        DataColumnSpec::new("title", "title").class("pk-dt-col-title"),
        DataColumnSpec::new("agents", "agents"),
        DataColumnSpec::new("steps", "steps")
            .number()
            .align(Align::Right),
        DataColumnSpec::new("stale", "stale holder"),
    ]
}

/// The id of the row of pair `at`.
pub fn pair_row(at: usize) -> String {
    format!("{PAIR_ROW}{at}")
}

/// The index of the pair a row id names.
pub fn pair_of_row(id: &str) -> Option<usize> {
    id.strip_prefix(PAIR_ROW)?.parse().ok()
}

/// The id of the row of thread `at`.
pub fn thread_row(at: usize) -> String {
    format!("{THREAD_ROW}{at}")
}

/// The index of the thread a row id names.
pub fn thread_of_row(id: &str) -> Option<usize> {
    id.strip_prefix(THREAD_ROW)?.parse().ok()
}

/// `items` joined by `, `: the first [`PAIR_LIST_CAP`], then `+N more`.
fn join_capped(out: &mut String, items: &[String]) {
    out.clear();
    for (at, item) in items.iter().take(PAIR_LIST_CAP).enumerate() {
        if at > 0 {
            out.push_str(", ");
        }
        out.push_str(item);
    }
    if items.len() > PAIR_LIST_CAP {
        let _ = write!(out, ", +{} more", items.len() - PAIR_LIST_CAP);
    }
}

/// One row per pair; row `n` is `collab.pairs[n]`. Notes are the total both
/// ways.
pub fn pair_table(collab: &Collaboration) -> TableData {
    let mut builder = TableDataBuilder::with_capacity(&pair_columns(), collab.pairs.len());
    let (mut id, mut name, mut items, mut names) =
        (String::new(), String::new(), String::new(), String::new());
    for (at, pair) in collab.pairs.iter().enumerate() {
        id.clear();
        let _ = write!(id, "{PAIR_ROW}{at}");
        name.clear();
        let _ = write!(name, "{} \u{2194} {}", pair.a, pair.b);
        join_capped(&mut items, &pair.shared_items);
        join_capped(&mut names, &pair.branches);
        builder.row(
            &id,
            [
                Value::Text(&name),
                Value::Number(pair.shared_items.len() as f64),
                Value::Text(&items),
                Value::Number((pair.notes_ab + pair.notes_ba) as f64),
                Value::Number(pair.reviews as f64),
                Value::Number(pair.handoffs.len() as f64),
                Value::Number(pair.branches.len() as f64),
                Value::Text(&names),
                Value::Number(pair.co_commits as f64),
            ],
        );
    }
    builder.build()
}

/// One row per thread; row `n` is `collab.threads[n]`. The stale cell names
/// the silent holder and since when, and is empty on a fresh thread.
pub fn thread_table(collab: &Collaboration) -> TableData {
    let mut builder = TableDataBuilder::with_capacity(&thread_columns(), collab.threads.len());
    let (mut id, mut agents, mut stale) = (String::new(), String::new(), String::new());
    for (at, thread) in collab.threads.iter().enumerate() {
        id.clear();
        let _ = write!(id, "{THREAD_ROW}{at}");
        join_capped(&mut agents, &thread.agents);
        stale.clear();
        if let Some(holder) = thread.stale_agent.as_deref().filter(|_| thread.stale) {
            stale.push_str(holder);
            if let Some(since) = thread.stale_since {
                let _ = write!(stale, " since {}", format_iso8601(since));
            }
        }
        builder.row(
            &id,
            [
                Value::Text(&thread.item_id),
                Value::from(thread.title.as_deref()),
                Value::Text(&agents),
                Value::Number(thread.steps.len() as f64),
                Value::Text(&stale),
            ],
        );
    }
    builder.build()
}
