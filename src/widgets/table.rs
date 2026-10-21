//! The semantic table painted by the one table component.
//!
//! A [`TableModel`](panel_kit_core::widgets::table::TableModel) is static data:
//! cells, columns, rows. The interactive table is
//! [`data_table`](super::data_table); the model converts itself to its rows by
//! column kind ([`TableModel::data`]) and this module paints each cell with the
//! same component, so a spec-authored table and a host-built one cannot drift.
//!
//! A table is a component over its model: while the model stays equal, the
//! converted rows stay the same [`TableData`], so the table's order and view
//! caches keep hitting however often the host re-renders.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::widgets::table::{TableCell, TableModel, TableView};

use crate::widgets::data_table::{DataColumnSpec, DataTable, TableData, TableDefaults};

#[cfg(test)]
thread_local! {
    /// Models converted and table components rendered on this thread: what
    /// the memo tests count.
    static CONVERSIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RENDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A model converted for the table component, and the model it came from.
struct Converted {
    model: Rc<TableModel>,
    columns: Vec<DataColumnSpec>,
    data: TableData,
}

impl Converted {
    fn new(model: Rc<TableModel>) -> Self {
        #[cfg(test)]
        CONVERSIONS.with(|count| count.set(count.get() + 1));
        let columns = model.data_columns();
        let data = model.data();
        Self { model, columns, data }
    }
}

/// The last conversion, kept while the model it came from stays equal.
type Memo = Rc<RefCell<Option<Rc<Converted>>>>;

/// `model` converted: the last conversion when it was of an equal model, else
/// a fresh one, which becomes the last.
fn convert(memo: &Memo, model: &Rc<TableModel>) -> Rc<Converted> {
    let mut last = memo.borrow_mut();
    match last.as_ref() {
        Some(converted) if *converted.model == **model => Rc::clone(converted),
        _ => {
            let converted = Rc::new(Converted::new(Rc::clone(model)));
            *last = Some(Rc::clone(&converted));
            converted
        }
    }
}

fn paint(cell: &TableCell) -> Element {
    match cell {
        TableCell::Text(text) => rsx! { "{text}" },
        TableCell::Status {
            label,
            color: (r, g, b),
        } => rsx! {
            span { class: "pk-table-status", role: "status", aria_label: "{label}", style: "--status-c:rgb({r},{g},{b});",
                span { aria_hidden: "true", "\u{25cf}" }
                " {label}"
            }
        },
        TableCell::Meter { ratio, text, color } => {
            let percent = (ratio.clamp(0.0, 1.0) * 100.0).round() as u32;
            let style = color
                .map(|(r, g, b)| format!("--meter-c:rgb({r},{g},{b});"))
                .unwrap_or_default();
            rsx! {
                span {
                    class: "pk-table-meter",
                    role: "meter",
                    style: "{style}",
                    aria_valuemin: "0",
                    aria_valuemax: "100",
                    aria_valuenow: "{percent}",
                    aria_label: "{text}",
                    span { class: "pk-table-meter-fill", style: "width:{percent}%;" }
                    span { class: "pk-table-meter-text", "{text}" }
                }
            }
        }
    }
}

/// A model painted by the table component. It re-renders only when its props
/// change, and converts only when the model does.
#[component]
fn ModelTable(
    /// DOM id; row, header and control ids derive from it.
    id: String,
    /// What the table shows.
    model: Rc<TableModel>,
    /// `localStorage` key for the durable state; none keeps it for the mount.
    #[props(default)]
    storage_key: Option<String>,
    /// Receives a clicked row's id.
    #[props(default)]
    on_row: Option<EventHandler<String>>,
) -> Element {
    #[cfg(test)]
    RENDERS.with(|count| count.set(count.get() + 1));
    let memo: Memo = use_hook(|| Rc::new(RefCell::new(None)));
    let converted = convert(&memo, &model);
    let columns = converted.columns.clone();
    let data = converted.data.clone();
    rsx! {
        DataTable {
            id,
            columns,
            data,
            cell: move |(row, column): (usize, usize)| {
                converted
                    .model
                    .rows
                    .get(row)
                    .and_then(|row| row.cells.get(column))
                    .map(paint)
                    .unwrap_or_else(|| rsx! {})
            },
            defaults: TableDefaults::default(),
            storage_key,
            on_row,
        }
    }
}

/// The DOM id of the table named `name`.
fn dom_id(name: &str) -> String {
    format!("pk-table-{name}")
}

/// Paint a borrowed table view with the table component. `name` is the
/// table's id on the page, which row, header and control ids derive from: two
/// tables painted together need two names. State lasts for the mount; a table
/// that should keep its sort and filters is a [`bound_table`]. `on_row`
/// receives a clicked row's id.
pub fn table(name: &str, view: TableView<'_>, on_row: Option<EventHandler<String>>) -> Element {
    let model = TableModel {
        columns: view.columns.to_vec(),
        rows: view.rows.iter().map(|row| (*row).clone()).collect(),
    };
    rsx! { ModelTable { id: dom_id(name), model: Rc::new(model), on_row } }
}

/// Paint the table a host binding resolves to. The binding names the table on
/// the page and keys its durable state (sort, filters, column layout) under
/// `pk-dt:{binding}`, so they survive a regime switch and a reload. `on_row`
/// receives a clicked row's id.
pub fn bound_table(
    binding: &str,
    model: &TableModel,
    on_row: Option<EventHandler<String>>,
) -> Element {
    rsx! {
        ModelTable {
            id: dom_id(binding),
            model: Rc::new(model.clone()),
            storage_key: format!("pk-dt:{binding}"),
            on_row,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use panel_kit_core::widgets::data_table::ColumnKind;
    use panel_kit_core::widgets::table::{ColumnWidth, TableColumn, TableRow, TextAlign};

    use super::*;

    fn model(rows: usize) -> TableModel {
        TableModel {
            columns: vec![TableColumn {
                key: "n".into(),
                title: "N".into(),
                width: ColumnWidth::Flex { weight: 1 },
                align: TextAlign::Left,
                kind: ColumnKind::Number,
            }],
            rows: (0..rows)
                .map(|at| TableRow {
                    id: format!("r{at}"),
                    tags: Vec::new(),
                    cells: vec![TableCell::Text(format!("{at}.5"))],
                })
                .collect(),
        }
    }

    fn conversions() -> usize {
        CONVERSIONS.with(Cell::get)
    }

    #[test]
    fn an_equal_model_reuses_the_conversion_and_a_changed_one_replaces_it() {
        let before = conversions();
        let memo = Memo::default();
        let first = convert(&memo, &Rc::new(model(3)));
        let again = convert(&memo, &Rc::new(model(3)));
        assert!(Rc::ptr_eq(&first, &again));
        assert!(first.data == again.data, "the rows are the same TableData");
        assert_eq!(conversions() - before, 1);

        let grown = convert(&memo, &Rc::new(model(4)));
        assert!(!Rc::ptr_eq(&first, &grown));
        assert!(first.data != grown.data);
        assert_eq!(grown.data.len(), 4);
        assert_eq!(conversions() - before, 2);
    }

    /// The handles that move a probe, and whether its host hands the table a
    /// new handler on every render.
    #[derive(Clone, Default)]
    struct Knobs {
        ticks: Rc<Cell<Option<Signal<usize>>>>,
        rows: Rc<Cell<Option<Signal<usize>>>>,
        new_handler: bool,
    }

    /// A host that re-renders on `ticks` while handing the table a fresh clone
    /// of the model, which has as many rows as the `rows` signal says.
    #[component]
    fn Host() -> Element {
        let knobs = use_context::<Knobs>();
        let ticks = use_signal(|| 0usize);
        let rows = use_signal(|| 3usize);
        knobs.ticks.set(Some(ticks));
        knobs.rows.set(Some(rows));
        let _render_again_on_every_tick = ticks();
        let on_row = knobs.new_handler.then(|| EventHandler::new(|_: String| {}));
        rsx! { ModelTable { id: dom_id("t"), model: Rc::new(model(rows())), on_row } }
    }

    /// `(table renders, conversions)` after `ticks` equal-model re-renders of a
    /// host, and the conversions after one more render over a model with a row more.
    fn counts_over_renders(new_handler: bool, ticks: usize) -> ((usize, usize), usize) {
        let (renders_before, conversions_before) = (RENDERS.with(Cell::get), conversions());
        let knobs = Knobs { new_handler, ..Knobs::default() };
        let mut dom = VirtualDom::new(Host);
        dom.provide_root_context(knobs.clone());
        dom.rebuild_to_vec();
        let (mut tick, mut rows) = (knobs.ticks.get().unwrap(), knobs.rows.get().unwrap());
        for at in 1..=ticks {
            dom.in_runtime(|| tick.set(at));
            dom.render_immediate_to_vec();
        }
        let equal = (RENDERS.with(Cell::get) - renders_before, conversions() - conversions_before);

        dom.in_runtime(|| rows.set(4));
        dom.render_immediate_to_vec();
        assert!(dioxus_ssr::render(&dom).contains("pk-table-t-r3"), "the new row is painted");
        (equal, conversions() - conversions_before)
    }

    #[test]
    fn a_host_that_re_renders_does_not_rebuild_the_table_data() {
        // Equal props: the table component renders once and is then skipped.
        assert_eq!(counts_over_renders(false, 5), ((1, 1), 2));
    }

    #[test]
    fn a_table_that_re_renders_anyway_keeps_its_table_data() {
        // A new handler each render: the component renders again, the memo holds.
        assert_eq!(counts_over_renders(true, 5), ((6, 1), 2));
    }

    #[test]
    fn a_bound_table_is_named_and_keyed_by_its_binding() {
        let html = dioxus_ssr::render_element(rsx! {
            {bound_table("agentic.status", &model(1), None)}
            {bound_table("agentic.physics", &model(1), None)}
        });
        assert!(html.contains("id=\"pk-table-agentic.status\""), "{html}");
        assert!(html.contains("id=\"pk-table-agentic.physics\""), "{html}");
        assert_eq!(html.matches("id=\"pk-table-agentic.status\"").count(), 1, "{html}");
    }
}
