//! COLLAB as tables: one row per pair and per thread, lists whole in their
//! cells and capped, rows named by index.

use super::super::*;
use super::fixtures::{at, ev};
use crate::widgets::trace::model::{Event, EventKind};

fn note(agent: &str, item: &str, hour: f64) -> Event {
    Event {
        agent: Some(agent.into()),
        ..ev(item, hour)
    }
}

/// `amy` and `bob` each note `items` items.
fn shared(items: usize) -> Collaboration {
    let events: Vec<Event> = (0..items)
        .flat_map(|n| {
            let item = format!("imp-{n}");
            [
                note("amy", &item, n as f64),
                note("bob", &item, n as f64 + 0.5),
            ]
        })
        .collect();
    Collaboration::build(&events, &current_items(&events), at(1_000.0))
}

fn column(id: &str, columns: &[crate::widgets::data_table::DataColumnSpec]) -> usize {
    columns.iter().position(|c| c.id == id).unwrap()
}

#[test]
fn a_pair_is_one_row_named_by_its_index_with_numbers_and_lists() {
    let collab = shared(5);

    let data = pair_table(&collab);
    let columns = pair_columns();

    assert_eq!(data.len(), 1);
    assert_eq!(data.id(0), pair_row(0));
    assert_eq!(pair_of_row(data.id(0)), Some(0));
    assert_eq!(data.text(0, column("pair", &columns)), "amy \u{2194} bob");
    assert_eq!(data.number(0, column("shared", &columns)), Some(5.0));
    assert_eq!(
        data.text(0, column("items", &columns)),
        "imp-0, imp-1, imp-2, imp-3, imp-4",
        "the cell holds the whole list: filters, tooltips and copy read it"
    );
    assert_eq!(data.number(0, column("notes", &columns)), Some(10.0));
}

#[test]
fn a_list_cell_stops_at_the_cap_and_counts_the_rest() {
    let collab = shared(PAIR_LIST_CAP + 30);

    let data = pair_table(&collab);
    let items = data.text(0, column("items", &pair_columns())).into_owned();

    assert!(
        items.ends_with(", +30 more"),
        "{}",
        &items[items.len() - 20..]
    );
    assert_eq!(items.matches(", ").count(), PAIR_LIST_CAP);
    assert_eq!(
        data.number(0, column("shared", &pair_columns())),
        Some((PAIR_LIST_CAP + 30) as f64),
        "the count column is the true count"
    );
}

#[test]
fn row_ids_lead_back_to_the_pair_or_the_thread_and_nothing_else() {
    assert_eq!(pair_of_row(&pair_row(7)), Some(7));
    assert_eq!(thread_of_row(&thread_row(12)), Some(12));
    assert_eq!(pair_of_row(&thread_row(7)), None);
    assert_eq!(thread_of_row("pair-1"), None);
    assert_eq!(pair_of_row("pair-x"), None);
}

#[test]
fn a_thread_is_one_row_and_a_stale_holder_names_since_when() {
    let events = vec![
        Event::new("c1", at(0.0), EventKind::ItemClaimed)
            .agent("amy")
            .item("imp-9"),
        Event::new("s1", at(1.0), EventKind::ItemStatus)
            .agent("amy")
            .item("imp-9")
            .status("claimed")
            .title("Mind the gap"),
        note("bob", "imp-9", 2.0),
        note("amy", "imp-1", 3.0),
        note("bob", "imp-1", 4.0),
    ];
    let collab = Collaboration::build(&events, &current_items(&events), at(24.0 * 30.0));

    let data = thread_table(&collab);
    let columns = thread_columns();

    assert_eq!(data.len(), collab.threads.len());
    let stale = (0..data.len())
        .find(|&row| data.text(row, column("item", &columns)) == "imp-9")
        .unwrap();
    assert_eq!(
        collab.threads[stale].stale_agent.as_deref(),
        Some("amy"),
        "the fixture holds a stale claim"
    );
    let cell = data.text(stale, column("stale", &columns)).into_owned();
    assert!(cell.starts_with("amy since "), "{cell}");
    assert_eq!(data.text(stale, column("title", &columns)), "Mind the gap");
    let fresh = (0..data.len()).find(|&row| row != stale).unwrap();
    assert_eq!(data.text(fresh, column("stale", &columns)), "");
    assert_eq!(data.text(fresh, column("agents", &columns)), "amy, bob");
}
