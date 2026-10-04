//! Web painters shared by topos hosts: one global section into panel content,
//! and the form for a section edit offered on a table row.
//!
//! Hosts own the state (which section, which row, where a write goes); these
//! functions only paint it and report interaction through handlers.

use std::collections::BTreeMap;

use dioxus::prelude::*;
use panel_kit_core::badge::BadgeAction;
use panel_kit_core::widgets::charts::{five_num, BoxItemView, SeriesView};
use panel_kit_core::widgets::table::TableView;
use panel_kit_core::widgets::{ContentSpec, ContentView, ScrollPolicy};
use panel_kit_grammar::{FieldType, Object, Section, SectionEdit};

/// Row interaction for a table whose rows offer edits.
pub struct RowSelect {
    /// Row painted as selected.
    pub selected: Option<usize>,
    /// Called with the clicked row's index.
    pub on_row: EventHandler<usize>,
}

fn noop_badge_action() -> EventHandler<BadgeAction> {
    EventHandler::new(|_: BadgeAction| {})
}

/// Paint one section with the panel's authored content spec (text keeps its
/// scroll policy, a series its unit). `rows` makes a table's rows selectable;
/// `on_badge` receives badge actions.
pub fn paint(
    section: &Section,
    content: &ContentSpec,
    rows: Option<RowSelect>,
    on_badge: Option<EventHandler<BadgeAction>>,
) -> Element {
    let badge = on_badge.unwrap_or_else(noop_badge_action);
    match section {
        Section::Table(table) => {
            let view = TableView { columns: &table.columns, rows: &table.rows };
            match rows {
                Some(rows) => panel_kit::widgets::table::table(view, rows.selected, Some(rows.on_row), false),
                None => panel_kit::widgets::content_view(ContentView::Table(view), 0, badge),
            }
        }
        Section::Text(text) => {
            let scroll = match content {
                ContentSpec::Text { scroll, .. } => *scroll,
                _ => ScrollPolicy::Auto,
            };
            panel_kit::widgets::content_view(ContentView::Text { text: &text.text, scroll }, 0, badge)
        }
        Section::TimeSeries(series) => {
            let views: Vec<SeriesView<'_>> =
                series.iter().map(|model| SeriesView { name: &model.name, points: &model.points }).collect();
            let unit = match content {
                ContentSpec::TimeSeries { unit, .. } => unit.as_str(),
                _ => "",
            };
            panel_kit::widgets::content_view(ContentView::TimeSeries { series: &views, unit }, 0, badge)
        }
        Section::Boxplot(items) => {
            let views: Vec<BoxItemView<'_>> = items
                .iter()
                .filter_map(|item| {
                    five_num(&item.samples).map(|summary| BoxItemView { label: &item.label, summary, color: item.color })
                })
                .collect();
            panel_kit::widgets::content_view(ContentView::Boxplot(&views), 0, badge)
        }
        Section::Gauges(gauges) => panel_kit::widgets::content_view(ContentView::Gauges(gauges), 0, badge),
        Section::Flamegraph(spans) => panel_kit::widgets::content_view(ContentView::Flamegraph(spans), 0, badge),
        Section::Badges(specs) => panel_kit::widgets::content_view(ContentView::Badges(specs), 0, badge),
        Section::Meter(meter) => panel_kit::widgets::content_view(ContentView::Meter(meter), 0, badge),
        Section::Status(status) => panel_kit::widgets::content_view(ContentView::Status(status), 0, badge),
    }
}

/// An edit being filled in on one object of one binding's table.
#[derive(Clone, Debug, PartialEq)]
pub struct EditDraft {
    /// Table binding the row belongs to.
    pub binding: String,
    /// Object id behind the row.
    pub object: String,
    /// Chosen edit id.
    pub edit: String,
    /// Field name to entered value.
    pub values: BTreeMap<String, String>,
}

impl EditDraft {
    /// A draft of `edit` on `object`, prefilled with the object's current values.
    pub fn open(binding: &str, object: &Object, edit_id: &str, edit: &SectionEdit) -> Self {
        let values = edit
            .set
            .keys()
            .map(|field| (field.clone(), object.fields.get(field).cloned().unwrap_or_default()))
            .collect();
        Self { binding: binding.to_owned(), object: object.id.clone(), edit: edit_id.to_owned(), values }
    }
}

/// One input per field, typed by its field type; edits update `draft`.
///
/// Text fields are uncontrolled (`initial_value`): writing the draft back into
/// `value` on every input races fast typing and resets text and caret. `scope`
/// keys the fields to one object and edit, so another draft gets fresh inputs.
fn field_input(mut draft: Signal<Option<EditDraft>>, scope: &str, field: &str, ty: &FieldType, value: &str) -> Element {
    let name = field.to_owned();
    let set = move |event: FormEvent| {
        let value = event.value();
        draft.with_mut(|draft| {
            if let Some(draft) = draft {
                draft.values.insert(name.clone(), value);
            }
        });
    };
    match ty {
        // Selection is per option: a select's `value` matches nothing while the
        // field is unset, and the browser would then show the first word.
        FieldType::Enum { values } => rsx! {
            label { key: "{scope}/{field}", "{field}"
                select { onchange: set,
                    option { value: "", disabled: true, selected: !values.iter().any(|v| v == value), "choose" }
                    for v in values.iter() { option { key: "{v}", value: "{v}", selected: v == value, "{v}" } }
                }
            }
        },
        FieldType::Int { min, max } => rsx! {
            label { key: "{scope}/{field}", "{field} ({min}–{max})"
                input { r#type: "number", min: "{min}", max: "{max}", initial_value: "{value}", oninput: set }
            }
        },
        FieldType::Text { max } => rsx! {
            label { key: "{scope}/{field}", "{field} (≤ {max} characters)"
                textarea { maxlength: "{max}", initial_value: "{value}", oninput: set }
            }
        },
        FieldType::Lines { max } => rsx! {
            label { key: "{scope}/{field}", "{field} (one per line, ≤ {max})"
                textarea { initial_value: "{value}", oninput: set }
            }
        },
    }
}

/// CSS for [`edit_form`]; hosts include it once.
pub const EDIT_FORM_CSS: &str = "
.pk-edit-form { position: fixed; right: .75rem; top: 3.25rem; width: min(26rem, 90vw); max-height: 80vh;
  overflow: auto; z-index: 50; background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 4px; padding: .6rem .75rem; font-size: .75rem; display: flex; flex-direction: column; gap: .5rem; }
.pk-edit-form h2 { font-size: .74rem; margin: 0; text-transform: uppercase; letter-spacing: .08em; }
.pk-edit-form .object { color: var(--dim); word-break: break-all; }
.pk-edit-form .edits, .pk-edit-form .actions { display: flex; gap: .35rem; flex-wrap: wrap; }
.pk-edit-form button { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.pk-edit-form button.active { background: var(--fg); color: var(--bg); }
.pk-edit-form label { display: flex; flex-direction: column; gap: .2rem; color: var(--dim); }
.pk-edit-form input, .pk-edit-form select, .pk-edit-form textarea { background: var(--bg); color: var(--fg);
  border: 1px solid var(--line2); border-radius: 3px; padding: .25rem; font: inherit; }
.pk-edit-form textarea { min-height: 4.5rem; resize: vertical; }
";

/// The form for `draft`: the row's offered edits as `(id, label)` (choosing
/// one calls `on_choose`), the chosen edit's fields, and write/close.
pub fn edit_form(
    mut draft: Signal<Option<EditDraft>>,
    title: &str,
    offered: &[(String, String)],
    edit: &SectionEdit,
    on_choose: EventHandler<String>,
    on_submit: EventHandler<EditDraft>,
) -> Element {
    let Some(open) = draft.read().clone() else { return rsx! {} };
    let scope = format!("{}/{}", open.object, open.edit);
    let fields: Vec<Element> = edit
        .set
        .iter()
        .map(|(field, ty)| field_input(draft, &scope, field, ty, open.values.get(field).map_or("", String::as_str)))
        .collect();
    let choices: Vec<Element> = offered
        .iter()
        .map(|(id, label)| {
            let chosen = id.clone();
            let active = *id == open.edit;
            rsx! {
                button {
                    key: "{id}",
                    class: if active { "active" },
                    aria_pressed: active,
                    onclick: move |_| on_choose.call(chosen.clone()),
                    "{label}"
                }
            }
        })
        .collect();
    rsx! {
        div { class: "pk-edit-form", role: "dialog", aria_label: "edit {title}",
            h2 { "{title}" }
            div { class: "object", "{open.object}" }
            div { class: "edits", {choices.into_iter()} }
            {fields.into_iter()}
            div { class: "actions",
                button { class: "active", onclick: move |_| on_submit.call(open.clone()), "write" }
                button { onclick: move |_| draft.set(None), "close" }
            }
        }
    }
}
