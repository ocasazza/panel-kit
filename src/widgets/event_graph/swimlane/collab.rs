//! The COLLAB body: who worked with whom, as a table of pairs, and the items
//! two or more agents touched, as a table of threads. Both are the one
//! `DataTable`: a sticky header, a virtual window, sort, filter, density, copy
//! and CSV. A row opens its pair (or its item) in the inspector.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::widgets::data_table::{Selectable, SortDir};
use panel_kit_core::widgets::event_graph::swimlane::{
    pair_columns, pair_of_row, pair_row, pair_table, thread_columns, thread_of_row, thread_table,
    Collaboration, Node, Pair, Thread,
};

use super::shared::Slot;
use crate::widgets::data_table::{Control, DataTable, SortSpec, TableData, TableDefaults};

/// Items or branches a cell names before its `+N` tail.
const CELL_NAMES: usize = 3;
/// Height at which a table scrolls instead of growing, in pixels.
const MAX_HEIGHT: f64 = 360.0;

/// The first `names` of a list, then a dim `+N`; the whole list is the
/// cell's tooltip.
fn tail(items: &[String], names: usize) -> Element {
    if items.is_empty() {
        return rsx! {
            span { class: "pk-ag-faint", "0" }
        };
    }
    let shown = items
        .iter()
        .take(names)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    let more = items.len().saturating_sub(names);
    rsx! {
        "{shown}"
        if more > 0 {
            span { class: "pk-ag-faint pk-ag-tail", " +{more}" }
        }
    }
}

fn pair_cell(pair: &Pair, column: usize) -> Element {
    match column {
        0 => rsx! {
            b { class: "pk-ag-id", "{pair.a}" }
            span { class: "pk-ag-tee", " \u{2194} " }
            b { class: "pk-ag-id", "{pair.b}" }
        },
        1 => rsx! { "{pair.shared_items.len()}" },
        2 => tail(&pair.shared_items, CELL_NAMES),
        3 => rsx! {
            span { class: "pk-ag-a", "{pair.notes_ab}" }
            span { class: "pk-ag-faint", " \u{2192} " }
            span { class: "pk-ag-b", "{pair.notes_ba}" }
        },
        4 => rsx! { "{pair.reviews}" },
        5 => rsx! { "{pair.handoffs.len()}" },
        6 => rsx! { "{pair.branches.len()}" },
        7 => tail(&pair.branches, 2),
        _ => rsx! { "{pair.co_commits}" },
    }
}

fn thread_cell(thread: &Thread, column: usize) -> Element {
    match column {
        0 => rsx! {
            b { class: "pk-ag-id", "{thread.item_id}" }
        },
        1 => rsx! {
            if let Some(title) = thread.title.as_deref() {
                "{title}"
            }
        },
        2 => tail(&thread.agents, CELL_NAMES),
        3 => rsx! { "{thread.steps.len()}" },
        _ => match thread.stale_agent.as_deref().filter(|_| thread.stale) {
            Some(holder) => rsx! {
                span { class: "pk-ag-ins-dot pk-ag-warn", aria_hidden: "true" }
                " {holder}"
            },
            None => rsx! {
                span { class: "pk-ag-faint", "\u{2014}" }
            },
        },
    }
}

/// A table's data, rebuilt only when the collaboration it came from changes.
type DataSlot = Rc<RefCell<Slot<Rc<Collaboration>, TableData>>>;

/// The COLLAB mode body.
#[component]
pub(super) fn CollabBody(
    id: String,
    collab: Rc<Collaboration>,
    open: Option<(String, String)>,
    on_open: EventHandler<Node>,
) -> Element {
    let pairs: DataSlot = use_hook(|| Rc::new(RefCell::new(Slot::default())));
    let threads: DataSlot = use_hook(|| Rc::new(RefCell::new(Slot::default())));
    let pair_rows = pairs
        .borrow_mut()
        .get(collab.clone(), || pair_table(&collab));
    let thread_rows = threads
        .borrow_mut()
        .get(collab.clone(), || thread_table(&collab));
    let toolbar = vec![
        Control::Quick,
        Control::Filters,
        Control::Columns,
        Control::Density,
        Control::Copy,
        Control::Export,
        Control::Tally,
    ];
    let open_id = open.as_ref().and_then(|(a, b)| {
        collab
            .pairs
            .iter()
            .position(|pair| pair.a == *a && pair.b == *b)
            .map(pair_row)
    });

    rsx! {
        section { id: "{id}-collaboration", class: "pk-ag-collab", aria_label: "collaboration",
            if collab.pairs.is_empty() {
                p { id: "{id}-collaboration-empty", class: "pk-ag-empty",
                    "no two agents worked the same item"
                }
            } else {
                DataTable {
                    id: "{id}-pairs",
                    columns: pair_columns(),
                    data: pair_rows,
                    cell: {
                        let collab = collab.clone();
                        move |(row, column): (usize, usize)| pair_cell(&collab.pairs[row], column)
                    },
                    defaults: TableDefaults {
                        sort: vec![SortSpec::new("shared", SortDir::Desc)],
                        selectable: Selectable::Single,
                        ..TableDefaults::default()
                    },
                    label: "collaboration pairs",
                    placeholder: "filter by agent",
                    ribbon: false,
                    windowed: true,
                    toolbar: toolbar.clone(),
                    max_height: MAX_HEIGHT,
                    selected: open_id.map(|row| vec![row]),
                    on_row: {
                        let collab = collab.clone();
                        move |row: String| {
                            let pair = pair_of_row(&row).and_then(|at| collab.pairs.get(at));
                            if let Some(pair) = pair {
                                on_open.call(Node::Pair(pair.a.clone(), pair.b.clone()));
                            }
                        }
                    },
                }
            }
            if !collab.threads.is_empty() {
                h3 { class: "pk-ag-threads-head", "threads" }
                DataTable {
                    id: "{id}-threads",
                    columns: thread_columns(),
                    data: thread_rows,
                    cell: {
                        let collab = collab.clone();
                        move |(row, column): (usize, usize)| thread_cell(&collab.threads[row], column)
                    },
                    defaults: TableDefaults {
                        selectable: Selectable::Single,
                        ..TableDefaults::default()
                    },
                    label: "collaboration threads",
                    placeholder: "filter by item or agent",
                    ribbon: false,
                    windowed: true,
                    toolbar,
                    max_height: MAX_HEIGHT,
                    on_row: {
                        let collab = collab.clone();
                        move |row: String| {
                            let thread = thread_of_row(&row).and_then(|at| collab.threads.get(at));
                            if let Some(thread) = thread {
                                on_open.call(Node::Item(thread.item_id.clone()));
                            }
                        }
                    },
                }
            }
        }
    }
}
