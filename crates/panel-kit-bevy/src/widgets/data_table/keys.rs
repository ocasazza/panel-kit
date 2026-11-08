//! Keyboard row navigation and the cursor ring.
//!
//! The keys are the web grid's `on_key`: arrows, Home/End and the page keys
//! move core's cursor, space and Enter select it, ctrl-A selects every row,
//! escape clears, and `t` walks the cursor row's tag segments. Every one ends
//! in a `TableEvent`; core moves the cursor, the selection and the window.
//! Ctrl/Cmd-C emits the TSV of the selected rows (every shown row when none
//! is selected) as a `DataTableExport`, as the COPY button does.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use panel_kit_core::widgets::data_table::{
    ExportFormat, SelectMode, TableEvent, TableKey, TableState, DEFAULT_PAGE,
};

use super::{DataTableExport, Paint, TableControl, TableFocus, TableRows, Tables};

/// The table keys go to: the one last pressed, or none after a press away
/// from every table.
#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub struct GridFocus(pub Option<Entity>);

/// Held modifier keys.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Rows a page key moves: the rows the viewport shows, less one.
pub(super) fn page_rows(visible: f32, row_height: f32) -> usize {
    if visible > 0.0 && row_height > 0.0 {
        ((visible / row_height).floor() as usize).saturating_sub(1).max(1)
    } else {
        DEFAULT_PAGE
    }
}

/// The event `key` raises on a table in `state`, as the web grid's `on_key`
/// decides it. `labels_of` gives a row's tags.
pub(super) fn translate(
    key: &Key,
    mods: Mods,
    state: &TableState,
    labels_of: impl Fn(&str) -> Vec<String>,
    page: usize,
) -> Option<TableEvent> {
    if let Some((row, tag)) = &state.tag_focus {
        let labels = labels_of(row);
        let at = labels.iter().position(|label| label == tag);
        let handled = match key {
            Key::Escape => Some(TableEvent::TagFocus(None)),
            Key::Character(c) if c == "t" && !mods.shift => Some(TableEvent::TagFocus(None)),
            Key::ArrowRight | Key::ArrowLeft => at
                .and_then(|at| {
                    let to = if *key == Key::ArrowRight {
                        at.checked_add(1)
                    } else {
                        at.checked_sub(1)
                    };
                    labels.get(to?)
                })
                .map(|next| TableEvent::TagFocus(Some((row.clone(), next.clone())))),
            Key::Enter | Key::Space => Some(TableEvent::Tag {
                tag: tag.clone(),
                all: mods.shift,
            }),
            _ => None,
        };
        if handled.is_some() {
            return handled;
        }
    }
    if let (Key::Character(c), false, Some(row)) = (key, mods.ctrl || mods.alt, &state.cursor) {
        if c == "t" {
            return labels_of(row)
                .into_iter()
                .next()
                .map(|first| TableEvent::TagFocus(Some((row.clone(), first))));
        }
    }
    if *key == Key::Enter && state.tag_focus.is_none() {
        return state.cursor.clone().map(|id| TableEvent::Select {
            id,
            mode: SelectMode::Toggle,
        });
    }
    let table_key = match key {
        Key::ArrowDown => TableKey::Down,
        Key::ArrowUp => TableKey::Up,
        Key::Home => TableKey::Home,
        Key::End => TableKey::End,
        Key::PageDown => TableKey::PageDown,
        Key::PageUp => TableKey::PageUp,
        Key::Escape => TableKey::Escape,
        Key::Space => TableKey::Space,
        Key::Character(c) if c == " " => TableKey::Space,
        Key::Character(c) if mods.ctrl && c.eq_ignore_ascii_case("a") => TableKey::SelectAll,
        _ => return None,
    };
    Some(TableEvent::Key {
        key: table_key,
        shift: mods.shift,
        page,
    })
}

/// A press inside a table's rows gives it the keys; a press elsewhere takes
/// them away.
pub(super) fn focus_grid(
    buttons: Res<ButtonInput<MouseButton>>,
    mut grid: ResMut<GridFocus>,
    rows: Query<(&RelativeCursorPosition, &TableControl), With<TableRows>>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let over = rows
        .iter()
        .find(|(cursor, _)| cursor.mouse_over())
        .map(|(_, control)| control.table);
    if grid.0 != over {
        grid.0 = over;
    }
}

/// Applies the keys the focused table owns.
pub(super) fn navigate_rows(
    mut tables: NonSendMut<Tables>,
    grid: Res<GridFocus>,
    typing: Res<TableFocus>,
    buttons: Res<ButtonInput<KeyCode>>,
    mut input: EventReader<KeyboardInput>,
    mut exports: EventWriter<DataTableExport>,
    rows: Query<(&ComputedNode, &TableControl), With<TableRows>>,
) {
    let Some(table) = grid.0 else {
        input.clear();
        return;
    };
    if typing.0.as_ref().is_some_and(|(held, _)| *held == table) {
        input.clear();
        return;
    }
    let Some(instance) = tables.0.get_mut(&table) else {
        input.clear();
        return;
    };
    let mods = Mods {
        shift: buttons.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
        ctrl: buttons.any_pressed([
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
        ]),
        alt: buttons.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
    };
    let visible = rows
        .iter()
        .find(|(_, control)| control.table == table)
        .map_or(0.0, |(node, _)| node.size().y * node.inverse_scale_factor());
    let row_height = instance.state.view.row_height(instance.state.density) as f32;
    for key in input.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        let labels_of = |row: &str| -> Vec<String> {
            instance
                .data
                .row_of(row)
                .map(|at| instance.data.tags(at).map(str::to_owned).collect())
                .unwrap_or_default()
        };
        if mods.ctrl && matches!(&key.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("c")) {
            exports.write(DataTableExport {
                table,
                format: ExportFormat::Tsv,
                text: instance.export(ExportFormat::Tsv),
            });
            continue;
        }
        let page = page_rows(visible, row_height);
        let Some(event) = translate(&key.logical_key, mods, &instance.state, labels_of, page) else {
            continue;
        };
        let moves = matches!(event, TableEvent::Key { .. });
        instance.apply(event);
        instance.ring = true;
        instance.reveal |= moves;
    }
}

/// The 1px edge bands a row's cells draw: the `box-shadow: inset` rules for
/// a selected row and for the row the keyboard sits on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Bands {
    pub top: Option<Color>,
    pub bottom: Option<Color>,
    pub left: Option<Color>,
    pub right: Option<Color>,
}

/// The bands of one cell. `first` and `last` are the row's end cells, which
/// also draw the row's side edges.
pub(super) fn row_bands(selected: bool, ring: bool, first: bool, last: bool, highlight: Color, focus: Color) -> Bands {
    let ends = |color: Color| (first.then_some(color), last.then_some(color));
    match (selected, ring) {
        (false, false) => Bands::default(),
        (true, true) => Bands {
            top: Some(highlight),
            bottom: Some(focus),
            ..Bands::default()
        },
        (true, false) => {
            let (left, right) = ends(highlight);
            Bands {
                top: Some(highlight),
                left,
                right,
                ..Bands::default()
            }
        }
        (false, true) => {
            let (left, right) = ends(focus);
            Bands {
                top: Some(focus),
                bottom: Some(focus),
                left,
                right,
            }
        }
    }
}

impl Paint<'_> {
    /// One cell's selection and cursor bands, over its content.
    pub(super) fn bands(&self, cell: &mut ChildSpawnerCommands, selected: bool, ring: bool, first: bool, last: bool) {
        let bands = row_bands(selected, ring, first, last, self.theme.highlight, self.theme.focus_ring);
        let mut edge = |color: Option<Color>, node: Node| {
            if let Some(color) = color {
                cell.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        ..node
                    },
                    BackgroundColor(color),
                ));
            }
        };
        let (zero, one) = (Val::Px(0.0), Val::Px(1.0));
        edge(
            bands.top,
            Node {
                top: zero,
                left: zero,
                right: zero,
                height: one,
                ..default()
            },
        );
        edge(
            bands.bottom,
            Node {
                bottom: zero,
                left: zero,
                right: zero,
                height: one,
                ..default()
            },
        );
        edge(
            bands.left,
            Node {
                left: zero,
                top: zero,
                bottom: zero,
                width: one,
                ..default()
            },
        );
        edge(
            bands.right,
            Node {
                right: zero,
                top: zero,
                bottom: zero,
                width: one,
                ..default()
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nobody(_: &str) -> Vec<String> {
        Vec::new()
    }

    #[test]
    fn page_keys_move_the_viewport_less_one_row() {
        assert_eq!(page_rows(260.0, 26.0), 9);
        assert_eq!(page_rows(26.0, 26.0), 1);
        assert_eq!(page_rows(0.0, 26.0), DEFAULT_PAGE);
    }

    #[test]
    fn navigation_keys_translate_like_the_web_grid() {
        let state = TableState::default();
        let mods = Mods::default();
        let key = |key: Key| translate(&key, mods, &state, nobody, 7);
        assert_eq!(
            key(Key::PageDown),
            Some(TableEvent::Key {
                key: TableKey::PageDown,
                shift: false,
                page: 7
            })
        );
        assert_eq!(
            key(Key::Space),
            Some(TableEvent::Key {
                key: TableKey::Space,
                shift: false,
                page: 7
            })
        );
        assert_eq!(key(Key::Enter), None, "Enter needs a cursor row");
        assert_eq!(key(Key::F5), None);
    }

    #[test]
    fn enter_toggles_the_cursor_row_and_ctrl_a_selects_all() {
        let state = TableState {
            cursor: Some("r1".to_owned()),
            ..TableState::default()
        };
        let plain = Mods::default();
        assert_eq!(
            translate(&Key::Enter, plain, &state, nobody, 1),
            Some(TableEvent::Select {
                id: "r1".to_owned(),
                mode: SelectMode::Toggle
            })
        );
        let ctrl = Mods { ctrl: true, ..plain };
        assert_eq!(
            translate(&Key::Character("a".into()), ctrl, &state, nobody, 1),
            Some(TableEvent::Key {
                key: TableKey::SelectAll,
                shift: false,
                page: 1
            })
        );
        assert_eq!(translate(&Key::Character("a".into()), plain, &state, nobody, 1), None);
    }

    #[test]
    fn t_enters_tag_focus_and_arrows_walk_the_segments() {
        let labels = |_: &str| vec!["alpha".to_owned(), "beta".to_owned()];
        let mut state = TableState {
            cursor: Some("r1".to_owned()),
            ..TableState::default()
        };
        assert_eq!(
            translate(&Key::Character("t".into()), Mods::default(), &state, labels, 1),
            Some(TableEvent::TagFocus(Some(("r1".to_owned(), "alpha".to_owned()))))
        );
        state.tag_focus = Some(("r1".to_owned(), "alpha".to_owned()));
        assert_eq!(
            translate(&Key::ArrowRight, Mods::default(), &state, labels, 1),
            Some(TableEvent::TagFocus(Some(("r1".to_owned(), "beta".to_owned()))))
        );
        assert_eq!(translate(&Key::ArrowLeft, Mods::default(), &state, labels, 1), None);
        assert_eq!(
            translate(&Key::Space, Mods { shift: true, ..Mods::default() }, &state, labels, 1),
            Some(TableEvent::Tag {
                tag: "alpha".to_owned(),
                all: true
            })
        );
    }

    #[test]
    fn a_selected_row_and_a_ringed_row_draw_the_stylesheets_bands() {
        let (hi, ring) = (Color::srgb(0.0, 1.0, 0.0), Color::WHITE);
        assert_eq!(row_bands(false, false, true, true, hi, ring), Bands::default());
        let selected = row_bands(true, false, true, false, hi, ring);
        assert_eq!((selected.top, selected.left, selected.right, selected.bottom), (Some(hi), Some(hi), None, None));
        let ringed = row_bands(false, true, false, true, hi, ring);
        assert_eq!((ringed.top, ringed.bottom, ringed.left, ringed.right), (Some(ring), Some(ring), None, Some(ring)));
        let both = row_bands(true, true, true, true, hi, ring);
        assert_eq!(both, Bands { top: Some(hi), bottom: Some(ring), left: None, right: None });
    }
}

#[cfg(test)]
mod navigation {
    use panel_kit_core::widgets::data_table::Selectable;

    use super::super::testing::*;
    use super::super::{Action, DataTable, Field};
    use super::*;
    use crate::theme::BevyTheme;

    fn table(rows: usize, selectable: Selectable) -> DataTable {
        let mut table = DataTable::new(long(rows));
        table.defaults.selectable = selectable;
        table
    }

    fn cursor(app: &App, table: Entity) -> Option<String> {
        with_instance(app, table, |i| i.state.cursor.clone())
    }

    fn selection(app: &App, table: Entity) -> Vec<String> {
        with_instance(app, table, |i| i.state.selection.iter().cloned().collect())
    }

    /// Press a row the way a click does, then hand over to the keyboard.
    fn focus_row(app: &mut App, table: Entity, id: &str) {
        press(app, table, |a| matches!(a, Action::Row(row) if row == id));
    }

    /// Cells carrying the ring colour as a 1px absolute band.
    fn ring_bands(app: &mut App) -> usize {
        let ring = BevyTheme::default().focus_ring;
        let world = app.world_mut();
        let mut bands = world.query::<(&Node, &BackgroundColor)>();
        bands
            .iter(world)
            .filter(|(node, color)| node.position_type == PositionType::Absolute && color.0 == ring)
            .count()
    }

    #[test]
    fn arrow_keys_move_the_cursor_one_row_at_a_time() {
        let (mut app, table) = app_with(table(10, Selectable::None));
        focus_row(&mut app, table, "r003");
        key(&mut app, Key::ArrowDown);
        assert_eq!(cursor(&app, table).as_deref(), Some("r004"));
        key(&mut app, Key::ArrowUp);
        key(&mut app, Key::ArrowUp);
        assert_eq!(cursor(&app, table).as_deref(), Some("r002"));
    }

    #[test]
    fn home_and_end_jump_to_the_first_and_last_row() {
        let (mut app, table) = app_with(table(10, Selectable::None));
        focus_row(&mut app, table, "r003");
        key(&mut app, Key::End);
        assert_eq!(cursor(&app, table).as_deref(), Some("r009"));
        key(&mut app, Key::Home);
        assert_eq!(cursor(&app, table).as_deref(), Some("r000"));
    }

    #[test]
    fn page_keys_move_a_default_page_when_the_viewport_is_unmeasured() {
        let (mut app, table) = app_with(table(60, Selectable::None));
        focus_row(&mut app, table, "r005");
        key(&mut app, Key::PageDown);
        assert_eq!(cursor(&app, table).as_deref(), Some("r025"));
        key(&mut app, Key::PageUp);
        assert_eq!(cursor(&app, table).as_deref(), Some("r005"));
    }

    #[test]
    fn space_and_enter_select_the_cursor_row() {
        let (mut app, table) = app_with(table(10, Selectable::Multi));
        focus_row(&mut app, table, "r001");
        key(&mut app, Key::ArrowDown);
        key(&mut app, Key::Space);
        assert_eq!(selection(&app, table), ["r001", "r002"].map(String::from));
        key(&mut app, Key::Enter);
        assert_eq!(selection(&app, table), ["r001"].map(String::from), "enter toggles the cursor row off");
    }

    #[test]
    fn shift_arrow_extends_the_selection_and_escape_clears_it() {
        let (mut app, table) = app_with(table(10, Selectable::Multi));
        focus_row(&mut app, table, "r001");
        hold_key(&mut app, KeyCode::ShiftLeft);
        key(&mut app, Key::ArrowDown);
        key(&mut app, Key::ArrowDown);
        release_key(&mut app, KeyCode::ShiftLeft);
        assert_eq!(selection(&app, table), ["r001", "r002", "r003"].map(String::from));
        key(&mut app, Key::Escape);
        assert_eq!(selection(&app, table), Vec::<String>::new());
    }

    #[test]
    fn ctrl_a_selects_every_row() {
        let (mut app, table) = app_with(table(4, Selectable::Multi));
        focus_row(&mut app, table, "r000");
        hold_key(&mut app, KeyCode::ControlLeft);
        key(&mut app, Key::Character("a".into()));
        assert_eq!(selection(&app, table).len(), 4);
    }

    fn exports(app: &App) -> Vec<DataTableExport> {
        app.world()
            .resource::<Events<DataTableExport>>()
            .iter_current_update_events()
            .cloned()
            .collect()
    }

    #[test]
    fn ctrl_c_exports_the_selected_rows_as_tsv() {
        let (mut app, table) = app_with(table(10, Selectable::Multi));
        focus_row(&mut app, table, "r001");
        hold_key(&mut app, KeyCode::ShiftLeft);
        key(&mut app, Key::ArrowDown);
        release_key(&mut app, KeyCode::ShiftLeft);
        hold_key(&mut app, KeyCode::ControlLeft);
        key(&mut app, Key::Character("c".into()));
        let sent = exports(&app);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].table, table);
        assert_eq!(sent[0].format, ExportFormat::Tsv);
        assert_eq!(sent[0].text, "node\tdetail\nr001\tx\nr002\tx");
    }

    #[test]
    fn cmd_c_and_a_capital_c_copy_too() {
        let (mut app, table) = app_with(table(3, Selectable::Multi));
        focus_row(&mut app, table, "r000");
        hold_key(&mut app, KeyCode::SuperLeft);
        key(&mut app, Key::Character("C".into()));
        assert_eq!(exports(&app).len(), 1);
    }

    #[test]
    fn ctrl_c_without_a_selection_exports_every_shown_row() {
        let (mut app, table) = app_with(table(3, Selectable::Multi));
        focus_row(&mut app, table, "r000");
        key(&mut app, Key::Escape);
        assert_eq!(selection(&app, table), Vec::<String>::new());
        hold_key(&mut app, KeyCode::ControlLeft);
        key(&mut app, Key::Character("c".into()));
        let sent = exports(&app);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].text, "node\tdetail\nr000\tx\nr001\tx\nr002\tx");
    }

    #[test]
    fn a_plain_c_does_not_copy() {
        let (mut app, table) = app_with(table(3, Selectable::Multi));
        focus_row(&mut app, table, "r000");
        key(&mut app, Key::Character("c".into()));
        assert!(exports(&app).is_empty());
    }

    #[test]
    fn ctrl_c_while_typing_in_a_field_copies_nothing() {
        let (mut app, table) = app_with(table(3, Selectable::Multi));
        focus_row(&mut app, table, "r000");
        press(&mut app, table, |a| matches!(a, Action::Focus(Field::Quick)));
        hold_key(&mut app, KeyCode::ControlLeft);
        key(&mut app, Key::Character("c".into()));
        assert!(exports(&app).is_empty());
    }

    #[test]
    fn the_cursor_row_paints_a_focus_ring_only_while_the_keyboard_drives_it() {
        let (mut app, table) = app_with(table(10, Selectable::None));
        focus_row(&mut app, table, "r003");
        assert_eq!(ring_bands(&mut app), 0, "a click is not a keyboard cursor");
        key(&mut app, Key::ArrowDown);
        // top and bottom on both cells, plus the row's left and right edges
        assert_eq!(ring_bands(&mut app), 6);
        focus_row(&mut app, table, "r007");
        assert_eq!(ring_bands(&mut app), 0, "a press hands the cursor back to the pointer");
    }

    #[test]
    fn a_selected_cursor_row_keeps_its_highlight_top_and_gains_the_ring_bottom() {
        let (mut app, table) = app_with(table(5, Selectable::Multi));
        focus_row(&mut app, table, "r001");
        key(&mut app, Key::ArrowDown);
        key(&mut app, Key::ArrowUp);
        let theme = BevyTheme::default();
        let world = app.world_mut();
        let mut bands = world.query::<(&Node, &BackgroundColor)>();
        let count = |color: Color, bands: &mut QueryState<(&Node, &BackgroundColor)>, world: &mut World| {
            bands
                .iter(world)
                .filter(|(node, c)| node.position_type == PositionType::Absolute && c.0 == color)
                .count()
        };
        assert_eq!(count(theme.highlight, &mut bands, world), 2, "one highlight top band per cell");
        assert_eq!(count(theme.focus_ring, &mut bands, world), 2, "one ring bottom band per cell");
    }

    #[test]
    fn keys_do_nothing_before_any_press_and_after_a_press_away() {
        let (mut app, table) = app_with(table(10, Selectable::None));
        key(&mut app, Key::End);
        assert_eq!(cursor(&app, table), None, "no table has the keys yet");
        focus_row(&mut app, table, "r001");
        mouse_down(&mut app);
        app.update();
        assert_eq!(app.world().resource::<GridFocus>().0, None, "a press over no table takes the keys away");
        mouse_up(&mut app);
        key(&mut app, Key::End);
        assert_eq!(cursor(&app, table).as_deref(), Some("r001"));
    }

    #[test]
    fn typing_in_a_field_keeps_the_arrows_out_of_the_grid() {
        let (mut app, table) = app_with(table(10, Selectable::None));
        focus_row(&mut app, table, "r001");
        press(&mut app, table, |a| matches!(a, Action::Focus(Field::Quick)));
        key(&mut app, Key::ArrowDown);
        assert_eq!(cursor(&app, table).as_deref(), Some("r001"));
        key(&mut app, Key::Enter);
        key(&mut app, Key::ArrowDown);
        assert_eq!(cursor(&app, table).as_deref(), Some("r002"), "the grid has the arrows back once typing ends");
    }

    #[test]
    fn a_key_release_is_ignored() {
        let (mut app, table) = app_with(table(10, Selectable::None));
        focus_row(&mut app, table, "r001");
        app.world_mut().send_event(KeyboardInput {
            key_code: KeyCode::ArrowDown,
            logical_key: Key::ArrowDown,
            state: ButtonState::Released,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        assert_eq!(cursor(&app, table).as_deref(), Some("r001"));
    }
}
