//! Form interaction state machine: focus, text editing, stepping, the select
//! popup, and pointer presses.
//!
//! Backends translate platform events into [`FormKey`]s or [`FormHit`]s and
//! feed the resulting [`FormInput`]s to [`reduce`], which is the only writer
//! of a [`FormModel`]'s values. It performs no I/O; hosts act on the returned
//! [`FormAction`]s.

use super::model::{ControlModel, FieldValue, FormModel};
use super::fmt_f;
use crate::widgets::dropdown::highlight_next;
use crate::{Key, KeyChord};

/// Ephemeral interaction state of one form. Never persisted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormState {
    /// Index of the focused field.
    pub focus: Option<usize>,
    /// Caret position in the focused text control, in characters.
    pub caret: usize,
    /// Whether the focused select's popup is open.
    pub open: bool,
    /// Option the keyboard highlights in the open popup.
    pub highlighted: Option<usize>,
    /// Uncommitted keyboard entry in the focused number box.
    pub draft: Option<String>,
}

impl FormState {
    /// Whether field `index` holds focus.
    pub fn is_focused(&self, index: usize) -> bool {
        self.focus == Some(index)
    }

    /// The field whose select popup is open.
    pub fn popup(&self) -> Option<usize> {
        self.focus.filter(|_| self.open)
    }

    /// The value text field `index` paints: its uncommitted number entry
    /// while one exists, else [`ControlModel::display`].
    pub fn text(&self, model: &FormModel, index: usize) -> String {
        match (&self.draft, model.fields.get(index)) {
            (Some(draft), _) if self.is_focused(index) => draft.clone(),
            (_, Some(field)) => field.control.display(),
            (_, None) => String::new(),
        }
    }
}

/// A key press, already stripped of platform detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormKey {
    /// Tab.
    Tab,
    /// Shift-Tab.
    BackTab,
    /// Enter or Return.
    Enter,
    /// Escape.
    Escape,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Backspace.
    Backspace,
    /// Forward delete.
    Delete,
    /// A printable character, space included.
    Char(char),
}

impl FormKey {
    /// The form key a chord means. Control, Alt, and Meta chords are
    /// shortcuts, not form input, and paging keys scroll the host: `None`.
    pub fn from_chord(chord: KeyChord) -> Option<Self> {
        if chord.ctrl || chord.alt || chord.meta {
            return None;
        }
        Some(match chord.key {
            Key::Tab if chord.shift => Self::BackTab,
            Key::Tab => Self::Tab,
            Key::Enter => Self::Enter,
            Key::Escape => Self::Escape,
            Key::Left => Self::Left,
            Key::Right => Self::Right,
            Key::Up => Self::Up,
            Key::Down => Self::Down,
            Key::Home => Self::Home,
            Key::End => Self::End,
            Key::Backspace => Self::Backspace,
            Key::Delete => Self::Delete,
            Key::Char(c) => Self::Char(c),
            Key::PageUp | Key::PageDown => return None,
        })
    }
}

/// What a pointer press landed on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FormHit {
    /// The body of a field's control, by field index.
    Control(usize),
    /// One option of a radio group, or of a select's open popup.
    Choice {
        /// Field index.
        field: usize,
        /// Option index.
        option: usize,
    },
    /// A point on a slider's track.
    Track {
        /// Field index.
        field: usize,
        /// Position along the track, `0.0..=1.0`.
        fraction: f64,
    },
    /// A number box's decrement or increment affordance.
    Stepper {
        /// Field index.
        field: usize,
        /// Steps to move; negative decrements.
        delta: i32,
    },
    /// Anywhere that is not a control.
    Outside,
}

/// One renderer-neutral input to [`reduce`].
#[derive(Clone, Debug, PartialEq)]
pub enum FormInput {
    /// Move focus to a field.
    Focus(usize),
    /// Drop focus.
    Blur,
    /// Move focus to the next enabled field, wrapping.
    FocusNext,
    /// Move focus to the previous enabled field, wrapping.
    FocusPrev,
    /// Act on the focused control: toggle, press, open or commit.
    Activate,
    /// Close the open popup, else discard an uncommitted number entry.
    Cancel,
    /// Step the focused control: a slider or number by its step, a radio
    /// group or select by one option.
    Step(i32),
    /// Type a character into the focused text control or number box.
    Insert(char),
    /// Erase the character before the caret.
    Backspace,
    /// Erase the character at the caret.
    Delete,
    /// Move the caret one character left.
    CaretLeft,
    /// Move the caret one character right.
    CaretRight,
    /// Move the caret to the start of the value.
    CaretHome,
    /// Move the caret to the end of the value.
    CaretEnd,
    /// A pointer press.
    Press(FormHit),
    /// A key press, resolved against the focused control by [`key_input`].
    Key(FormKey),
    /// Replace a text control's whole value, as a native text input reports it.
    SetText {
        /// Field index.
        field: usize,
        /// New value.
        value: String,
    },
    /// Set a slider or number, clamped and snapped to its step.
    SetNumber {
        /// Field index.
        field: usize,
        /// Requested value.
        value: f64,
    },
}

/// What the form reports back to the host.
#[derive(Clone, Debug, PartialEq)]
pub enum FormAction {
    /// A field's value changed.
    Changed {
        /// The field's stable id.
        id: String,
        /// Its new value.
        value: FieldValue,
    },
    /// A button was pressed.
    Pressed {
        /// The button field's stable id.
        id: String,
    },
}

/// Translate a key press into the input it means for the focused control.
/// `None` leaves the key to the host.
pub fn key_input(model: &FormModel, state: &FormState, key: FormKey) -> Option<FormInput> {
    use FormInput as I;
    use FormKey as K;

    match key {
        K::Tab => return Some(I::FocusNext),
        K::BackTab => return Some(I::FocusPrev),
        _ => {}
    }
    let (index, field) = match state.focus.and_then(|i| model.fields.get(i).map(|f| (i, f))) {
        Some(focused) => focused,
        None => {
            return match key {
                K::Down => Some(I::FocusNext),
                K::Up => Some(I::FocusPrev),
                _ => None,
            }
        }
    };
    if key == K::Escape {
        return Some(if state.open || state.draft.is_some() { I::Cancel } else { I::Blur });
    }
    match &field.control {
        ControlModel::Text { .. } | ControlModel::TextArea { .. } => match key {
            K::Char(c) => Some(I::Insert(c)),
            K::Left => Some(I::CaretLeft),
            K::Right => Some(I::CaretRight),
            K::Home => Some(I::CaretHome),
            K::End => Some(I::CaretEnd),
            K::Backspace => Some(I::Backspace),
            K::Delete => Some(I::Delete),
            K::Up => Some(I::FocusPrev),
            K::Down => Some(I::FocusNext),
            K::Enter if matches!(field.control, ControlModel::TextArea { .. }) => {
                Some(I::Insert('\n'))
            }
            K::Enter => Some(I::FocusNext),
            _ => None,
        },
        ControlModel::Number { .. } => match key {
            K::Char(c) => Some(I::Insert(c)),
            K::Backspace => Some(I::Backspace),
            K::Up | K::Right => Some(I::Step(1)),
            K::Down | K::Left => Some(I::Step(-1)),
            K::Enter => Some(I::Activate),
            _ => None,
        },
        ControlModel::Slider { min, max, .. } => match key {
            K::Up | K::Right => Some(I::Step(1)),
            K::Down | K::Left => Some(I::Step(-1)),
            K::Home => Some(I::SetNumber { field: index, value: *min }),
            K::End => Some(I::SetNumber { field: index, value: *max }),
            _ => None,
        },
        ControlModel::Checkbox { .. } | ControlModel::Switch { .. } | ControlModel::Button { .. } => {
            match key {
                K::Enter | K::Char(' ') => Some(I::Activate),
                K::Up => Some(I::FocusPrev),
                K::Down => Some(I::FocusNext),
                _ => None,
            }
        }
        ControlModel::Radio { .. } => match key {
            K::Down | K::Right => Some(I::Step(1)),
            K::Up | K::Left => Some(I::Step(-1)),
            _ => None,
        },
        ControlModel::Select { .. } => match key {
            K::Enter | K::Char(' ') => Some(I::Activate),
            K::Down => Some(I::Step(1)),
            K::Up => Some(I::Step(-1)),
            _ => None,
        },
    }
}

/// Who a key chord belongs to while a form may hold focus.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyRoute {
    /// The form acts on it: feed the input to [`reduce`].
    Input(FormInput),
    /// The form holds focus and the focused control has no use for the key;
    /// it goes nowhere, so typing in a form never fires a host shortcut.
    Swallowed,
    /// Not the form's: no field is focused, or the chord is a modified
    /// shortcut or a paging key.
    Host,
}

/// Route a host key chord: to the focused control, nowhere, or back to the
/// host. A form with no focused field claims no key; hosts move focus into
/// it with [`FormInput::FocusNext`] or a press.
pub fn route_chord(model: &FormModel, state: &FormState, chord: KeyChord) -> KeyRoute {
    if state.focus.is_none() {
        return KeyRoute::Host;
    }
    match FormKey::from_chord(chord) {
        Some(key) => key_input(model, state, key).map_or(KeyRoute::Swallowed, KeyRoute::Input),
        None => KeyRoute::Host,
    }
}

/// Apply one input to the form. Returns the value changes and button presses
/// it caused, in order; an input that changes nothing returns none.
pub fn reduce(model: &mut FormModel, state: &mut FormState, input: FormInput) -> Vec<FormAction> {
    let mut out = Vec::new();
    match input {
        FormInput::Focus(index) => {
            focus(model, state, Some(index), &mut out);
        }
        FormInput::Blur => {
            focus(model, state, None, &mut out);
        }
        FormInput::FocusNext | FormInput::FocusPrev => {
            let forward = input == FormInput::FocusNext;
            if let Some(target) = neighbor(model, state.focus, forward) {
                focus(model, state, Some(target), &mut out);
            }
        }
        FormInput::Press(hit) => press(model, state, hit, &mut out),
        FormInput::Key(key) => {
            if let Some(input) = key_input(model, state, key) {
                return reduce(model, state, input);
            }
        }
        FormInput::SetText { field, value } => set_text(model, state, field, value, &mut out),
        FormInput::SetNumber { field, value } => {
            if state.is_focused(field) {
                state.draft = None;
            }
            set_number(model, field, value, &mut out);
        }
        FormInput::Activate => {
            if let Some(index) = enabled_focus(model, state) {
                activate(model, state, index, &mut out);
            }
        }
        FormInput::Cancel => {
            if state.open {
                close(state);
            } else {
                state.draft = None;
            }
        }
        FormInput::Step(delta) => {
            if let Some(index) = enabled_focus(model, state) {
                step(model, state, index, delta, &mut out);
            }
        }
        FormInput::Insert(c) => {
            if let Some(index) = enabled_focus(model, state) {
                insert(model, state, index, c, &mut out);
            }
        }
        FormInput::Backspace | FormInput::Delete => {
            if let Some(index) = enabled_focus(model, state) {
                erase(model, state, index, input == FormInput::Backspace, &mut out);
            }
        }
        FormInput::CaretLeft => state.caret = state.caret.saturating_sub(1),
        FormInput::CaretRight => {
            let len = state.focus.map_or(0, |index| text_len(model, index));
            state.caret = (state.caret + 1).min(len);
        }
        FormInput::CaretHome => state.caret = 0,
        FormInput::CaretEnd => state.caret = state.focus.map_or(0, |index| text_len(model, index)),
    }
    out
}

fn enabled_focus(model: &FormModel, state: &FormState) -> Option<usize> {
    state
        .focus
        .filter(|&index| model.fields.get(index).is_some_and(|field| !field.disabled))
}

fn text_len(model: &FormModel, index: usize) -> usize {
    match model.fields.get(index).map(|field| &field.control) {
        Some(ControlModel::Text { value, .. } | ControlModel::TextArea { value, .. }) => {
            value.chars().count()
        }
        _ => 0,
    }
}

/// Next enabled field after (or before) `from`, wrapping; from no focus, the
/// first (or last) enabled field.
fn neighbor(model: &FormModel, from: Option<usize>, forward: bool) -> Option<usize> {
    let len = model.fields.len();
    let enabled = |index: &usize| !model.fields[*index].disabled;
    match (from, forward) {
        (None, true) => (0..len).find(enabled),
        (None, false) => (0..len).rev().find(enabled),
        (Some(from), true) => (1..=len).map(|offset| (from + offset) % len).find(enabled),
        (Some(from), false) => (1..=len).map(|offset| (from + len - offset % len) % len).find(enabled),
    }
}

/// Move focus, committing a pending number entry and closing any popup.
/// Returns whether `target` now holds focus.
fn focus(
    model: &mut FormModel,
    state: &mut FormState,
    target: Option<usize>,
    out: &mut Vec<FormAction>,
) -> bool {
    if let Some(index) = target {
        if model.fields.get(index).is_none_or(|field| field.disabled) {
            return false;
        }
    }
    if state.focus == target {
        return true;
    }
    commit_draft(model, state, out);
    state.focus = target;
    close(state);
    state.caret = target.map_or(0, |index| text_len(model, index));
    true
}

fn close(state: &mut FormState) {
    state.open = false;
    state.highlighted = None;
}

fn commit_draft(model: &mut FormModel, state: &mut FormState, out: &mut Vec<FormAction>) {
    let Some(draft) = state.draft.take() else {
        return;
    };
    if let (Some(index), Ok(value)) = (state.focus, draft.trim().parse::<f64>()) {
        set_number(model, index, value, out);
    }
}

fn snap(value: f64, min: f64, max: f64, step: f64) -> f64 {
    let stepped = if step > 0.0 {
        min + ((value - min) / step).round() * step
    } else {
        value
    };
    // Rounds away the binary noise `n * step` accumulates (0.1 * 3).
    let rounded = (stepped * 1e9).round() / 1e9;
    rounded.max(min).min(max)
}

fn set_number(model: &mut FormModel, index: usize, requested: f64, out: &mut Vec<FormAction>) {
    let Some(field) = model.fields.get_mut(index) else {
        return;
    };
    if field.disabled || requested.is_nan() {
        return;
    }
    if let ControlModel::Number { value, min, max, step }
    | ControlModel::Slider { value, min, max, step } = &mut field.control
    {
        let next = snap(requested, *min, *max, *step);
        if next != *value {
            *value = next;
            out.push(FormAction::Changed {
                id: field.id.clone(),
                value: FieldValue::Number(next),
            });
        }
    }
}

fn set_text(
    model: &mut FormModel,
    state: &mut FormState,
    index: usize,
    next: String,
    out: &mut Vec<FormAction>,
) {
    let Some(field) = model.fields.get_mut(index) else {
        return;
    };
    if field.disabled {
        return;
    }
    let (value, next) = match &mut field.control {
        // A single-line value holds only what its entry kind accepts.
        ControlModel::Text { value, entry, .. } => {
            let entry = *entry;
            let accepted = next.chars().filter(|c| entry.accepts(*c));
            let next: String = accepted.take(entry.max_len().unwrap_or(usize::MAX)).collect();
            (value, next)
        }
        ControlModel::TextArea { value, .. } => (value, next),
        _ => return,
    };
    if *value != next {
        *value = next;
        out.push(FormAction::Changed {
            id: field.id.clone(),
            value: FieldValue::Text(value.clone()),
        });
    }
    if state.focus == Some(index) {
        state.caret = state.caret.min(value.chars().count());
    }
}

fn set_choice(model: &mut FormModel, index: usize, option: usize, out: &mut Vec<FormAction>) {
    let field = &mut model.fields[index];
    if let ControlModel::Radio { value, options } | ControlModel::Select { value, options, .. } =
        &mut field.control
    {
        let Some(choice) = options.get(option) else {
            return;
        };
        if *value != choice.value {
            *value = choice.value.clone();
            out.push(FormAction::Changed {
                id: field.id.clone(),
                value: FieldValue::Choice(value.clone()),
            });
        }
    }
}

fn activate(model: &mut FormModel, state: &mut FormState, index: usize, out: &mut Vec<FormAction>) {
    let field = &mut model.fields[index];
    match &mut field.control {
        ControlModel::Checkbox { checked: flag } | ControlModel::Switch { on: flag } => {
            *flag = !*flag;
            out.push(FormAction::Changed {
                id: field.id.clone(),
                value: FieldValue::Bool(*flag),
            });
        }
        ControlModel::Button { .. } => out.push(FormAction::Pressed {
            id: field.id.clone(),
        }),
        ControlModel::Select { .. } if state.open => {
            let chosen = state.highlighted;
            close(state);
            if let Some(option) = chosen {
                set_choice(model, index, option, out);
            }
        }
        ControlModel::Select { .. } => {
            state.open = true;
            state.highlighted = field.control.selected_index();
        }
        ControlModel::TextArea { .. } => insert(model, state, index, '\n', out),
        ControlModel::Number { .. } => commit_draft(model, state, out),
        ControlModel::Text { .. } | ControlModel::Slider { .. } | ControlModel::Radio { .. } => {}
    }
}

fn step(
    model: &mut FormModel,
    state: &mut FormState,
    index: usize,
    delta: i32,
    out: &mut Vec<FormAction>,
) {
    commit_draft(model, state, out);
    let len = model.fields[index].control.options().len();
    let selected = model.fields[index].control.selected_index();
    match model.fields[index].control {
        ControlModel::Number { value, step, .. } | ControlModel::Slider { value, step, .. } => {
            let unit = if step > 0.0 { step } else { 1.0 };
            set_number(model, index, value + f64::from(delta) * unit, out);
        }
        ControlModel::Select { .. } if state.open => {
            state.highlighted = highlight_next(state.highlighted, len, delta > 0);
        }
        ControlModel::Radio { .. } if len > 0 => {
            if let Some(next) = highlight_next(selected, len, delta > 0) {
                set_choice(model, index, next, out);
            }
        }
        ControlModel::Select { .. } if len > 0 => {
            let next = match selected {
                None if delta > 0 => 0,
                None => len - 1,
                Some(at) if delta > 0 => (at + 1).min(len - 1),
                Some(at) => at.saturating_sub(1),
            };
            set_choice(model, index, next, out);
        }
        _ => {}
    }
}

fn byte_at(value: &str, caret: usize) -> usize {
    value
        .char_indices()
        .nth(caret)
        .map_or(value.len(), |(byte, _)| byte)
}

fn insert(
    model: &mut FormModel,
    state: &mut FormState,
    index: usize,
    c: char,
    out: &mut Vec<FormAction>,
) {
    let field = &mut model.fields[index];
    let (value, accepted) = match &mut field.control {
        ControlModel::Text { value, entry, .. } => {
            let fits = entry.max_len().is_none_or(|max| value.chars().count() < max);
            let accepted = entry.accepts(c) && fits;
            (value, accepted)
        }
        ControlModel::TextArea { value, .. } => (value, c == '\n' || !c.is_control()),
        ControlModel::Number { .. } => {
            if c.is_ascii_digit() || c == '.' || c == '-' {
                state.draft.get_or_insert_with(String::new).push(c);
            }
            return;
        }
        _ => return,
    };
    if !accepted {
        return;
    }
    let caret = state.caret.min(value.chars().count());
    value.insert(byte_at(value, caret), c);
    state.caret = caret + 1;
    out.push(FormAction::Changed {
        id: field.id.clone(),
        value: FieldValue::Text(value.clone()),
    });
}

fn erase(
    model: &mut FormModel,
    state: &mut FormState,
    index: usize,
    backward: bool,
    out: &mut Vec<FormAction>,
) {
    let field = &mut model.fields[index];
    let value = match &mut field.control {
        ControlModel::Text { value, .. } | ControlModel::TextArea { value, .. } => value,
        ControlModel::Number { value, .. } => {
            if backward {
                state.draft.get_or_insert_with(|| fmt_f(*value)).pop();
            }
            return;
        }
        _ => return,
    };
    let len = value.chars().count();
    let caret = state.caret.min(len);
    let at = match (backward, caret) {
        (true, 0) => return,
        (true, caret) => caret - 1,
        (false, caret) if caret >= len => return,
        (false, caret) => caret,
    };
    value.remove(byte_at(value, at));
    state.caret = at;
    out.push(FormAction::Changed {
        id: field.id.clone(),
        value: FieldValue::Text(value.clone()),
    });
}

fn press(model: &mut FormModel, state: &mut FormState, hit: FormHit, out: &mut Vec<FormAction>) {
    match hit {
        FormHit::Outside => {
            focus(model, state, None, out);
        }
        FormHit::Control(index) => {
            if !focus(model, state, Some(index), out) {
                return;
            }
            match &model.fields[index].control {
                ControlModel::Checkbox { .. }
                | ControlModel::Switch { .. }
                | ControlModel::Button { .. } => activate(model, state, index, out),
                ControlModel::Select { .. } if state.open => close(state),
                ControlModel::Select { .. } => {
                    state.open = true;
                    state.highlighted = model.fields[index].control.selected_index();
                }
                ControlModel::Text { .. } | ControlModel::TextArea { .. } => {
                    state.caret = text_len(model, index);
                }
                ControlModel::Number { .. }
                | ControlModel::Slider { .. }
                | ControlModel::Radio { .. } => {}
            }
        }
        FormHit::Choice { field, option } => {
            if focus(model, state, Some(field), out) {
                close(state);
                set_choice(model, field, option, out);
            }
        }
        FormHit::Track { field, fraction } => {
            if !focus(model, state, Some(field), out) {
                return;
            }
            if let ControlModel::Slider { min, max, .. } = model.fields[field].control {
                set_number(model, field, min + fraction.clamp(0.0, 1.0) * (max - min), out);
            }
        }
        FormHit::Stepper { field, delta } => {
            if focus(model, state, Some(field), out) {
                step(model, state, field, delta, out);
            }
        }
    }
}
