//! Bevy UI painter and input systems for core's form controls.
//!
//! Core owns the fields, values, focus, text editing, stepping, and the
//! select popup ([`panel_kit_core::widgets::forms`]). This module paints a
//! [`Form`] with `bevy_ui` at the web stylesheet's metrics (`assets/panel-kit.css`,
//! "form controls"), turns [`Interaction`] presses into core [`FormHit`]s and
//! key events into core chords, and reports what core's `reduce` returns as
//! [`FormActionEvent`]s.
//!
//! Add [`FormPlugin`] once ([`PanelUiPlugin`](crate::PanelUiPlugin) adds it).
//! The [`Form`] on the root entity is the live model: read it back with
//! `world.get::<Form>(root)`, or assign it to repaint from host state.

use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use panel_kit_core::widgets::forms::{
    self, ControlModel, FieldModel, FormHit, FormInput, FormModel, FormState, KeyRoute,
};

pub use panel_kit_core::widgets::forms::FormAction;

use super::data_table::{insert_chevron_image, CHEVRON_BOX, CHEVRON_IMAGE, CHEVRON_RIGHT};
use super::{mix_srgb, styled_text, TextStyle};
use crate::fonts::{ch_em, glyph_advance_em};
use crate::input::{text_chord, ClaimedKeys, ModifierState, WorkspaceKeys};
use crate::render::BodyPaint;
use crate::theme::BevyTheme;

/// `.pk-field { grid-template-columns: 88px … }`: the key column.
const KEY_W: f32 = 88.0;
/// `--hit-min`: the row's minimum height and the key label's line box.
const ROW_MIN: f32 = 24.0;
/// Key labels, notes, and captions are `11px`.
const CAPTION: f32 = 11.0;
/// `letter-spacing: .06em` of the tracked-caps key labels and captions.
const TRACKING: f32 = 0.06;
/// `line-height: 1.4` of every control's text.
const LINE: f32 = 1.4;
/// `.pk-input { padding: 1px 6px }`.
const PAD: (f32, f32) = (6.0, 1.0);
/// `.pk-check`, `.pk-radio`: the `14px` box, and the marks inside it.
const CHECK: f32 = 14.0;
const CHECK_MARK: f32 = 8.0;
const RADIO_DOT: f32 = 6.0;
/// `.pk-switch`: a `28x14` track and a `10px` thumb `1px` inside its edge.
const SWITCH: (f32, f32) = (28.0, 14.0);
const SWITCH_THUMB: f32 = 10.0;
/// `.pk-range`: a `2px` track in a `14px` row and an `8x14` thumb.
const TRACK_H: f32 = 2.0;
const SLIDER_THUMB: (f32, f32) = (8.0, 14.0);
/// `.pk-slider-value { min-width: 4ch }`.
const SLIDER_VALUE_CH: f32 = 4.0;
/// `.pk-number > .pk-input { width: 72px }` and the `20px` steppers.
const NUMBER_W: f32 = 72.0;
const STEP_W: f32 = 20.0;
/// `.pk-select { padding-right: 26px }`: room for the chevron.
const SELECT_PAD_RIGHT: f32 = 26.0;
/// `.pk-select-wrap > .pk-dropdown-popup { top: calc(100% + 2px) }`.
const POPUP_GAP: f32 = 2.0;
/// `.pk-dropdown-item { padding: .28rem .55rem; font-size: 12px }`.
const OPTION_PAD_REM: (f32, f32) = (0.55, 0.28);
const OPTION_SIZE: f32 = 12.0;
/// `.pk-dropdown-list { padding: .25rem 0 }`.
const LIST_PAD_REM: f32 = 0.25;
/// The shared keyboard ring: `2px` wide, `2px` out.
const FOCUS_RING: f32 = 2.0;

/// One form: the model core's `reduce` writes and its interaction state.
/// Lives on the root entity [`spawn`] returns.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Form {
    /// Fields and values.
    pub model: FormModel,
    /// Focus, caret, and popup state.
    pub state: FormState,
}

impl Form {
    /// A form over `model` with nothing focused.
    pub fn new(model: FormModel) -> Self {
        Self {
            model,
            state: FormState::default(),
        }
    }
}

/// A value change or button press core reported for one form.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct FormActionEvent {
    /// Root entity of the form.
    pub form: Entity,
    /// What changed.
    pub action: FormAction,
}

/// Paints [`Form`] entities and routes their presses and keys through core.
#[derive(Default)]
pub struct FormPlugin;

impl Plugin for FormPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<FormActionEvent>()
            .add_event::<KeyboardInput>()
            .init_resource::<ClaimedKeys>()
            .init_resource::<BevyTheme>()
            .add_systems(Startup, insert_chevron_image)
            .add_systems(Update, (press_forms, key_forms).chain().before(WorkspaceKeys))
            // After the body painters, before layout: a form spawned or
            // changed this frame is laid out in it.
            .add_systems(
                PostUpdate,
                paint_forms.after(BodyPaint).before(bevy::ui::UiSystem::Prepare),
            );
    }
}

/// Spawn a form under `parent` and return its root entity. [`FormPlugin`]
/// paints it before the frame is laid out.
pub fn spawn(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, form: Form) -> Entity {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(theme.metrics.space_sm),
                ..default()
            },
            form,
        ))
        .id()
}

/// What a pressable node of a painted form is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Control(usize),
    Choice { field: usize, option: usize },
    Track(usize),
    Stepper { field: usize, delta: i32 },
}

/// Marks a pressable node with its form and part.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
struct FormNode {
    form: Entity,
    part: Part,
}

/// Apply `input` through core and report the actions. Marks the form changed
/// only when its values or state did.
fn apply(
    entity: Entity,
    form: &mut Mut<Form>,
    input: FormInput,
    actions: &mut EventWriter<FormActionEvent>,
) {
    let before = form.state.clone();
    let inner = form.bypass_change_detection();
    let reported = forms::reduce(&mut inner.model, &mut inner.state, input);
    if !reported.is_empty() || form.state != before {
        form.set_changed();
    }
    for action in reported {
        actions.write(FormActionEvent { form: entity, action });
    }
}

/// Turn presses on form parts into core hits. A left press that lands on no
/// part of a focused form closes its popup and drops its focus.
fn press_forms(
    buttons: Option<Res<ButtonInput<MouseButton>>>,
    pressed: Query<
        (&Interaction, &FormNode, Option<&RelativeCursorPosition>),
        Changed<Interaction>,
    >,
    mut forms: Query<(Entity, &mut Form)>,
    mut actions: EventWriter<FormActionEvent>,
) {
    let mut hit_forms = Vec::new();
    for (interaction, node, cursor) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok((entity, mut form)) = forms.get_mut(node.form) else {
            continue;
        };
        let hit = match node.part {
            Part::Control(field) => FormHit::Control(field),
            Part::Choice { field, option } => FormHit::Choice { field, option },
            Part::Stepper { field, delta } => FormHit::Stepper { field, delta },
            Part::Track(field) => FormHit::Track {
                field,
                fraction: cursor
                    .and_then(|cursor| cursor.normalized)
                    .map_or(0.0, |at| f64::from(at.x)),
            },
        };
        apply(entity, &mut form, FormInput::Press(hit), &mut actions);
        hit_forms.push(entity);
    }
    if !buttons.is_some_and(|buttons| buttons.just_pressed(MouseButton::Left)) {
        return;
    }
    for (entity, mut form) in &mut forms {
        if form.state.focus.is_some() && !hit_forms.contains(&entity) {
            apply(entity, &mut form, FormInput::Press(FormHit::Outside), &mut actions);
        }
    }
}

/// Route key presses to the focused form through core's `route_chord`, and
/// record the events it took so the workspace keymap skips them.
fn key_forms(
    mut keys: EventReader<KeyboardInput>,
    mut modifiers: Local<ModifierState>,
    mut claimed: ResMut<ClaimedKeys>,
    mut forms: Query<(Entity, &mut Form)>,
    mut actions: EventWriter<FormActionEvent>,
) {
    if !claimed.0.is_empty() {
        claimed.0.clear();
    }
    for (index, event) in keys.read().enumerate() {
        modifiers.observe(event);
        let Some(chord) = text_chord(event, *modifiers) else {
            continue;
        };
        for (entity, mut form) in &mut forms {
            match forms::route_chord(&form.model, &form.state, chord) {
                KeyRoute::Host => continue,
                KeyRoute::Swallowed => {}
                KeyRoute::Input(input) => apply(entity, &mut form, input, &mut actions),
            }
            claimed.0.push(index);
            break;
        }
    }
}

/// Repaint every form whose model, state, or theme changed.
fn paint_forms(
    theme: Res<BevyTheme>,
    forms: Query<(Entity, Ref<Form>)>,
    mut commands: Commands,
) {
    let rethemed = theme.is_changed();
    for (entity, form) in &forms {
        if !(rethemed || form.is_changed()) {
            continue;
        }
        commands
            .entity(entity)
            .despawn_related::<Children>()
            .with_children(|rows| paint(rows, &theme, entity, &form));
    }
}

/// Colours of one field's control, by state.
#[derive(Clone, Copy)]
struct Tone {
    edge: Color,
    text: Color,
    muted: Color,
    live: Color,
    focused: bool,
}

/// Body text at the controls' `1.4` line height.
fn body(theme: &BevyTheme) -> TextStyle {
    TextStyle::body(theme).line_height(LINE)
}

/// The `11px` muted caption: notes.
fn caption(theme: &BevyTheme) -> TextStyle {
    TextStyle {
        size: CAPTION,
        ..body(theme)
    }
    .color(theme.text_muted)
}

/// A square control edge on the recessed background.
fn boxed(node: Node, theme: &BevyTheme, tone: Tone) -> impl Bundle {
    (
        Node {
            border: UiRect::all(Val::Px(1.0)),
            ..node
        },
        BackgroundColor(theme.background),
        BorderColor(tone.edge),
    )
}

/// The shared keyboard ring: `2px`, `2px` out.
fn ring(entity: &mut EntityCommands, theme: &BevyTheme, tone: Tone) {
    if tone.focused {
        entity.insert(Outline::new(
            Val::Px(FOCUS_RING),
            Val::Px(FOCUS_RING),
            theme.focus_ring,
        ));
    }
}

fn paint(rows: &mut ChildSpawnerCommands, theme: &BevyTheme, entity: Entity, form: &Form) {
    for (index, field) in form.model.fields.iter().enumerate() {
        let focused = form.state.is_focused(index);
        let tone = if field.disabled {
            Tone {
                edge: theme.divider,
                text: theme.text_muted,
                muted: theme.text_muted,
                live: theme.text_muted,
                focused: false,
            }
        } else {
            Tone {
                edge: theme.border,
                text: theme.text,
                muted: theme.text_muted,
                live: theme.highlight,
                focused,
            }
        };
        rows.spawn(Node {
            width: Val::Percent(100.0),
            column_gap: Val::Px(theme.metrics.space_md),
            align_items: AlignItems::FlexStart,
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                width: Val::Px(KEY_W),
                height: Val::Px(ROW_MIN),
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|key| {
                if !field.label.is_empty() {
                    let style = caption(theme)
                        .line_height(ROW_MIN / CAPTION)
                        .tracked_caps(TRACKING);
                    styled_text(key, field.label.as_str(), style);
                }
            });
            row.spawn(Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_width: Val::Px(0.0),
                min_height: Val::Px(ROW_MIN),
                align_items: AlignItems::Center,
                column_gap: Val::Px(theme.metrics.space_md),
                ..default()
            })
            .with_children(|cell| {
                let part = FormNode {
                    form: entity,
                    part: Part::Control(index),
                };
                control(cell, theme, tone, part, index, field, form);
                if !field.note.is_empty() {
                    styled_text(cell, field.note.as_str(), caption(theme));
                }
            });
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn control(
    cell: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    tone: Tone,
    node: FormNode,
    index: usize,
    field: &FieldModel,
    form: &Form,
) {
    let text = body(theme).color(tone.text);
    match &field.control {
        ControlModel::Text { .. } | ControlModel::TextArea { .. } => {
            text_box(cell, theme, tone, node, field, &form.state)
        }
        ControlModel::Number { .. } => {
            let value = form.state.text(&form.model, index);
            cell.spawn(Node {
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|number| {
                let mut value_box = number.spawn((
                    Button,
                    Interaction::default(),
                    node,
                    boxed(
                        Node {
                            width: Val::Px(NUMBER_W),
                            padding: UiRect::axes(Val::Px(PAD.0), Val::Px(PAD.1)),
                            justify_content: JustifyContent::FlexEnd,
                            ..default()
                        },
                        theme,
                        tone,
                    ),
                ));
                ring(&mut value_box, theme, tone);
                value_box.with_children(|value_box| {
                    styled_text(value_box, value, text);
                });
                for (glyph, delta) in [("\u{2212}", -1), ("+", 1)] {
                    number
                        .spawn((
                            Button,
                            Interaction::default(),
                            FormNode {
                                part: Part::Stepper { field: index, delta },
                                ..node
                            },
                            boxed(
                                Node {
                                    width: Val::Px(STEP_W),
                                    margin: UiRect::left(Val::Px(-1.0)),
                                    padding: UiRect::vertical(Val::Px(PAD.1)),
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                                theme,
                                tone,
                            ),
                        ))
                        .with_children(|step| {
                            styled_text(step, glyph, body(theme).color(tone.muted));
                        });
                }
            });
        }
        ControlModel::Slider { .. } => {
            let fraction = field.control.fraction().unwrap_or(0.0) as f32;
            let rest = if field.disabled { theme.divider } else { theme.border };
            cell.spawn(Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_width: Val::Px(0.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(theme.metrics.space_md),
                ..default()
            })
            .with_children(|slider| {
                let mut track = slider.spawn((
                    Button,
                    Interaction::default(),
                    RelativeCursorPosition::default(),
                    FormNode {
                        part: Part::Track(index),
                        ..node
                    },
                    Node {
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        min_width: Val::Px(0.0),
                        height: Val::Px(SLIDER_THUMB.1),
                        ..default()
                    },
                ));
                ring(&mut track, theme, tone);
                track.with_children(|track| {
                    let line = |left: Val, width: Val| Node {
                        position_type: PositionType::Absolute,
                        left,
                        top: Val::Px((SLIDER_THUMB.1 - TRACK_H) / 2.0),
                        width,
                        height: Val::Px(TRACK_H),
                        ..default()
                    };
                    track.spawn((
                        line(Val::ZERO, Val::Percent(100.0)),
                        BackgroundColor(rest),
                    ));
                    track.spawn((
                        line(Val::ZERO, Val::Percent(fraction * 100.0)),
                        BackgroundColor(tone.text),
                    ));
                    // The thumb travels the track less its own width.
                    track.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(fraction * 100.0),
                            margin: UiRect::left(Val::Px(-SLIDER_THUMB.0 * fraction)),
                            top: Val::ZERO,
                            width: Val::Px(SLIDER_THUMB.0),
                            height: Val::Px(SLIDER_THUMB.1),
                            ..default()
                        },
                        BackgroundColor(tone.text),
                    ));
                });
                slider
                    .spawn(Node {
                        min_width: Val::Px(SLIDER_VALUE_CH * ch_em() * text.size),
                        flex_shrink: 0.0,
                        justify_content: JustifyContent::FlexEnd,
                        ..default()
                    })
                    .with_children(|output| {
                        styled_text(output, field.control.display(), text);
                    });
            });
        }
        ControlModel::Checkbox { checked } => {
            let mut check = cell.spawn((
                Button,
                Interaction::default(),
                node,
                boxed(
                    Node {
                        width: Val::Px(CHECK),
                        height: Val::Px(CHECK),
                        flex_shrink: 0.0,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    theme,
                    tone,
                ),
            ));
            ring(&mut check, theme, tone);
            if *checked {
                check.with_children(|check| {
                    check.spawn((
                        Node {
                            width: Val::Px(CHECK_MARK),
                            height: Val::Px(CHECK_MARK),
                            ..default()
                        },
                        BackgroundColor(tone.live),
                    ));
                });
            }
        }
        ControlModel::Switch { on } => {
            let (left, thumb, word, ink) = if *on {
                (SWITCH.0 - 2.0 - SWITCH_THUMB - 1.0, tone.live, "on", tone.text)
            } else {
                (1.0, tone.muted, "off", tone.muted)
            };
            let mut track = cell.spawn((
                Button,
                Interaction::default(),
                node,
                boxed(
                    Node {
                        width: Val::Px(SWITCH.0),
                        height: Val::Px(SWITCH.1),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    theme,
                    tone,
                ),
            ));
            ring(&mut track, theme, tone);
            track.with_children(|track| {
                track.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(left),
                        top: Val::Px(1.0),
                        width: Val::Px(SWITCH_THUMB),
                        height: Val::Px(SWITCH_THUMB),
                        ..default()
                    },
                    BackgroundColor(thumb),
                ));
            });
            styled_text(cell, word, caption(theme).color(ink).tracked_caps(TRACKING));
        }
        ControlModel::Radio { options, .. } => {
            let chosen = field.control.selected_index();
            let mut group = cell.spawn(Node {
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(0.0),
                ..default()
            });
            ring(&mut group, theme, tone);
            group.with_children(|group| {
                for (option, choice) in options.iter().enumerate() {
                    group
                        .spawn((
                            Button,
                            Interaction::default(),
                            FormNode {
                                part: Part::Choice { field: index, option },
                                ..node
                            },
                            Node {
                                min_height: Val::Px(ROW_MIN),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(theme.metrics.space_sm),
                                ..default()
                            },
                        ))
                        .with_children(|row| {
                            let mut radio = row.spawn((
                                boxed(
                                    Node {
                                        width: Val::Px(CHECK),
                                        height: Val::Px(CHECK),
                                        flex_shrink: 0.0,
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    theme,
                                    tone,
                                ),
                                BorderRadius::MAX,
                            ));
                            if chosen == Some(option) {
                                radio.with_children(|radio| {
                                    radio.spawn((
                                        Node {
                                            width: Val::Px(RADIO_DOT),
                                            height: Val::Px(RADIO_DOT),
                                            ..default()
                                        },
                                        BackgroundColor(tone.live),
                                        BorderRadius::MAX,
                                    ));
                                });
                            }
                            styled_text(row, choice.label.as_str(), text);
                        });
                }
            });
        }
        ControlModel::Select { .. } => select(cell, theme, tone, node, index, field, &form.state),
        ControlModel::Button { text: caption_text } => {
            let mut button = cell.spawn((
                Button,
                Interaction::default(),
                node,
                boxed(
                    Node {
                        padding: UiRect::axes(Val::Px(PAD.0), Val::Px(PAD.1)),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    theme,
                    tone,
                ),
            ));
            ring(&mut button, theme, tone);
            button.with_children(|button| {
                let style = caption(theme).tracked_caps(TRACKING);
                styled_text(button, caption_text.as_str(), style);
            });
        }
    }
}

/// A text or text-area box: the value (or its muted placeholder), one line
/// per row, and the caret while focused.
fn text_box(
    cell: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    tone: Tone,
    node: FormNode,
    field: &FieldModel,
    state: &FormState,
) {
    let text = body(theme).color(tone.text);
    let rows = match &field.control {
        ControlModel::TextArea { rows, .. } => usize::from((*rows).max(1)),
        _ => 1,
    };
    let value = field.control.display();
    let mut input = cell.spawn((
        Button,
        Interaction::default(),
        node,
        boxed(
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_width: Val::Px(0.0),
                height: Val::Px(rows as f32 * text.line_box() + 2.0 * (PAD.1 + 1.0)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(PAD.0), Val::Px(PAD.1)),
                overflow: Overflow::clip(),
                ..default()
            },
            theme,
            tone,
        ),
    ));
    ring(&mut input, theme, tone);
    input.with_children(|input| {
        if value.is_empty() && !tone.focused {
            styled_text(input, field.control.placeholder(), text.color(tone.muted));
            return;
        }
        for line in value.split('\n').take(rows) {
            styled_text(input, line, text);
        }
        if !tone.focused {
            return;
        }
        // The caret: a 1px highlight bar before the character it precedes.
        let before: Vec<char> = value.chars().take(state.caret).collect();
        let line = before.iter().filter(|c| **c == '\n').count();
        let column = before.iter().rev().take_while(|c| **c != '\n');
        let left: f32 = column
            .map(|c| glyph_advance_em(*c).unwrap_or_else(ch_em) * text.size)
            .sum();
        if line < rows {
            input.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(PAD.0 + left),
                    top: Val::Px(PAD.1 + line as f32 * text.line_box()),
                    width: Val::Px(1.0),
                    height: Val::Px(text.line_box()),
                    ..default()
                },
                BackgroundColor(theme.highlight),
            ));
        }
    });
}

/// The select trigger and, while open, its option list under it.
fn select(
    cell: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    tone: Tone,
    node: FormNode,
    index: usize,
    field: &FieldModel,
    state: &FormState,
) {
    let open = state.popup() == Some(index);
    let chosen = field.control.selected_index();
    let text = body(theme).color(tone.text);
    let (edge, chevron) = if open {
        (theme.highlight, theme.highlight)
    } else {
        (tone.edge, tone.muted)
    };
    cell.spawn(Node {
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        min_width: Val::Px(0.0),
        ..default()
    })
    .with_children(|wrap| {
        let mut trigger = wrap.spawn((
            Button,
            Interaction::default(),
            node,
            boxed(
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    padding: UiRect {
                        left: Val::Px(PAD.0),
                        right: Val::Px(SELECT_PAD_RIGHT),
                        top: Val::Px(PAD.1),
                        bottom: Val::Px(PAD.1),
                    },
                    ..default()
                },
                theme,
                Tone { edge, ..tone },
            ),
        ));
        ring(&mut trigger, theme, tone);
        trigger.with_children(|trigger| {
            // The label clips at the content edge; the chevron sits in the
            // padding, outside Bevy's content-box clip.
            trigger
                .spawn(Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    min_width: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_children(|label| {
                    let chosen_label = field.control.display();
                    if chosen_label.is_empty() {
                        let placeholder = field.control.placeholder();
                        styled_text(label, placeholder, text.color(tone.muted));
                    } else {
                        styled_text(label, chosen_label, text);
                    }
                });
            trigger.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    // Insets resolve against the padding box, CSS's
                    // background position against the border box.
                    right: Val::Px(CHEVRON_RIGHT - 1.0),
                    top: Val::Percent(50.0),
                    margin: UiRect::top(Val::Px(-CHEVRON_BOX.1 / 2.0)),
                    width: Val::Px(CHEVRON_BOX.0),
                    height: Val::Px(CHEVRON_BOX.1),
                    ..default()
                },
                ImageNode::new(CHEVRON_IMAGE).with_color(chevron),
            ));
        });
        if !open {
            return;
        }
        let rem = theme.metrics.rem;
        wrap.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::ZERO,
                right: Val::ZERO,
                margin: UiRect::top(Val::Px(POPUP_GAP)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::vertical(Val::Px(LIST_PAD_REM * rem)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme.background),
            BorderColor(theme.border),
            BoxShadow::new(
                Color::srgba(0.0, 0.0, 0.0, 0.5),
                Val::ZERO,
                Val::Px(8.0),
                Val::ZERO,
                Val::Px(24.0),
            ),
            // The list floats over the rows after its field.
            GlobalZIndex(1),
        ))
        .with_children(|list| {
            let options = field.control.options();
            for (option, choice) in options.iter().enumerate() {
                let highlighted = state.highlighted == Some(option);
                let ground = if highlighted {
                    mix_srgb(theme.background, theme.text, 0.07)
                } else {
                    theme.background
                };
                let ink = if chosen == Some(option) { theme.highlight } else { theme.text };
                list.spawn((
                    Button,
                    Interaction::default(),
                    FormNode {
                        part: Part::Choice { field: index, option },
                        ..node
                    },
                    Node {
                        padding: UiRect::axes(
                            Val::Px(OPTION_PAD_REM.0 * rem),
                            Val::Px(OPTION_PAD_REM.1 * rem),
                        ),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(ground),
                ))
                .with_children(|row| {
                    let style = TextStyle {
                        size: OPTION_SIZE,
                        ..TextStyle::body(theme)
                    };
                    styled_text(row, choice.label.as_str(), style.color(ink));
                    if highlighted {
                        // `box-shadow: inset 2px 0 0 var(--color-highlight)`.
                        row.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::ZERO,
                                top: Val::ZERO,
                                bottom: Val::ZERO,
                                width: Val::Px(2.0),
                                ..default()
                            },
                            BackgroundColor(theme.highlight),
                        ));
                    }
                });
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::event::Events;
    use bevy::input::keyboard::{Key as BevyKey, KeyCode};
    use bevy::input::ButtonState;
    use panel_kit_core::widgets::forms::{ChoiceModel, FieldValue, TextEntry};

    fn field(id: &str, control: ControlModel) -> FieldModel {
        FieldModel {
            id: id.into(),
            label: id.into(),
            control,
            disabled: false,
            note: String::new(),
        }
    }

    /// host, drain, load, region, owner (disabled).
    fn model() -> FormModel {
        let choices = |values: &[&str]| {
            values
                .iter()
                .map(|value| ChoiceModel {
                    value: (*value).into(),
                    label: value.to_uppercase(),
                })
                .collect()
        };
        let text = |value: &str| ControlModel::Text {
            value: value.into(),
            placeholder: String::new(),
            entry: TextEntry::Plain,
        };
        FormModel {
            fields: vec![
                field("host", text("pdx")),
                field("drain", ControlModel::Checkbox { checked: false }),
                field(
                    "load",
                    ControlModel::Slider { value: 50.0, min: 0.0, max: 100.0, step: 10.0 },
                ),
                field(
                    "region",
                    ControlModel::Select {
                        value: String::new(),
                        placeholder: "choose".into(),
                        options: choices(&["pdx", "iad"]),
                    },
                ),
                FieldModel {
                    disabled: true,
                    ..field("owner", ControlModel::Checkbox { checked: false })
                },
            ],
        }
    }

    fn app_with_form() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(FormPlugin);
        let parent = app.world_mut().spawn(Node::default()).id();
        let mut root = Entity::PLACEHOLDER;
        app.world_mut().commands().entity(parent).with_children(|children| {
            root = spawn(children, &BevyTheme::default(), Form::new(model()));
        });
        app.update();
        (app, root)
    }

    fn part(app: &mut App, part: Part) -> Entity {
        let mut nodes = app.world_mut().query::<(Entity, &FormNode)>();
        nodes
            .iter(app.world())
            .find_map(|(entity, node)| (node.part == part).then_some(entity))
            .unwrap_or_else(|| panic!("{part:?} is not painted"))
    }

    fn press(app: &mut App, target: Part) {
        let entity = part(app, target);
        *app.world_mut().get_mut::<Interaction>(entity).expect("pressable part") =
            Interaction::Pressed;
        app.update();
    }

    fn key(app: &mut App, key_code: KeyCode, logical_key: BevyKey) {
        app.world_mut().resource_mut::<Events<KeyboardInput>>().send(KeyboardInput {
            key_code,
            logical_key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }

    fn actions(app: &mut App) -> Vec<FormAction> {
        app.world_mut()
            .resource_mut::<Events<FormActionEvent>>()
            .drain()
            .map(|event| event.action)
            .collect()
    }

    fn form(app: &App, root: Entity) -> &Form {
        app.world().get::<Form>(root).expect("the form component")
    }

    fn texts(app: &mut App) -> Vec<String> {
        let mut query = app.world_mut().query::<&Text>();
        query.iter(app.world()).map(|text| text.0.clone()).collect()
    }

    #[test]
    fn pressing_the_checkbox_toggles_it_and_reports_the_change() {
        let (mut app, root) = app_with_form();
        press(&mut app, Part::Control(1));

        assert_eq!(
            actions(&mut app),
            [FormAction::Changed { id: "drain".into(), value: FieldValue::Bool(true) }]
        );
        assert_eq!(form(&app, root).state.focus, Some(1));
    }

    #[test]
    fn the_select_opens_on_press_and_an_option_press_chooses_and_closes() {
        let (mut app, root) = app_with_form();
        press(&mut app, Part::Control(3));
        assert_eq!(form(&app, root).state.popup(), Some(3));
        assert!(texts(&mut app).contains(&"IAD".to_owned()), "the open list paints its options");

        press(&mut app, Part::Choice { field: 3, option: 1 });
        assert_eq!(
            actions(&mut app),
            [FormAction::Changed { id: "region".into(), value: FieldValue::Choice("iad".into()) }]
        );
        assert_eq!(form(&app, root).state.popup(), None);
        let painted = texts(&mut app);
        assert!(painted.contains(&"IAD".to_owned()), "the trigger shows the choice: {painted:?}");
        assert!(!painted.contains(&"PDX".to_owned()), "the list is gone: {painted:?}");
    }

    #[test]
    fn a_focused_form_takes_bare_keys_and_leaves_shortcuts_to_the_workspace() {
        let (mut app, root) = app_with_form();
        key(&mut app, KeyCode::KeyM, BevyKey::Character("m".into()));
        assert!(app.world().resource::<ClaimedKeys>().0.is_empty(), "nothing is focused yet");

        press(&mut app, Part::Track(2));
        actions(&mut app);
        key(&mut app, KeyCode::ArrowRight, BevyKey::ArrowRight);
        assert_eq!(app.world().resource::<ClaimedKeys>().0, [0], "the arrow is the slider's");
        assert_eq!(actions(&mut app).len(), 1, "one step");

        key(&mut app, KeyCode::KeyM, BevyKey::Character("m".into()));
        assert_eq!(app.world().resource::<ClaimedKeys>().0, [0], "a stray key is swallowed");
        assert!(actions(&mut app).is_empty());

        key(&mut app, KeyCode::ControlLeft, BevyKey::Control);
        key(&mut app, KeyCode::KeyM, BevyKey::Character("m".into()));
        assert!(app.world().resource::<ClaimedKeys>().0.is_empty(), "ctrl-m is a shortcut");
        assert_eq!(form(&app, root).state.focus, Some(2));
    }

    #[test]
    fn typed_characters_edit_the_focused_text_field_and_keep_their_case() {
        let (mut app, root) = app_with_form();
        press(&mut app, Part::Control(0));
        key(&mut app, KeyCode::KeyX, BevyKey::Character("X".into()));
        assert!(
            actions(&mut app).contains(&FormAction::Changed {
                id: "host".into(),
                value: FieldValue::Text("pdxX".into())
            }),
            "the typed X kept its case"
        );
        key(&mut app, KeyCode::Backspace, BevyKey::Backspace);
        key(&mut app, KeyCode::Space, BevyKey::Space);

        assert_eq!(
            form(&app, root).model.fields[0].control.value(),
            Some(FieldValue::Text("pdx ".into()))
        );
        assert!(texts(&mut app).contains(&"pdx ".to_owned()), "the box repaints the value");
    }

    #[test]
    fn a_disabled_field_ignores_presses() {
        let (mut app, root) = app_with_form();
        press(&mut app, Part::Control(4));

        assert!(actions(&mut app).is_empty());
        assert_eq!(form(&app, root).state, FormState::default());
    }
}
