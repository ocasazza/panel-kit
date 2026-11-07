//! Header gestures: a press on a header sorts, a drag moves the column, and a
//! drag of the header's edge resizes it.
//!
//! Which order and width result is core's: the gesture ends in
//! `TableEvent::MoveColumn` or `TableEvent::Resize`, exactly as the web
//! header's `ondrop` and `onpointerup` raise them.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use panel_kit_core::widgets::data_table::{MoveTarget, TableEvent, MAX_WIDTH, MIN_WIDTH};

use super::{Action, Paint, TableControl, Tables};
use crate::theme::BevyTheme;

/// `.pk-dt-resize`: a 7px zone overhanging the header's right edge by 3px.
pub(super) const RESIZE_ZONE: f32 = 7.0;
pub(super) const RESIZE_OVERHANG: f32 = 3.0;
/// Pointer travel, in pixels, before a header press is a column drag.
const DRAG_THRESHOLD: f32 = 4.0;
/// The width a drag starts from when its header was never laid out.
const UNMEASURED_WIDTH: f32 = 120.0;

/// The gesture a pressed header or header edge started.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Drag {
    /// A header press: a click until the pointer travels.
    Header {
        column: String,
        x0: Option<f32>,
        moved: bool,
    },
    /// A header edge press, from the header's width at the press.
    Resize {
        column: String,
        basis: f32,
        handle: Entity,
        x0: Option<f32>,
        w0: Option<f32>,
    },
}

impl Drag {
    pub(super) fn header(column: String) -> Drag {
        Drag::Header {
            column,
            x0: None,
            moved: false,
        }
    }

    pub(super) fn resize(column: String, basis: f32, handle: Entity) -> Drag {
        Drag::Resize {
            column,
            basis,
            handle,
            x0: None,
            w0: None,
        }
    }
}

/// A header's resize zone; the flag paints the edge while its drag runs.
#[derive(Component, Clone, Copy, Debug)]
pub(super) struct ResizeHandle(pub bool);

/// Whether a column dragged from `source` lands before `over`: it does when
/// `over` is the earlier of the two, as the web header's `ondragover` decides.
pub(super) fn lands_before(order: &[String], source: &str, over: &str) -> Option<bool> {
    let at = |key: &str| order.iter().position(|column| column == key);
    Some(at(source)? > at(over)?)
}

impl Paint<'_> {
    /// `.pk-dt-resize`: the zone, with its `inset -1px` edge once hovered.
    pub(super) fn resize_handle(&self, cell: &mut ChildSpawnerCommands, column: &str, basis: f32) {
        let active = matches!(
            &self.instance.drag,
            Some(Drag::Resize { column: dragged, .. }) if dragged == column
        );
        cell.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                right: Val::Px(-RESIZE_OVERHANG),
                width: Val::Px(RESIZE_ZONE),
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BorderColor(if active { self.theme.text } else { Color::NONE }),
            Button,
            TableControl {
                table: self.table,
                action: Action::Resize(column.to_owned(), basis),
            },
            ResizeHandle(active),
        ));
    }

    /// `.pk-dt-drop-before` / `.pk-dt-drop-after`: a 2px highlight edge.
    pub(super) fn drop_mark(&self, cell: &mut ChildSpawnerCommands, before: bool) {
        let side = |at: f32| if before { (Val::Px(at), Val::Auto) } else { (Val::Auto, Val::Px(at)) };
        let (left, right) = side(0.0);
        cell.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                left,
                right,
                width: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(self.theme.highlight),
        ));
    }
}

/// Runs the gesture a header press started, to its release.
pub(super) fn drag_columns(
    mut tables: NonSendMut<Tables>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    parents: Query<&ChildOf>,
    nodes: Query<&ComputedNode>,
    controls: Query<(&Interaction, &TableControl)>,
) {
    let x = windows
        .iter()
        .next()
        .and_then(Window::cursor_position)
        .map(|at| at.x);
    let released = buttons.just_released(MouseButton::Left);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    for (table, instance) in tables.0.iter_mut() {
        let Some(drag) = instance.drag.take() else {
            continue;
        };
        match drag {
            Drag::Resize {
                column,
                basis,
                handle,
                x0,
                w0,
            } => {
                let x0 = x0.or(x);
                let w0 = w0.unwrap_or_else(|| {
                    parents
                        .get(handle)
                        .ok()
                        .and_then(|parent| nodes.get(parent.0).ok())
                        .map(|node| node.size().x * node.inverse_scale_factor())
                        .filter(|width| *width > 0.0)
                        .unwrap_or(if basis > 0.0 { basis } else { UNMEASURED_WIDTH })
                });
                let width = w0 + x.zip(x0).map_or(0.0, |(x, x0)| x - x0);
                if released {
                    instance.live = None;
                    instance.apply(TableEvent::Resize(column, f64::from(width)));
                    continue;
                }
                let live = (column.clone(), width.clamp(MIN_WIDTH as f32, MAX_WIDTH as f32).round());
                if instance.live.as_ref() != Some(&live) {
                    instance.live = Some(live);
                    instance.dirty = true;
                }
                instance.drag = Some(Drag::Resize {
                    column,
                    basis,
                    handle,
                    x0,
                    w0: Some(w0),
                });
            }
            Drag::Header { column, x0, mut moved } => {
                let x0 = x0.or(x);
                if !moved && x.zip(x0).is_some_and(|(x, x0)| (x - x0).abs() >= DRAG_THRESHOLD) {
                    moved = true;
                    instance.dirty = true;
                }
                if moved {
                    let order: Vec<String> = instance
                        .state
                        .visible(&instance.columns)
                        .iter()
                        .map(|c| c.id.clone())
                        .collect();
                    let over = controls.iter().find_map(|(interaction, control)| {
                        match (&control.action, interaction) {
                            (Action::Header(over), Interaction::Hovered | Interaction::Pressed)
                                if control.table == *table && *over != column =>
                            {
                                Some(over.clone())
                            }
                            _ => None,
                        }
                    });
                    if let Some(over) = over {
                        let mark = lands_before(&order, &column, &over).map(|before| (over, before));
                        if mark.is_some() && mark != instance.drop {
                            instance.drop = mark;
                            instance.dirty = true;
                        }
                    }
                }
                if !released {
                    instance.drag = Some(Drag::Header { column, x0, moved });
                    continue;
                }
                let drop = instance.drop.take();
                instance.dirty = true;
                if moved {
                    if let Some((target, before)) = drop {
                        let to = if before {
                            MoveTarget::Before(target)
                        } else {
                            MoveTarget::After(target)
                        };
                        instance.apply(TableEvent::MoveColumn(column, to));
                    }
                } else if instance.columns.iter().any(|c| c.id == column && c.sortable) {
                    instance.apply(TableEvent::Sort {
                        column,
                        append: shift,
                    });
                }
            }
        }
    }
}

/// Draws a resize zone's edge while the pointer is over it.
pub(super) fn hover_handles(
    theme: Res<BevyTheme>,
    mut handles: Query<(&Interaction, &ResizeHandle, &mut BorderColor), Changed<Interaction>>,
) {
    for (interaction, handle, mut edge) in &mut handles {
        let shown = handle.0 || *interaction != Interaction::None;
        edge.0 = if shown { theme.text } else { Color::NONE };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_column_lands_before_an_earlier_target_and_after_a_later_one() {
        let order: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        assert_eq!(lands_before(&order, "c", "a"), Some(true));
        assert_eq!(lands_before(&order, "a", "c"), Some(false));
        assert_eq!(lands_before(&order, "a", "z"), None);
    }
}

#[cfg(test)]
mod gestures {
    use super::super::testing::*;
    use super::super::Instance;
    use super::*;

    fn basis_of(app: &mut App, table: Entity, column: &str) -> f32 {
        let world = app.world_mut();
        let mut controls = world.query::<&TableControl>();
        controls
            .iter(world)
            .find_map(|control| match &control.action {
                Action::Resize(id, basis) if control.table == table && id == column => Some(*basis),
                _ => None,
            })
            .expect("the column has a resize handle")
    }

    fn header_basis(app: &mut App, table: Entity, column: &str) -> Val {
        let world = app.world_mut();
        let mut cells = world.query::<(&Node, &TableControl)>();
        cells
            .iter(world)
            .find_map(|(node, control)| match &control.action {
                Action::Header(id) if control.table == table && id == column => Some(node.flex_basis),
                _ => None,
            })
            .expect("the column has a header")
    }

    fn state_of<R>(app: &App, table: Entity, read: impl FnOnce(&Instance) -> R) -> R {
        with_instance(app, table, read)
    }

    #[test]
    fn the_resize_zone_is_seven_pixels_overhanging_the_edge_by_three() {
        let (mut app, table) = app();
        let handle = control(&mut app, table, |a| matches!(a, Action::Resize(id, _) if id == "node"));
        let node = app.world().get::<Node>(handle).expect("handle node");
        assert_eq!(node.position_type, PositionType::Absolute);
        assert_eq!((node.width, node.right), (Val::Px(7.0), Val::Px(-3.0)));
        assert_eq!((node.top, node.bottom), (Val::Px(0.0), Val::Px(0.0)));
    }

    #[test]
    fn dragging_a_header_edge_resizes_live_and_commits_on_release() {
        let (mut app, table) = app();
        let basis = basis_of(&mut app, table, "node");
        cursor_at(&mut app, 100.0);
        mouse_down(&mut app);
        press(&mut app, table, |a| matches!(a, Action::Resize(id, _) if id == "node"));
        cursor_at(&mut app, 160.0);
        app.update();
        let live = (basis + 60.0).clamp(MIN_WIDTH as f32, MAX_WIDTH as f32).round();
        assert_eq!(state_of(&app, table, |i| i.live.clone()), Some(("node".to_owned(), live)));
        assert_eq!(header_basis(&mut app, table, "node"), Val::Px(live), "the header paints the live width");
        assert!(state_of(&app, table, |i| i.state.widths.is_empty()), "nothing is committed mid-drag");
        mouse_up(&mut app);
        assert_eq!(state_of(&app, table, |i| i.state.widths.get("node").copied()), Some(live as u32));
        assert_eq!(state_of(&app, table, |i| i.live.clone()), None);
        assert_eq!(header_basis(&mut app, table, "node"), Val::Px(live), "the committed width stays painted");
    }

    #[test]
    fn a_resize_clamps_to_cores_minimum_width() {
        let (mut app, table) = app();
        cursor_at(&mut app, 300.0);
        mouse_down(&mut app);
        press(&mut app, table, |a| matches!(a, Action::Resize(id, _) if id == "detail"));
        cursor_at(&mut app, 0.0);
        mouse_up(&mut app);
        assert_eq!(
            state_of(&app, table, |i| i.state.widths.get("detail").copied()),
            Some(MIN_WIDTH)
        );
    }

    #[test]
    fn dragging_a_header_onto_another_marks_the_drop_and_moves_the_column() {
        let (mut app, table) = app();
        cursor_at(&mut app, 200.0);
        mouse_down(&mut app);
        press(&mut app, table, |a| matches!(a, Action::Header(id) if id == "detail"));
        cursor_at(&mut app, 120.0);
        hover(&mut app, table, |a| matches!(a, Action::Header(id) if id == "node"));
        app.update();
        assert_eq!(state_of(&app, table, |i| i.drop.clone()), Some(("node".to_owned(), true)));
        let highlight = BevyTheme::default().highlight;
        let world = app.world_mut();
        let mut marks = world.query::<(&Node, &BackgroundColor)>();
        assert_eq!(
            marks
                .iter(world)
                .filter(|(node, color)| node.width == Val::Px(2.0) && color.0 == highlight)
                .count(),
            1,
            "one 2px highlight edge marks the drop"
        );
        mouse_up(&mut app);
        assert_eq!(shown_columns(app.world(), table), Some(vec!["detail".to_owned(), "node".to_owned()]));
        assert_eq!(state_of(&app, table, |i| i.drop.clone()), None);
        assert!(
            state_of(&app, table, |i| i.state.sort.is_empty()),
            "a drag does not sort"
        );
    }

    #[test]
    fn a_column_dropped_after_a_later_header_lands_after_it() {
        let (mut app, table) = app();
        cursor_at(&mut app, 20.0);
        mouse_down(&mut app);
        press(&mut app, table, |a| matches!(a, Action::Header(id) if id == "node"));
        cursor_at(&mut app, 200.0);
        hover(&mut app, table, |a| matches!(a, Action::Header(id) if id == "detail"));
        app.update();
        assert_eq!(state_of(&app, table, |i| i.drop.clone()), Some(("detail".to_owned(), false)));
        mouse_up(&mut app);
        assert_eq!(shown_columns(app.world(), table), Some(vec!["detail".to_owned(), "node".to_owned()]));
    }

    #[test]
    fn a_drag_released_over_no_header_changes_nothing() {
        let (mut app, table) = app();
        cursor_at(&mut app, 200.0);
        mouse_down(&mut app);
        press(&mut app, table, |a| matches!(a, Action::Header(id) if id == "detail"));
        cursor_at(&mut app, 120.0);
        mouse_up(&mut app);
        assert_eq!(shown_columns(app.world(), table), Some(vec!["node".to_owned(), "detail".to_owned()]));
        assert!(state_of(&app, table, |i| i.state.sort.is_empty()));
    }

    #[test]
    fn a_press_released_in_place_sorts_and_a_second_release_does_not_repeat_it() {
        let (mut app, table) = app();
        cursor_at(&mut app, 50.0);
        mouse_down(&mut app);
        press(&mut app, table, |a| matches!(a, Action::Header(id) if id == "node"));
        assert!(state_of(&app, table, |i| i.state.sort.is_empty()), "the press alone does not sort");
        mouse_up(&mut app);
        assert_eq!(state_of(&app, table, |i| i.state.sort.len()), 1);
        mouse_up(&mut app);
        assert_eq!(state_of(&app, table, |i| i.state.sort.len()), 1);
    }

    #[test]
    fn the_resize_edge_shows_while_hovered() {
        let (mut app, table) = app();
        let handle = control(&mut app, table, |a| matches!(a, Action::Resize(id, _) if id == "node"));
        let theme = BevyTheme::default();
        assert_eq!(app.world().get::<BorderColor>(handle).map(|b| b.0), Some(Color::NONE));
        app.world_mut().entity_mut(handle).insert(Interaction::Hovered);
        app.update();
        assert_eq!(app.world().get::<BorderColor>(handle).map(|b| b.0), Some(theme.text));
        app.world_mut().entity_mut(handle).insert(Interaction::None);
        app.update();
        assert_eq!(app.world().get::<BorderColor>(handle).map(|b| b.0), Some(Color::NONE));
    }
}
