//! Dioxus painters for form controls.
//!
//! [`Form`] / [`form`] paint a whole [`FormModel`]: core
//! ([`panel_kit_core::widgets::forms`]) owns the fields, values, focus, and
//! the select popup; this adapter only maps the model to DOM and translates
//! DOM events into [`FormInput`]s.
//!
//! [`Slider`], [`IntRow`], [`CheckRow`], and [`TextRow`] are the standalone
//! label + control rows ported from jump-cannon's layout-panel form rows
//! (`panels/layout.rs`); they share the control primitives (`.pk-input`,
//! `.pk-range`, `.pk-check`) and core's value parsing and clamping.

use dioxus::prelude::*;
use panel_kit_core::widgets::forms as forms_core;

pub use panel_kit_core::widgets::forms::{
    ChoiceModel, ControlModel, FieldModel, FieldValue, FormAction, FormHit, FormInput, FormKey,
    FormModel, FormState, TextEntry,
};

/// Paint a whole form and drive it through core's reducer.
///
/// The host owns `model` and `state`; every DOM event becomes a
/// [`FormInput`] applied with [`forms_core::reduce`], and each resulting
/// [`FormAction`] is reported through `on_action`.
#[component]
pub fn Form(
    /// Fields and values, owned by the host.
    model: Signal<FormModel>,
    /// Focus and popup state, owned by the host.
    state: Signal<FormState>,
    /// Receives every value change and button press.
    #[props(default)]
    on_action: EventHandler<FormAction>,
    /// Prefix of every control's DOM id; unique per form on the page.
    #[props(default = "pk-form".to_string())]
    id: String,
) -> Element {
    let mut model = model;
    let mut state = state;
    let on_input = use_callback(move |input: FormInput| {
        let actions = forms_core::reduce(&mut model.write(), &mut state.write(), input);
        for action in actions {
            on_action.call(action);
        }
    });
    // Bound so the signal guards drop before `model` and `state` do.
    let painted = form(&id, &model.read(), &state.read(), Some(on_input));
    painted
}

/// Paint `model` with `state` as DOM.
///
/// `id` prefixes every control's DOM id. With no `on_input` the form is
/// painted `inert`: visible, but not focusable or operable.
pub fn form(
    id: &str,
    model: &FormModel,
    state: &FormState,
    on_input: Option<EventHandler<FormInput>>,
) -> Element {
    rsx! {
        div {
            class: "pk-widget pk-form",
            role: "group",
            "inert": if on_input.is_none() { "true" },
            for (index , field) in model.fields.iter().enumerate() {
                {field_row(id, index, field, state, on_input)}
            }
        }
    }
}

/// One `key | control note` row.
fn field_row(
    form_id: &str,
    index: usize,
    field: &FieldModel,
    state: &FormState,
    on_input: Option<EventHandler<FormInput>>,
) -> Element {
    let control_id = format!("{form_id}-{}", field.id);
    let label_id = format!("{control_id}-label");
    let note_id = format!("{control_id}-note");
    // A radio group and a button have no single labelable element.
    let labels_control = !matches!(
        field.control,
        ControlModel::Radio { .. } | ControlModel::Button { .. }
    );
    let kind = field.control.kind();
    let label = field.label.clone();
    let note = field.note.clone();

    rsx! {
        div {
            class: "pk-field",
            "data-control": "{kind}",
            "data-disabled": if field.disabled { "true" } else { "false" },
            if labels_control {
                label { class: "pk-field-key", id: "{label_id}", r#for: "{control_id}", "{label}" }
            } else {
                span { class: "pk-field-key", id: "{label_id}", "{label}" }
            }
            div { class: "pk-field-control",
                {control(&control_id, &label_id, &note_id, index, field, state, on_input)}
                if !note.is_empty() {
                    span { class: "pk-field-note", id: "{note_id}", "{note}" }
                }
            }
        }
    }
}

/// The DOM `type` of a single-line text control. Date and time are typed
/// text: a native picker cannot be themed or drawn in cells.
fn input_type(entry: TextEntry) -> &'static str {
    match entry {
        TextEntry::Password => "password",
        TextEntry::Search => "search",
        TextEntry::Email => "email",
        TextEntry::Url => "url",
        TextEntry::Tel => "tel",
        TextEntry::Plain | TextEntry::Date | TextEntry::Time => "text",
    }
}

/// The virtual-keyboard hint for an entry kind.
fn input_mode(entry: TextEntry) -> &'static str {
    match entry {
        TextEntry::Date | TextEntry::Time => "numeric",
        TextEntry::Tel => "tel",
        TextEntry::Email => "email",
        TextEntry::Url => "url",
        TextEntry::Search => "search",
        TextEntry::Plain | TextEntry::Password => "text",
    }
}

/// The form key a DOM key means on the select trigger.
fn select_key(key: &Key) -> Option<FormKey> {
    match key {
        Key::Enter => Some(FormKey::Enter),
        Key::Escape => Some(FormKey::Escape),
        Key::ArrowUp => Some(FormKey::Up),
        Key::ArrowDown => Some(FormKey::Down),
        Key::Character(c) if c == " " => Some(FormKey::Char(' ')),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn control(
    control_id: &str,
    label_id: &str,
    note_id: &str,
    index: usize,
    field: &FieldModel,
    state: &FormState,
    on_input: Option<EventHandler<FormInput>>,
) -> Element {
    let emit = move |input: FormInput| {
        if let Some(handler) = on_input {
            handler.call(input);
        }
    };
    let disabled = field.disabled;
    let described_by = (!field.note.is_empty()).then(|| note_id.to_owned());

    match &field.control {
        ControlModel::Text { value, placeholder, entry } => {
            let entry = *entry;
            let max_length = entry.max_len().map(|len| len.to_string());
            rsx! {
                input {
                    class: "pk-input",
                    id: "{control_id}",
                    r#type: input_type(entry),
                    inputmode: input_mode(entry),
                    maxlength: max_length,
                    value: "{value}",
                    placeholder: (!placeholder.is_empty()).then(|| placeholder.clone()),
                    disabled,
                    aria_describedby: described_by,
                    onfocus: move |_| emit(FormInput::Focus(index)),
                    onblur: move |_| emit(FormInput::Blur),
                    // A character the entry kind refuses never reaches the field.
                    onkeydown: move |event: KeyboardEvent| {
                        let modifiers = event.modifiers();
                        let shortcut = modifiers.ctrl() || modifiers.meta() || modifiers.alt();
                        if let Key::Character(text) = event.key() {
                            if !shortcut && !text.chars().all(|c| entry.accepts(c)) {
                                event.prevent_default();
                            }
                        }
                    },
                    oninput: move |event: FormEvent| {
                        emit(FormInput::SetText { field: index, value: event.value() })
                    },
                }
            }
        }
        ControlModel::TextArea { value, placeholder, rows } => rsx! {
            textarea {
                class: "pk-input",
                id: "{control_id}",
                rows: "{rows}",
                value: "{value}",
                placeholder: (!placeholder.is_empty()).then(|| placeholder.clone()),
                disabled,
                aria_describedby: described_by,
                onfocus: move |_| emit(FormInput::Focus(index)),
                onblur: move |_| emit(FormInput::Blur),
                oninput: move |event: FormEvent| {
                    emit(FormInput::SetText { field: index, value: event.value() })
                },
            }
        },
        ControlModel::Number { value, min, max, step } => {
            let label = field.label.clone();
            rsx! {
                span { class: "pk-number",
                    input {
                        class: "pk-input",
                        id: "{control_id}",
                        r#type: "number",
                        min: "{min}",
                        max: "{max}",
                        step: "{step}",
                        value: "{forms_core::fmt_f(*value)}",
                        disabled,
                        aria_describedby: described_by,
                        onfocus: move |_| emit(FormInput::Focus(index)),
                        onblur: move |_| emit(FormInput::Blur),
                        onchange: move |event: FormEvent| {
                            if let Ok(value) = event.value().trim().parse::<f64>() {
                                emit(FormInput::SetNumber { field: index, value });
                            }
                        },
                    }
                    button {
                        class: "pk-step",
                        r#type: "button",
                        tabindex: "-1",
                        disabled,
                        aria_label: "decrease {label}",
                        onclick: move |_| {
                            emit(FormInput::Press(FormHit::Stepper { field: index, delta: -1 }))
                        },
                        "\u{2212}"
                    }
                    button {
                        class: "pk-step",
                        r#type: "button",
                        tabindex: "-1",
                        disabled,
                        aria_label: "increase {label}",
                        onclick: move |_| {
                            emit(FormInput::Press(FormHit::Stepper { field: index, delta: 1 }))
                        },
                        "+"
                    }
                }
            }
        }
        ControlModel::Slider { value, min, max, step } => {
            let fill = field.control.fraction().unwrap_or(0.0) * 100.0;
            rsx! {
                span { class: "pk-slider",
                    input {
                        class: "pk-range",
                        id: "{control_id}",
                        r#type: "range",
                        min: "{min}",
                        max: "{max}",
                        step: "{step}",
                        value: "{value}",
                        style: "--pk-fill:{fill}%",
                        disabled,
                        aria_describedby: described_by,
                        onfocus: move |_| emit(FormInput::Focus(index)),
                        onblur: move |_| emit(FormInput::Blur),
                        oninput: move |event: FormEvent| {
                            if let Ok(value) = event.value().parse::<f64>() {
                                emit(FormInput::SetNumber { field: index, value });
                            }
                        },
                    }
                    output { class: "pk-slider-value", "for": "{control_id}", "{forms_core::fmt_f(*value)}" }
                }
            }
        }
        ControlModel::Checkbox { checked } => rsx! {
            input {
                class: "pk-check",
                id: "{control_id}",
                r#type: "checkbox",
                checked: *checked,
                disabled,
                aria_describedby: described_by,
                onfocus: move |_| emit(FormInput::Focus(index)),
                onblur: move |_| emit(FormInput::Blur),
                onchange: move |_| emit(FormInput::Press(FormHit::Control(index))),
            }
        },
        ControlModel::Switch { on } => rsx! {
            button {
                class: "pk-switch",
                id: "{control_id}",
                r#type: "button",
                role: "switch",
                aria_checked: if *on { "true" } else { "false" },
                disabled,
                aria_describedby: described_by,
                onfocus: move |_| emit(FormInput::Focus(index)),
                onblur: move |_| emit(FormInput::Blur),
                onclick: move |_| emit(FormInput::Press(FormHit::Control(index))),
            }
            span { class: "pk-switch-state", aria_hidden: "true",
                if *on { "on" } else { "off" }
            }
        },
        ControlModel::Radio { value, options } => rsx! {
            div {
                class: "pk-radio-group",
                id: "{control_id}",
                role: "radiogroup",
                aria_labelledby: "{label_id}",
                aria_describedby: described_by,
                for (option , choice) in options.iter().enumerate() {
                    label { class: "pk-choice", key: "{choice.value}",
                        input {
                            class: "pk-radio",
                            r#type: "radio",
                            name: "{control_id}",
                            value: "{choice.value}",
                            checked: choice.value == *value,
                            disabled,
                            onfocus: move |_| emit(FormInput::Focus(index)),
                            onblur: move |_| emit(FormInput::Blur),
                            onchange: move |_| {
                                emit(FormInput::Press(FormHit::Choice { field: index, option }))
                            },
                        }
                        span { "{choice.label}" }
                    }
                }
            }
        },
        ControlModel::Select { placeholder, options, .. } => {
            let open = state.popup() == Some(index);
            let chosen = field.control.selected_index();
            let face = match chosen {
                Some(at) => options[at].label.clone(),
                None => placeholder.clone(),
            };
            let list_id = format!("{control_id}-list");
            let active = state
                .highlighted
                .filter(|_| open)
                .map(|at| format!("{control_id}-opt-{at}"));
            rsx! {
                div { class: "pk-select-wrap",
                    button {
                        class: "pk-select",
                        id: "{control_id}",
                        r#type: "button",
                        role: "combobox",
                        aria_haspopup: "listbox",
                        aria_expanded: if open { "true" } else { "false" },
                        aria_controls: "{list_id}",
                        aria_activedescendant: active,
                        aria_describedby: described_by,
                        "data-empty": if chosen.is_none() { "true" } else { "false" },
                        disabled,
                        onfocus: move |_| emit(FormInput::Focus(index)),
                        onblur: move |_| emit(FormInput::Blur),
                        onclick: move |_| emit(FormInput::Press(FormHit::Control(index))),
                        // Escape closes an open popup; closed, it is the host's key.
                        onkeydown: move |event: KeyboardEvent| {
                            let key = select_key(&event.key())
                                .filter(|key| open || *key != FormKey::Escape);
                            if let Some(key) = key {
                                event.prevent_default();
                                event.stop_propagation();
                                emit(FormInput::Key(key));
                            }
                        },
                        "{face}"
                    }
                    if open {
                        div {
                            class: "pk-dropdown-popup",
                            id: "{list_id}",
                            role: "listbox",
                            aria_labelledby: "{label_id}",
                            // The trigger keeps focus while the list is pressed.
                            onmousedown: move |event| event.prevent_default(),
                            div { class: "pk-dropdown-list",
                                for (option , choice) in options.iter().enumerate() {
                                    div {
                                        key: "{choice.value}",
                                        class: option_class(
                                            state.highlighted == Some(option),
                                            chosen == Some(option),
                                        ),
                                        id: "{control_id}-opt-{option}",
                                        role: "option",
                                        aria_selected: if chosen == Some(option) { "true" } else { "false" },
                                        onclick: move |_| {
                                            emit(FormInput::Press(FormHit::Choice { field: index, option }))
                                        },
                                        "{choice.label}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        ControlModel::Button { text } => rsx! {
            button {
                class: "pk-btn",
                id: "{control_id}",
                r#type: "button",
                disabled,
                aria_describedby: described_by,
                onfocus: move |_| emit(FormInput::Focus(index)),
                onblur: move |_| emit(FormInput::Blur),
                onclick: move |_| emit(FormInput::Press(FormHit::Control(index))),
                "{text}"
            }
        },
    }
}

fn option_class(highlighted: bool, chosen: bool) -> &'static str {
    match (highlighted, chosen) {
        (true, true) => "pk-dropdown-item highlighted selected",
        (true, false) => "pk-dropdown-item highlighted",
        (false, true) => "pk-dropdown-item selected",
        (false, false) => "pk-dropdown-item",
    }
}

/// Slider row: range input (optionally log-mapped, like egui's
/// `.logarithmic(true)`) plus a number box for direct entry. The range input
/// reports continuously; the number box commits on change, clamped into
/// `[min, max]`.
#[component]
pub fn Slider(
    /// Label in the row's key column.
    label: String,
    /// Inclusive minimum. Must be positive when `log` is set.
    min: f64,
    /// Inclusive maximum.
    max: f64,
    /// Current value.
    value: f64,
    /// Receives the committed value.
    on: EventHandler<f64>,
    /// Log-map the slider across orders of magnitude.
    #[props(default)]
    log: Option<bool>,
    /// Native tooltip.
    #[props(default)]
    title: Option<String>,
) -> Element {
    let log = log.unwrap_or(false);
    let pos = if log {
        forms_core::log_position(value, min, max)
    } else {
        value
    };
    let (rmin, rmax, rstep) = forms_core::slider_range(min, max, log);
    let fill = if rmax > rmin {
        ((pos - rmin) / (rmax - rmin)).clamp(0.0, 1.0) * 100.0
    } else {
        0.0
    };
    rsx! {
        div { class: "pk-form-row", title: title.unwrap_or_default(),
            span { class: "pk-form-key", "{label}" }
            input {
                r#type: "range",
                class: "pk-range pk-form-range",
                min: "{rmin}",
                max: "{rmax}",
                step: "{rstep}",
                value: "{pos}",
                style: "--pk-fill:{fill}%",
                oninput: move |e| {
                    if let Ok(p) = e.value().parse::<f64>() {
                        let v = if log { forms_core::log_value(p, min, max) } else { p };
                        on.call(v);
                    }
                },
            }
            input {
                r#type: "number",
                class: "pk-input pk-form-num",
                value: "{forms_core::fmt_f(value)}",
                onchange: move |e| {
                    if let Some(v) = forms_core::parse_clamped_f64(&e.value(), min, max) {
                        on.call(v);
                    }
                },
            }
        }
    }
}

/// Integer drag-value stand-in (egui `DragValue`): a clamped number box.
#[component]
pub fn IntRow(
    /// Label in the row's key column.
    label: String,
    /// Inclusive minimum.
    min: i64,
    /// Inclusive maximum.
    max: i64,
    /// Current value.
    value: i64,
    /// Receives the committed value.
    on: EventHandler<i64>,
    /// Native tooltip.
    #[props(default)]
    title: Option<String>,
    /// Muted note after the input, e.g. an unit or a consequence.
    #[props(default)]
    note: Option<String>,
) -> Element {
    rsx! {
        div { class: "pk-form-row", title: title.unwrap_or_default(),
            span { class: "pk-form-key", "{label}" }
            input {
                r#type: "number",
                class: "pk-input pk-form-num",
                min: "{min}",
                max: "{max}",
                step: "1",
                value: "{value}",
                onchange: move |e| {
                    if let Some(v) = forms_core::parse_clamped_i64(&e.value(), min, max) {
                        on.call(v);
                    }
                },
            }
            if let Some(n) = note {
                span { class: "pk-form-note", "{n}" }
            }
        }
    }
}

/// Checkbox row with an optional trailing text span.
#[component]
pub fn CheckRow(
    /// Label in the row's key column.
    label: String,
    /// Whether the box is checked.
    value: bool,
    /// Receives the new checked state.
    on: EventHandler<bool>,
    /// Text after the box.
    #[props(default)]
    text: Option<String>,
    /// Native tooltip.
    #[props(default)]
    title: Option<String>,
) -> Element {
    rsx! {
        div { class: "pk-form-row", title: title.unwrap_or_default(),
            span { class: "pk-form-key", "{label}" }
            label { class: "pk-form-check",
                input {
                    r#type: "checkbox",
                    class: "pk-check",
                    checked: value,
                    onchange: move |e| on.call(e.checked()),
                }
                if let Some(t) = text {
                    span { "{t}" }
                }
            }
        }
    }
}

/// Single-line text row; commits on change.
#[component]
pub fn TextRow(
    /// Label in the row's key column.
    label: String,
    /// Current value.
    value: String,
    /// Receives the committed value.
    on: EventHandler<String>,
    /// Native tooltip.
    #[props(default)]
    title: Option<String>,
) -> Element {
    rsx! {
        div { class: "pk-form-row", title: title.unwrap_or_default(),
            span { class: "pk-form-key", "{label}" }
            input {
                r#type: "text",
                class: "pk-input pk-form-text",
                value: "{value}",
                onchange: move |e| on.call(e.value()),
            }
        }
    }
}
