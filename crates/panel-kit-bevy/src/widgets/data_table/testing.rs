//! Fixtures the data-table tests share: a one-table app, injected pointer
//! and key input, and views of what a repaint spawned.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseWheel;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use panel_kit_core::widgets::data_table::ColumnKind;
use panel_kit_core::widgets::table::{ColumnWidth, TableCell, TableColumn, TableModel, TableRow, TextAlign};

use super::{Action, DataTable, DataTablePlugin, Instance, RowMark, TableControl, Tables};
pub(super) use super::{shown_columns, shown_rows};
use crate::theme::BevyTheme;

fn columns() -> Vec<TableColumn> {
    let column = |key: &str, value: u16| TableColumn {
        key: key.into(),
        title: key.into(),
        width: ColumnWidth::Fixed { value },
        align: TextAlign::Left,
        kind: ColumnKind::Text,
    };
    vec![column("node", 12), column("detail", 10)]
}

fn row(id: &str, detail: &str, tags: &[&str]) -> TableRow {
    TableRow {
        id: id.into(),
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        cells: vec![TableCell::Text(id.into()), TableCell::Text(detail.into())],
    }
}

/// Three untagged rows.
pub(super) fn model() -> TableModel {
    TableModel {
        columns: columns(),
        rows: vec![row("pdx-01", "leader", &[]), row("pdx-02", "ready", &[]), row("gfr-01", "stale", &[])],
    }
}

/// One row per `(id, tags)`.
pub(super) fn tagged(rows: &[(&str, &[&str])]) -> TableModel {
    TableModel {
        columns: columns(),
        rows: rows.iter().map(|(id, tags)| row(id, "x", tags)).collect(),
    }
}

/// `count` untagged rows, `r000`, `r001`, ….
pub(super) fn long(count: usize) -> TableModel {
    TableModel {
        columns: columns(),
        rows: (0..count).map(|at| row(&format!("r{at:03}"), "x", &[])).collect(),
    }
}

/// An app with the plugin, a primary window and `table` painted once.
pub(super) fn app_with(table: DataTable) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_event::<MouseWheel>()
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(BevyTheme::default())
        .add_plugins(DataTablePlugin);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    let table = app.world_mut().spawn((table, Node::default())).id();
    app.update();
    (app, table)
}

pub(super) fn app() -> (App, Entity) {
    app_with(DataTable::new(model()))
}

/// The painted control under `table` whose action matches.
pub(super) fn control(app: &mut App, table: Entity, matches: impl Fn(&Action) -> bool) -> Entity {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &TableControl)>();
    query
        .iter(world)
        .find(|(_, control)| control.table == table && matches(&control.action))
        .map(|(entity, _)| entity)
        .expect("a matching control is painted")
}

/// Press the first control under `table` whose action matches, and update.
pub(super) fn press(app: &mut App, table: Entity, matches: impl Fn(&Action) -> bool) {
    let target = control(app, table, matches);
    app.world_mut().entity_mut(target).insert(Interaction::Pressed);
    app.update();
}

/// Hover the first control under `table` whose action matches.
pub(super) fn hover(app: &mut App, table: Entity, matches: impl Fn(&Action) -> bool) {
    let target = control(app, table, matches);
    app.world_mut().entity_mut(target).insert(Interaction::Hovered);
}

/// Hold the left mouse button.
pub(super) fn mouse_down(app: &mut App) {
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
}

/// Release the left mouse button, update, and clear the frame's edges.
pub(super) fn mouse_up(app: &mut App) {
    let mut input = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    input.clear();
    input.release(MouseButton::Left);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
}

/// Put the pointer at logical `x`, and update.
pub(super) fn cursor_at(app: &mut App, x: f32) {
    let world = app.world_mut();
    let mut windows = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
    for mut window in windows.iter_mut(world) {
        window.set_cursor_position(Some(Vec2::new(x, 10.0)));
    }
}

/// Send one key press and update.
pub(super) fn key(app: &mut App, logical_key: Key) {
    app.world_mut().send_event(KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}

/// Type `text` a glyph at a time, in one update.
pub(super) fn type_text(app: &mut App, text: &str) {
    for glyph in text.chars() {
        app.world_mut().send_event(KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: Key::Character(glyph.to_string().into()),
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// Hold a keyboard key until [`release_key`].
pub(super) fn hold_key(app: &mut App, code: KeyCode) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(code);
}

pub(super) fn release_key(app: &mut App, code: KeyCode) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(code);
}

/// Read `table`'s instance.
pub(super) fn with_instance<R>(app: &App, table: Entity, read: impl FnOnce(&Instance) -> R) -> R {
    let tables = app.world().non_send_resource::<Tables>();
    read(tables.0.get(&table).expect("the table is adopted"))
}

pub(super) fn painted_texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut query = world.query::<&Text>();
    query.iter(world).map(|text| text.0.clone()).collect()
}

/// The ids of the data rows a repaint mounted, in paint order.
pub(super) fn mounted_ids(app: &mut App, table: Entity) -> Vec<String> {
    let world = app.world_mut();
    let mut query = world.query::<&RowMark>();
    let mut ids: Vec<String> = query
        .iter(world)
        .filter(|mark| mark.table == table)
        .map(|mark| mark.id.clone())
        .collect();
    ids.sort();
    ids
}
