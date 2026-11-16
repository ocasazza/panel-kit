use super::*;

fn field(id: &str, control: ControlModel) -> FieldModel {
    FieldModel {
        id: id.into(),
        label: id.into(),
        control,
        disabled: false,
        note: String::new(),
    }
}

fn choices(values: &[&str]) -> Vec<ChoiceModel> {
    values
        .iter()
        .map(|value| ChoiceModel {
            value: (*value).into(),
            label: value.to_uppercase(),
        })
        .collect()
}

fn text(value: &str, entry: TextEntry) -> ControlModel {
    ControlModel::Text {
        value: value.into(),
        placeholder: String::new(),
        entry,
    }
}

/// host, token (disabled), replicas, load, drain, tier, region, apply.
fn form() -> FormModel {
    let mut token = field("token", text("s3cret", TextEntry::Password));
    token.disabled = true;
    FormModel {
        fields: vec![
            field("host", text("pdx", TextEntry::Plain)),
            token,
            field("replicas", ControlModel::Number { value: 3.0, min: 1.0, max: 8.0, step: 1.0 }),
            field("load", ControlModel::Slider { value: 0.5, min: 0.0, max: 1.0, step: 0.1 }),
            field("drain", ControlModel::Checkbox { checked: false }),
            field("tier", ControlModel::Radio { value: "hot".into(), options: choices(&["hot", "warm", "cold"]) }),
            field(
                "region",
                ControlModel::Select {
                    value: String::new(),
                    placeholder: "choose".into(),
                    options: choices(&["pdx", "iad", "fra"]),
                },
            ),
            field("apply", ControlModel::Button { text: "apply".into() }),
        ],
    }
}

fn changed(id: &str, value: FieldValue) -> FormAction {
    FormAction::Changed { id: id.into(), value }
}

/// Feed `keys` through [`key_input`] then [`reduce`], collecting every action.
fn type_keys(model: &mut FormModel, state: &mut FormState, keys: &[FormKey]) -> Vec<FormAction> {
    let mut actions = Vec::new();
    for key in keys {
        if let Some(input) = key_input(model, state, *key) {
            actions.extend(reduce(model, state, input));
        }
    }
    actions
}

fn value_of(model: &FormModel, id: &str) -> FieldValue {
    model.field(id).and_then(|f| f.control.value()).expect("field holds a value")
}

#[test]
fn control_type_is_the_serialized_tag() {
    let model = form();
    let encoded = serde_json::to_value(&model).expect("form serializes");
    assert_eq!(encoded["fields"][2]["control"]["type"], "number");
    assert_eq!(encoded["fields"][0]["control"]["entry"], "plain");
    let decoded: FormModel = serde_json::from_value(encoded).expect("form round-trips");
    assert_eq!(decoded, model);
}

#[test]
fn focus_traversal_wraps_and_skips_disabled_fields() {
    let (mut model, mut state) = (form(), FormState::default());
    type_keys(&mut model, &mut state, &[FormKey::Tab]);
    assert_eq!(state.focus, Some(0));
    type_keys(&mut model, &mut state, &[FormKey::Tab]);
    assert_eq!(state.focus, Some(2), "the disabled token field is skipped");
    type_keys(&mut model, &mut state, &[FormKey::BackTab, FormKey::BackTab]);
    assert_eq!(state.focus, Some(7), "shift-tab wraps to the last field");

    reduce(&mut model, &mut state, FormInput::Focus(1));
    assert_eq!(state.focus, Some(7), "a disabled field refuses focus");
}

#[test]
fn text_edits_happen_at_the_caret() {
    let (mut model, mut state) = (form(), FormState::default());
    reduce(&mut model, &mut state, FormInput::Focus(0));
    assert_eq!(state.caret, 3, "focus parks the caret after the value");

    let actions = type_keys(
        &mut model,
        &mut state,
        &[FormKey::Home, FormKey::Right, FormKey::Char('é'), FormKey::Delete, FormKey::End, FormKey::Backspace],
    );
    assert_eq!(value_of(&model, "host"), FieldValue::Text("pé".into()));
    assert_eq!(actions.last(), Some(&changed("host", FieldValue::Text("pé".into()))));
    assert_eq!(state.caret, 2);
}

#[test]
fn typed_entry_kinds_filter_and_bound_their_input() {
    let mut model = FormModel { fields: vec![field("since", text("", TextEntry::Date))] };
    let mut state = FormState::default();
    reduce(&mut model, &mut state, FormInput::Focus(0));
    for c in "2026-x11-015".chars() {
        reduce(&mut model, &mut state, FormInput::Insert(c));
    }
    assert_eq!(value_of(&model, "since"), FieldValue::Text("2026-11-01".into()));
}

#[test]
fn passwords_display_masked() {
    assert_eq!(text("s3cret", TextEntry::Password).display(), "••••••");
}

#[test]
fn numbers_step_snap_and_clamp() {
    let (mut model, mut state) = (form(), FormState::default());
    reduce(&mut model, &mut state, FormInput::Focus(2));
    let actions = type_keys(&mut model, &mut state, &[FormKey::Up; 9]);
    assert_eq!(value_of(&model, "replicas"), FieldValue::Number(8.0));
    assert_eq!(actions.len(), 5, "steps past the maximum change nothing");

    reduce(&mut model, &mut state, FormInput::Press(FormHit::Stepper { field: 2, delta: -2 }));
    assert_eq!(value_of(&model, "replicas"), FieldValue::Number(6.0));

    reduce(&mut model, &mut state, FormInput::SetNumber { field: 3, value: 0.34 });
    assert_eq!(value_of(&model, "load"), FieldValue::Number(0.3));
}

#[test]
fn a_typed_number_commits_on_enter_or_focus_change_and_escape_discards_it() {
    let (mut model, mut state) = (form(), FormState::default());
    reduce(&mut model, &mut state, FormInput::Focus(2));

    let actions = type_keys(&mut model, &mut state, &[FormKey::Char('4'), FormKey::Char('2')]);
    assert!(actions.is_empty(), "a draft is not a value yet");
    assert_eq!(state.text(&model, 2), "42");
    let actions = type_keys(&mut model, &mut state, &[FormKey::Enter]);
    assert_eq!(actions, [changed("replicas", FieldValue::Number(8.0))], "42 clamps to the maximum");

    type_keys(&mut model, &mut state, &[FormKey::Char('5'), FormKey::Escape]);
    assert_eq!(state.draft, None);
    assert_eq!(value_of(&model, "replicas"), FieldValue::Number(8.0));

    type_keys(&mut model, &mut state, &[FormKey::Backspace, FormKey::Char('2')]);
    let actions = type_keys(&mut model, &mut state, &[FormKey::Tab]);
    assert_eq!(actions, [changed("replicas", FieldValue::Number(2.0))]);
    assert_eq!(state.focus, Some(3));
}

#[test]
fn slider_track_presses_and_range_keys_set_the_value() {
    let (mut model, mut state) = (form(), FormState::default());
    let actions = reduce(&mut model, &mut state, FormInput::Press(FormHit::Track { field: 3, fraction: 0.78 }));
    assert_eq!(actions, [changed("load", FieldValue::Number(0.8))]);
    assert_eq!(state.focus, Some(3));

    type_keys(&mut model, &mut state, &[FormKey::Home]);
    assert_eq!(value_of(&model, "load"), FieldValue::Number(0.0));
    type_keys(&mut model, &mut state, &[FormKey::End, FormKey::Left]);
    assert_eq!(value_of(&model, "load"), FieldValue::Number(0.9));
}

#[test]
fn toggles_and_buttons_act_on_press_and_on_space() {
    let (mut model, mut state) = (form(), FormState::default());
    let actions = reduce(&mut model, &mut state, FormInput::Press(FormHit::Control(4)));
    assert_eq!(actions, [changed("drain", FieldValue::Bool(true))]);
    let actions = type_keys(&mut model, &mut state, &[FormKey::Char(' ')]);
    assert_eq!(actions, [changed("drain", FieldValue::Bool(false))]);

    let actions = reduce(&mut model, &mut state, FormInput::Press(FormHit::Control(7)));
    assert_eq!(actions, [FormAction::Pressed { id: "apply".into() }]);
}

#[test]
fn radio_arrows_wrap_and_presses_choose() {
    let (mut model, mut state) = (form(), FormState::default());
    reduce(&mut model, &mut state, FormInput::Focus(5));
    type_keys(&mut model, &mut state, &[FormKey::Up]);
    assert_eq!(value_of(&model, "tier"), FieldValue::Choice("cold".into()));
    reduce(&mut model, &mut state, FormInput::Press(FormHit::Choice { field: 5, option: 1 }));
    assert_eq!(value_of(&model, "tier"), FieldValue::Choice("warm".into()));
}

#[test]
fn a_closed_select_steps_its_value_without_wrapping() {
    let (mut model, mut state) = (form(), FormState::default());
    reduce(&mut model, &mut state, FormInput::Focus(6));
    type_keys(&mut model, &mut state, &[FormKey::Down]);
    assert_eq!(value_of(&model, "region"), FieldValue::Choice("pdx".into()));
    let actions = type_keys(&mut model, &mut state, &[FormKey::Up]);
    assert!(actions.is_empty(), "the first option is the floor");
}

#[test]
fn the_select_popup_highlights_commits_and_cancels() {
    let (mut model, mut state) = (form(), FormState::default());
    reduce(&mut model, &mut state, FormInput::Press(FormHit::Control(6)));
    assert_eq!(state.popup(), Some(6));
    assert_eq!(state.highlighted, None, "nothing is selected yet");

    let actions = type_keys(&mut model, &mut state, &[FormKey::Down, FormKey::Down]);
    assert!(actions.is_empty(), "moving the highlight is not a selection");
    let actions = type_keys(&mut model, &mut state, &[FormKey::Enter]);
    assert_eq!(actions, [changed("region", FieldValue::Choice("iad".into()))]);
    assert_eq!(state.popup(), None);

    type_keys(&mut model, &mut state, &[FormKey::Enter]);
    assert_eq!(state.highlighted, Some(1), "reopening highlights the selection");
    type_keys(&mut model, &mut state, &[FormKey::Up, FormKey::Up]);
    assert_eq!(state.highlighted, Some(2), "the highlight wraps");
    type_keys(&mut model, &mut state, &[FormKey::Escape]);
    assert_eq!((state.popup(), state.focus), (None, Some(6)), "escape closes but keeps focus");
    assert_eq!(value_of(&model, "region"), FieldValue::Choice("iad".into()));

    reduce(&mut model, &mut state, FormInput::Press(FormHit::Control(6)));
    let actions = reduce(&mut model, &mut state, FormInput::Press(FormHit::Choice { field: 6, option: 2 }));
    assert_eq!(actions, [changed("region", FieldValue::Choice("fra".into()))]);
    assert_eq!(state.popup(), None);

    reduce(&mut model, &mut state, FormInput::Press(FormHit::Control(6)));
    reduce(&mut model, &mut state, FormInput::Press(FormHit::Outside));
    assert_eq!((state.popup(), state.focus), (None, None));
}

#[test]
fn a_focused_form_claims_bare_keys_and_leaves_shortcuts_to_the_host() {
    let chord = |key: crate::Key| crate::KeyChord { key, shift: false, alt: false, ctrl: false, meta: false };
    let (mut model, mut state) = (form(), FormState::default());
    let quit = chord(crate::Key::Char('q'));
    assert_eq!(route_chord(&model, &state, quit), KeyRoute::Host, "an unfocused form claims nothing");

    reduce(&mut model, &mut state, FormInput::Focus(4));
    assert_eq!(route_chord(&model, &state, quit), KeyRoute::Swallowed);
    assert_eq!(
        route_chord(&model, &state, chord(crate::Key::Char(' '))),
        KeyRoute::Input(FormInput::Activate)
    );
    let back_tab = crate::KeyChord { shift: true, ..chord(crate::Key::Tab) };
    assert_eq!(route_chord(&model, &state, back_tab), KeyRoute::Input(FormInput::FocusPrev));
    let ctrl_q = crate::KeyChord { ctrl: true, ..quit };
    assert_eq!(route_chord(&model, &state, ctrl_q), KeyRoute::Host);
    assert_eq!(route_chord(&model, &state, chord(crate::Key::PageDown)), KeyRoute::Host);
}

#[test]
fn a_disabled_field_never_changes() {
    let (mut model, mut state) = (form(), FormState::default());
    let before = model.clone();
    let mut actions = reduce(&mut model, &mut state, FormInput::SetText { field: 1, value: "x".into() });
    actions.extend(reduce(&mut model, &mut state, FormInput::Press(FormHit::Control(1))));
    model.fields[4].disabled = true;
    let disabled = model.clone();
    actions.extend(reduce(&mut model, &mut state, FormInput::Press(FormHit::Control(4))));
    assert!(actions.is_empty());
    assert_eq!(state, FormState::default());
    assert_eq!(model, disabled);
    assert_eq!(before.fields[1], model.fields[1]);
}

#[test]
fn keys_the_focused_control_does_not_use_stay_with_the_host() {
    let (mut model, mut state) = (form(), FormState::default());
    assert_eq!(key_input(&model, &state, FormKey::Char('q')), None);
    assert_eq!(key_input(&model, &state, FormKey::Escape), None);
    reduce(&mut model, &mut state, FormInput::Focus(4));
    assert_eq!(key_input(&model, &state, FormKey::Char('q')), None);
    assert_eq!(key_input(&model, &state, FormKey::Escape), Some(FormInput::Blur));
}
