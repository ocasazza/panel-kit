//! Bevy painter for panel-kit core's interactive data table.
//!
//! The web `DataTable` and this module paint the same core state: rows,
//! filters, sorting, grouping, column order and visibility, density, view
//! mode and selection all live in [`TableState`] and change only through
//! [`TableEvent::apply`]. This module paints the `.pk-dt` toolbar, active
//! filter chips, filter row, header and body to the web stylesheet's
//! geometry, and translates clicks, wheel and keyboard input into those
//! events. COPY and CSV emit [`DataTableExport`] for the host.
//!
//! Core's table state shares rows through `Rc`, so per-table state lives in
//! a main-thread [`NonSend`] map keyed by the [`DataTable`] entity; every
//! change repaints that table from it.

use std::collections::HashMap;

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use panel_kit_core::widgets::data_table::{
    empty_sentence, export_view, tally, BuildOptions, ColumnFilter, ColumnKind, DataColumnSpec,
    Density, Entry, ExportFormat, MoveTarget, QueryMode, SelectMode, Selectable, SortDir,
    TableData, TableDefaults, TableEvent, TableState, View, ViewMode,
};
use panel_kit_core::widgets::table::{TableCell, TableModel};

use super::{styled_text, TextStyle};
use crate::theme::BevyTheme;

/// `.pk-dt` gap between toolbar, chips and grid (`.3rem`).
const DT_GAP_REM: f32 = 0.3;
/// `.pk-dt-bar` gap (`.5rem`) and bottom padding.
const BAR_GAP_REM: f32 = 0.5;
const BAR_PAD_BOTTOM: f32 = 4.0;
/// `.pk-btn`: 11px tracked caps on a 1.4 line, `1px 6px` padding.
const BTN_SIZE: f32 = 11.0;
const BTN_LINE: f32 = 1.4;
const BTN_TRACKING: f32 = 0.06;
const BTN_PAD: (f32, f32) = (6.0, 1.0);
const BTN_COMPACT_PAD_X: f32 = 4.0;
/// `.pk-dt input`: `1px 6px` padding on a 1.4 line in the table type.
const INPUT_PAD: (f32, f32) = (6.0, 1.0);
/// `.pk-dt-text input`: 12px, `0 4px` padding.
const FILTER_SIZE: f32 = 12.0;
const FILTER_PAD_X: f32 = 4.0;
/// `.pk-dt-quick`: grows from 12ch, never below 8ch.
const QUICK_BASIS_CH: f32 = 12.0;
const QUICK_MIN_CH: f32 = 8.0;
/// Flex weight of the quick filter against the tally's 1.
const QUICK_GROW: f32 = 1.0e6;
/// `.pk-dt-th` header type: 11px, bold, `.06em`, uppercase.
const HEAD_SIZE: f32 = 11.0;
/// `.pk-dt-mark` / `.pk-dt-filtered-mark` / `.pk-dt-n`.
const MARK_SIZE: f32 = 9.0;
/// `.pk-dt-menu-body` and its rows.
const MENU_PAD: f32 = 4.0;
const MENU_GAP: f32 = 2.0;
const MENU_ROW_GAP: f32 = 4.0;
const MENU_MIN_CH: f32 = 14.0;
const CHECK_SIZE: f32 = 11.0;
const CHECK_GAP: f32 = 6.0;
/// `.pk-dt-filters .pk-dt-th` padding.
const FILTER_CELL_PAD: (f32, f32) = (4.0, 2.0);
/// `.pk-dt-active` padding and gap; `.pk-dt-active-chip` gap.
const ACTIVE_PAD_Y: f32 = 4.0;
const ACTIVE_GAP: f32 = 6.0;
const CHIP_GAP: f32 = 4.0;
/// `.pk-dt-card`: padding, gaps, line heights and corner radius.
const CARD_RADIUS: f32 = 4.0;
const CARD_META_GAP: f32 = 12.0;
const CARD_FIELD_GAP: f32 = 4.0;
/// `.pk-dt-empty .pk-btn` margin.
const EMPTY_BUTTON_GAP: f32 = 8.0;

/// Density-dependent `.pk-dt` variables: `--dt-head` and `--dt-pad`.
fn dt_head(density: Density) -> f32 {
    match density {
        Density::Normal => 24.0,
        Density::Compact => 20.0,
    }
}

fn dt_pad(density: Density) -> (f32, f32) {
    match density {
        Density::Normal => (8.0, 3.0),
        Density::Compact => (6.0, 1.0),
    }
}

/// A data table over a [`TableModel`]; [`DataTablePlugin`] paints it into
/// this entity and keeps it in step with its core state.
#[derive(Component, Clone, Debug)]
pub struct DataTable {
    /// The rows and columns.
    pub model: TableModel,
    /// Declared sort, density, grouping, selection mode and view.
    pub defaults: TableDefaults,
    /// Quick-filter placeholder.
    pub placeholder: String,
}

impl DataTable {
    /// A table over `model` with the default state and placeholder.
    pub fn new(model: TableModel) -> Self {
        Self {
            model,
            defaults: TableDefaults::default(),
            placeholder: "filter rows".to_owned(),
        }
    }
}

/// The rows a table's COPY (TSV) or CSV produced; the host puts them on
/// the clipboard or in a file.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct DataTableExport {
    /// The table.
    pub table: Entity,
    /// TSV for COPY, CSV for CSV.
    pub format: ExportFormat,
    /// The exported text.
    pub text: String,
}

/// The text field holding keyboard focus, if any.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct TableFocus(pub Option<(Entity, Field)>);

/// A text field a table paints.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    /// The quick filter.
    Quick,
    /// A text column's filter.
    Text(String),
    /// A number column's lower bound.
    Min(String),
    /// A number column's upper bound.
    Max(String),
}

/// A menu a table opens below its button.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Menu {
    Columns,
    Group,
    Values(String),
}

/// What pressing a painted control does.
#[derive(Clone, Debug)]
enum Action {
    Apply(TableEvent),
    Sort(String),
    Row(String),
    Toggle(Menu),
    Focus(Field),
    Pick(String, String),
    Export(ExportFormat),
}

/// A pressable part of a painted table.
#[derive(Component, Clone, Debug)]
struct TableControl {
    table: Entity,
    action: Action,
}

/// The rows' scroll viewport of a painted table.
#[derive(Component, Clone, Copy, Debug)]
pub struct TableRows;

/// One table's core state and the text being typed into its fields.
struct Instance {
    columns: Vec<DataColumnSpec>,
    data: TableData,
    model: TableModel,
    placeholder: String,
    state: TableState,
    view: View,
    open: Option<Menu>,
    inputs: HashMap<Field, String>,
    scroll: Vec2,
    dirty: bool,
}

impl Instance {
    fn new(table: &DataTable) -> Self {
        let columns = table.model.data_columns();
        let data = table.model.data();
        let state = TableState::new(&columns, table.defaults.clone());
        let mut instance = Self {
            columns,
            data,
            model: table.model.clone(),
            placeholder: table.placeholder.clone(),
            state,
            view: View::default(),
            open: None,
            inputs: HashMap::new(),
            scroll: Vec2::ZERO,
            dirty: true,
        };
        instance.rebuild();
        instance
    }

    /// Apply one event through core and rebuild the view.
    fn apply(&mut self, event: TableEvent) {
        event.apply(&mut self.state, &self.columns, &self.view, &self.data);
        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.state.sync(&self.columns);
        let options = BuildOptions {
            mode: QueryMode::Client,
            tags: false,
        };
        self.view = View::build(&self.columns, &self.data, &self.state, options);
        self.dirty = true;
    }

    fn export(&self, format: ExportFormat) -> String {
        let selected_only = format == ExportFormat::Tsv;
        export_view(
            &self.columns,
            &self.data,
            &self.view,
            &self.state,
            &self.state.selection,
            selected_only,
            format,
        )
    }

    /// The text a field shows: what is being typed, else the applied value.
    fn field_text(&self, field: &Field) -> String {
        if let Some(typed) = self.inputs.get(field) {
            return typed.clone();
        }
        match field {
            Field::Quick => self.state.quick.clone(),
            Field::Text(column) => match self.state.filters.get(column) {
                Some(ColumnFilter::Text(text)) => text.clone(),
                _ => String::new(),
            },
            Field::Min(column) | Field::Max(column) => {
                let bound = match self.state.filters.get(column) {
                    Some(ColumnFilter::Range { min, max }) => {
                        if matches!(field, Field::Min(_)) {
                            *min
                        } else {
                            *max
                        }
                    }
                    _ => None,
                };
                bound.map(|n| n.to_string()).unwrap_or_default()
            }
        }
    }

    /// The event a field's text applies.
    fn field_event(&self, field: &Field, text: &str) -> TableEvent {
        let range = |column: &str| match self.state.filters.get(column) {
            Some(ColumnFilter::Range { min, max }) => (*min, *max),
            _ => (None, None),
        };
        match field {
            Field::Quick => TableEvent::Quick(text.to_owned()),
            Field::Text(column) => TableEvent::Filter(column.clone(), ColumnFilter::text(text)),
            Field::Min(column) => {
                let (_, max) = range(column);
                TableEvent::Filter(column.clone(), ColumnFilter::range(text.trim().parse().ok(), max))
            }
            Field::Max(column) => {
                let (min, _) = range(column);
                TableEvent::Filter(column.clone(), ColumnFilter::range(min, text.trim().parse().ok()))
            }
        }
    }
}

/// Main-thread per-table state, keyed by the [`DataTable`] entity.
#[derive(Default)]
struct Tables(HashMap<Entity, Instance>);

/// Paints [`DataTable`] entities and routes their input through core.
pub struct DataTablePlugin;

impl Plugin for DataTablePlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send_resource(Tables::default())
            .init_resource::<TableFocus>()
            .add_event::<DataTableExport>()
            .add_systems(Startup, insert_chevron_image)
            .add_systems(
                Update,
                (
                    adopt_tables,
                    press_controls,
                    type_into_fields,
                    scroll_rows,
                    repaint_tables,
                )
                    .chain(),
            );
    }
}

/// The group select's chevron (`.pk-dt-select::after`), tinted with the
/// text colour.
const CHEVRON_IMAGE: Handle<Image> = bevy::asset::weak_handle!("2d6f0b8e-93c4-4a1d-b7e5-6c0a9f3e1d24");
/// Chevron box, in CSS px.
const CHEVRON_BOX: (f32, f32) = (10.0, 7.0);
/// The CSS `clip-path` polygon inside [`CHEVRON_BOX`].
const CHEVRON_POLYGON: [(f32, f32); 6] =
    [(0.0, 1.5), (1.5, 0.0), (5.0, 3.5), (8.5, 0.0), (10.0, 1.5), (5.0, 6.5)];
/// `right: 9px` from the select's border box.
const CHEVRON_RIGHT: f32 = 9.0;
const CHEVRON_TEXELS: u32 = 4;
/// `.pk-dt-select select { padding-right: 22px }`: room for the chevron.
const SELECT_PAD_RIGHT: f32 = 22.0;

fn insert_chevron_image(images: Option<ResMut<Assets<Image>>>) {
    if let Some(mut images) = images {
        images.insert(&CHEVRON_IMAGE, chevron_image());
    }
}

fn chevron_image() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::ImageSampler;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    const SAMPLES: u32 = 4;
    let (width, height) = (
        CHEVRON_BOX.0 as u32 * CHEVRON_TEXELS,
        CHEVRON_BOX.1 as u32 * CHEVRON_TEXELS,
    );
    let polygon = CHEVRON_POLYGON.map(|(x, y)| Vec2::new(x, y));
    // Even-odd ray cast; the chevron polygon is concave.
    let covers = |p: Vec2| {
        let mut inside = false;
        for (index, a) in polygon.iter().enumerate() {
            let b = polygon[(index + 1) % polygon.len()];
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
                inside = !inside;
            }
        }
        inside
    };
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let mut covered = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let p = Vec2::new(
                        x as f32 + (sx as f32 + 0.5) / SAMPLES as f32,
                        y as f32 + (sy as f32 + 0.5) / SAMPLES as f32,
                    ) / CHEVRON_TEXELS as f32;
                    if covers(p) {
                        covered += 1;
                    }
                }
            }
            let alpha = (covered * 255 / (SAMPLES * SAMPLES)) as u8;
            data.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image
}

/// Spawn a data table over `model` with the default state. The painter
/// fills it on the next update.
pub fn spawn(parent: &mut ChildSpawnerCommands, model: &TableModel) -> Entity {
    parent
        .spawn((
            DataTable::new(model.clone()),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                min_height: Val::ZERO,
                min_width: Val::ZERO,
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .id()
}

fn adopt_tables(
    mut tables: NonSendMut<Tables>,
    added: Query<(Entity, &DataTable), Added<DataTable>>,
    mut removed: RemovedComponents<DataTable>,
    theme: Res<BevyTheme>,
) {
    for entity in removed.read() {
        tables.0.remove(&entity);
    }
    for (entity, table) in &added {
        tables.0.insert(entity, Instance::new(table));
    }
    if theme.is_changed() {
        for instance in tables.0.values_mut() {
            instance.dirty = true;
        }
    }
}

fn press_controls(
    mut tables: NonSendMut<Tables>,
    mut focus: ResMut<TableFocus>,
    keys: Res<ButtonInput<KeyCode>>,
    controls: Query<(&Interaction, &TableControl), Changed<Interaction>>,
    mut exports: EventWriter<DataTableExport>,
) {
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    for (interaction, control) in &controls {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(instance) = tables.0.get_mut(&control.table) else {
            continue;
        };
        match &control.action {
            Action::Apply(event) => {
                instance.open = None;
                instance.apply(event.clone());
            }
            Action::Sort(column) => instance.apply(TableEvent::Sort {
                column: column.clone(),
                append: shift,
            }),
            Action::Row(id) => {
                instance.apply(TableEvent::Cursor(id.clone()));
                if instance.state.selectable != Selectable::None {
                    let mode = if shift {
                        SelectMode::Range
                    } else {
                        SelectMode::Toggle
                    };
                    instance.apply(TableEvent::Select {
                        id: id.clone(),
                        mode,
                    });
                }
            }
            Action::Toggle(menu) => {
                instance.open = (instance.open.as_ref() != Some(menu)).then(|| menu.clone());
                instance.dirty = true;
            }
            Action::Focus(field) => {
                focus.0 = Some((control.table, field.clone()));
                instance.dirty = true;
            }
            Action::Pick(column, value) => {
                let mut picked = match instance.state.filters.get(column) {
                    Some(ColumnFilter::Values(values)) => values.clone(),
                    _ => Vec::new(),
                };
                match picked.iter().position(|v| v == value) {
                    Some(at) => {
                        picked.remove(at);
                    }
                    None => picked.push(value.clone()),
                }
                let filter = ColumnFilter::values(picked.iter().map(String::as_str));
                instance.apply(TableEvent::Filter(column.clone(), filter));
            }
            Action::Export(format) => {
                exports.write(DataTableExport {
                    table: control.table,
                    format: *format,
                    text: instance.export(*format),
                });
            }
        }
        if !matches!(control.action, Action::Focus(_)) {
            if let Some((table, _)) = &focus.0 {
                if *table == control.table {
                    focus.0 = None;
                    instance.dirty = true;
                }
            }
        }
    }
}

fn type_into_fields(
    mut tables: NonSendMut<Tables>,
    mut focus: ResMut<TableFocus>,
    mut keys: EventReader<KeyboardInput>,
) {
    let Some((table, field)) = focus.0.clone() else {
        keys.clear();
        return;
    };
    let Some(instance) = tables.0.get_mut(&table) else {
        focus.0 = None;
        return;
    };
    for key in keys.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        let mut text = instance.field_text(&field);
        match &key.logical_key {
            Key::Character(typed) => text.push_str(typed),
            Key::Space => text.push(' '),
            Key::Backspace => {
                text.pop();
            }
            Key::Escape | Key::Enter | Key::Tab => {
                focus.0 = None;
                instance.dirty = true;
                return;
            }
            _ => continue,
        }
        instance.inputs.insert(field.clone(), text.clone());
        let event = instance.field_event(&field, &text);
        instance.apply(event);
    }
}

fn scroll_rows(
    mut tables: NonSendMut<Tables>,
    mut wheels: EventReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    mut rows: Query<(
        &RelativeCursorPosition,
        &mut ScrollPosition,
        &ComputedNode,
        &Children,
        &TableControl,
    ), With<TableRows>>,
    content: Query<&ComputedNode>,
) {
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    for wheel in wheels.read() {
        let step = match wheel.unit {
            MouseScrollUnit::Line => 19.5,
            MouseScrollUnit::Pixel => 1.0,
        };
        let (mut dx, mut dy) = (-wheel.x * step, -wheel.y * step);
        if shift {
            (dx, dy) = (dy, dx);
        }
        for (cursor, mut position, viewport, children, control) in &mut rows {
            if !cursor.mouse_over() {
                continue;
            }
            let scale = viewport.inverse_scale_factor();
            let inner = children
                .iter()
                .filter_map(|child| content.get(child).ok())
                .fold(Vec2::ZERO, |size, node| size.max(node.size() * scale));
            let view = viewport.size() * scale;
            let max = (inner - view).max(Vec2::ZERO);
            position.offset_x = (position.offset_x + dx).clamp(0.0, max.x);
            position.offset_y = (position.offset_y + dy).clamp(0.0, max.y);
            if let Some(instance) = tables.0.get_mut(&control.table) {
                instance.scroll = Vec2::new(position.offset_x, position.offset_y);
            }
        }
    }
}

fn repaint_tables(
    mut tables: NonSendMut<Tables>,
    focus: Res<TableFocus>,
    theme: Res<BevyTheme>,
    mut commands: Commands,
) {
    for (entity, instance) in tables.0.iter_mut() {
        if !instance.dirty {
            continue;
        }
        instance.dirty = false;
        let focused = focus
            .0
            .as_ref()
            .filter(|(table, _)| table == entity)
            .map(|(_, field)| field.clone());
        let Ok(mut root) = commands.get_entity(*entity) else {
            continue;
        };
        root.despawn_related::<Children>();
        let table = *entity;
        let paint = Paint {
            table,
            theme: &theme,
            instance,
            focused: focused.as_ref(),
        };
        root.with_children(|parent| paint.table(parent));
    }
}

/// One repaint of one table.
struct Paint<'a> {
    table: Entity,
    theme: &'a BevyTheme,
    instance: &'a Instance,
    focused: Option<&'a Field>,
}

impl Paint<'_> {
    fn density(&self) -> Density {
        self.instance.state.density
    }

    fn body(&self) -> TextStyle {
        TextStyle::body(self.theme)
    }

    fn control(&self, action: Action) -> (Button, TableControl) {
        (
            Button,
            TableControl {
                table: self.table,
                action,
            },
        )
    }

    fn ch(&self, size: f32) -> f32 {
        crate::fonts::ch_em() * size
    }

    fn table(&self, parent: &mut ChildSpawnerCommands) {
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                min_height: Val::ZERO,
                min_width: Val::ZERO,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(self.theme.metrics.rem(DT_GAP_REM)),
                ..default()
            })
            .with_children(|dt| {
                self.bar(dt);
                let active = self.instance.state.active_filters(&self.instance.columns);
                if !active.is_empty() {
                    self.active(dt, &active);
                }
                self.grid(dt);
            });
    }

    /// `.pk-btn`; `pressed` latches the highlight edge.
    fn button(
        &self,
        parent: &mut ChildSpawnerCommands,
        label: &str,
        action: Option<Action>,
        pressed: bool,
        compact: bool,
    ) -> Entity {
        let theme = self.theme;
        let ink = if pressed { theme.highlight } else { theme.text_muted };
        let edge = match (&action, pressed) {
            (None, _) => theme.divider,
            (_, true) => theme.highlight,
            _ => theme.border,
        };
        let pad_x = if compact { BTN_COMPACT_PAD_X } else { BTN_PAD.0 };
        let node = Node {
            padding: UiRect::axes(Val::Px(pad_x), Val::Px(BTN_PAD.1)),
            border: UiRect::all(Val::Px(1.0)),
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            ..default()
        };
        let mut button = parent.spawn((node, BackgroundColor(theme.background), BorderColor(edge)));
        if let Some(action) = action {
            button.insert(self.control(action));
        }
        button.with_children(|inner| {
            styled_text(inner, label, self.button_style().color(ink));
        });
        button.id()
    }

    fn button_style(&self) -> TextStyle {
        TextStyle {
            size: BTN_SIZE,
            line_height: BTN_LINE,
            ..self.body()
        }
        .tracked_caps(BTN_TRACKING)
    }

    /// A `.pk-dt input`: its text or placeholder, a caret while focused.
    fn input(
        &self,
        parent: &mut ChildSpawnerCommands,
        field: Field,
        placeholder: &str,
        style: TextStyle,
        pad: (f32, f32),
        node: Node,
    ) {
        let theme = self.theme;
        let text = self.instance.field_text(&field);
        let focused = self.focused == Some(&field);
        let mut box_node = node;
        box_node.padding = UiRect::axes(Val::Px(pad.0), Val::Px(pad.1));
        box_node.border = UiRect::all(Val::Px(1.0));
        box_node.align_items = AlignItems::Center;
        box_node.overflow = Overflow::clip();
        let mut field_box = parent.spawn((
            box_node,
            BackgroundColor(theme.background),
            BorderColor(theme.divider),
            self.control(Action::Focus(field)),
        ));
        if focused {
            field_box.insert(Outline::new(Val::Px(1.0), Val::Px(1.0), theme.focus_ring));
        }
        field_box.with_children(|inner| {
            if text.is_empty() && !focused {
                styled_text(inner, placeholder, style.color(theme.text_muted));
            } else {
                styled_text(inner, text, style.color(theme.text));
            }
            if focused {
                inner.spawn((
                    Node {
                        width: Val::Px(1.0),
                        height: Val::Px(style.size),
                        ..default()
                    },
                    BackgroundColor(theme.highlight),
                ));
            }
        });
    }

    fn bar(&self, dt: &mut ChildSpawnerCommands) {
        let theme = self.theme;
        let state = &self.instance.state;
        let columns = &self.instance.columns;
        dt.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(theme.metrics.rem(BAR_GAP_REM)),
                row_gap: Val::Px(theme.metrics.rem(BAR_GAP_REM)),
                padding: UiRect::bottom(Val::Px(BAR_PAD_BOTTOM)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BorderColor(theme.divider),
        ))
        .with_children(|bar| {
            let table_size = theme.metrics.body_size;
            let input_style = TextStyle {
                line_height: BTN_LINE,
                ..self.body()
            };
            self.input(
                bar,
                Field::Quick,
                &self.instance.placeholder,
                input_style,
                INPUT_PAD,
                Node {
                    flex_grow: QUICK_GROW,
                    flex_shrink: 1.0,
                    flex_basis: Val::Px(QUICK_BASIS_CH * self.ch(table_size)),
                    min_width: Val::Px(QUICK_MIN_CH * self.ch(table_size)),
                    ..default()
                },
            );
            let filterable = columns.iter().any(|column| column.filterable);
            if filterable {
                let count = state.filters.len();
                let id = self.button(
                    bar,
                    "filters",
                    Some(Action::Apply(TableEvent::ToggleFilters)),
                    state.filters_open,
                    false,
                );
                if count > 0 {
                    bar.commands().entity(id).with_children(|inner| {
                        inner.spawn(Node {
                            width: Val::Px(4.0),
                            ..default()
                        });
                        styled_text(inner, count.to_string(), self.button_style().color(theme.highlight));
                    });
                }
            }
            if state.filtering() {
                self.button(bar, "clear", Some(Action::Apply(TableEvent::Clear)), false, false);
            }
            self.menu_button(bar, "columns", Menu::Columns, false);
            self.group(bar);
            self.button(
                bar,
                state.density.as_str(),
                Some(Action::Apply(TableEvent::ToggleDensity)),
                state.density == Density::Compact,
                false,
            );
            self.button(
                bar,
                state.view.label(),
                Some(Action::Apply(TableEvent::View(state.view.next()))),
                false,
                false,
            );
            self.button(bar, "copy", Some(Action::Export(ExportFormat::Tsv)), false, false);
            self.button(bar, "csv", Some(Action::Export(ExportFormat::Csv)), false, false);
            let label = tally(&self.instance.view, state.selection.len());
            // `.pk-dt-count { margin-left: auto }`. Taffy drops `gap` on a line
            // with an auto margin, so the tally grows instead; QUICK_GROW leaves
            // it ~0px of a line it shares with the quick filter, as in CSS.
            bar.spawn(Node {
                flex_grow: 1.0,
                justify_content: JustifyContent::FlexEnd,
                ..default()
            })
            .with_children(|tally| {
                styled_text(
                    tally,
                    label,
                    TextStyle {
                        size: BTN_SIZE,
                        ..self.body()
                    }
                    .tracked_caps(BTN_TRACKING)
                    .color(theme.text_muted),
                );
            });
        });
    }

    /// A `<details>` menu: its summary button and, when open, the body below.
    fn menu_button(&self, bar: &mut ChildSpawnerCommands, label: &str, menu: Menu, plain: bool) {
        let open = self.instance.open.as_ref() == Some(&menu);
        bar.spawn(Node {
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|holder| {
            if plain {
                self.plain_summary(holder, label, menu.clone(), open);
            } else {
                self.button(holder, label, Some(Action::Toggle(menu.clone())), open, false);
            }
            if open {
                self.menu_body(holder, &menu);
            }
        });
    }

    /// `.pk-dt-enum > summary`: a filter-row button in 12px, no caps.
    fn plain_summary(&self, parent: &mut ChildSpawnerCommands, label: &str, menu: Menu, open: bool) {
        let theme = self.theme;
        let ink = if open { theme.highlight } else { theme.text_muted };
        let edge = if open { theme.highlight } else { theme.border };
        parent
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(BTN_PAD.0), Val::Px(BTN_PAD.1)),
                    border: UiRect::all(Val::Px(1.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(theme.background),
                BorderColor(edge),
                self.control(Action::Toggle(menu)),
            ))
            .with_children(|inner| {
                let style = TextStyle {
                    size: FILTER_SIZE,
                    line_height: BTN_LINE,
                    ..self.body()
                };
                styled_text(inner, label, style.color(ink));
            });
    }

    fn menu_body(&self, holder: &mut ChildSpawnerCommands, menu: &Menu) {
        let theme = self.theme;
        holder
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(100.0),
                    left: Val::ZERO,
                    margin: UiRect::top(Val::Px(1.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(MENU_GAP),
                    min_width: Val::Px(MENU_MIN_CH * self.ch(theme.metrics.body_size)),
                    padding: UiRect::all(Val::Px(MENU_PAD)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme.background),
                BorderColor(theme.divider),
                ZIndex(5),
            ))
            .with_children(|body| match menu {
                Menu::Columns => self.columns_menu(body),
                Menu::Group => self.group_menu(body),
                Menu::Values(column) => self.values_menu(body, column),
            });
    }

    /// `.pk-dt-check`: an 11px checkbox (filled with the highlight when
    /// checked) and its label.
    fn check(&self, parent: &mut ChildSpawnerCommands, label: &str, checked: bool, action: Option<Action>) {
        let theme = self.theme;
        let mut row = parent.spawn(Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(CHECK_GAP),
            ..default()
        });
        if let Some(action) = action {
            row.insert(self.control(action));
        }
        row.with_children(|check| {
            check.spawn((
                Node {
                    width: Val::Px(CHECK_SIZE),
                    height: Val::Px(CHECK_SIZE),
                    border: UiRect::all(Val::Px(1.0)),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(if checked { theme.highlight } else { theme.background }),
                BorderColor(if checked { theme.highlight } else { theme.border }),
            ));
            styled_text(check, label, self.body());
        });
    }

    fn columns_menu(&self, body: &mut ChildSpawnerCommands) {
        let state = &self.instance.state;
        let ordered = state.ordered(&self.instance.columns);
        let last = ordered.len().saturating_sub(1);
        for (index, column) in ordered.iter().enumerate() {
            body.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(MENU_ROW_GAP),
                ..default()
            })
            .with_children(|row| {
                let toggle = column
                    .hideable
                    .then(|| Action::Apply(TableEvent::ToggleColumn(column.id.clone())));
                self.check(row, &column.title, state.is_visible(column), toggle);
                let left = (index > 0)
                    .then(|| Action::Apply(TableEvent::MoveColumn(column.id.clone(), MoveTarget::Left)));
                self.button(row, "◂", left, false, true);
                let right = (index < last)
                    .then(|| Action::Apply(TableEvent::MoveColumn(column.id.clone(), MoveTarget::Right)));
                self.button(row, "▸", right, false, true);
            });
        }
        self.button(body, "reset layout", Some(Action::Apply(TableEvent::Reset)), false, false);
    }

    fn group(&self, bar: &mut ChildSpawnerCommands) {
        let theme = self.theme;
        let state = &self.instance.state;
        let current = state
            .group_by
            .as_deref()
            .and_then(|id| self.instance.columns.iter().find(|column| column.id == id))
            .map_or("none", |column| column.title.as_str())
            .to_owned();
        bar.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|field| {
            styled_text(
                field,
                "group",
                TextStyle {
                    size: BTN_SIZE,
                    ..self.body()
                }
                .tracked_caps(BTN_TRACKING)
                .color(theme.text_muted),
            );
            field
                .spawn(Node::default())
                .with_children(|holder| {
                    let open = self.instance.open.as_ref() == Some(&Menu::Group);
                    // A native select is as wide as its widest option.
                    let widest = self
                        .instance
                        .columns
                        .iter()
                        .map(|column| column.title.chars().count())
                        .chain(std::iter::once("none".len()))
                        .max()
                        .unwrap_or_default();
                    holder
                        .spawn((
                            Node {
                                padding: UiRect {
                                    left: Val::Px(INPUT_PAD.0),
                                    right: Val::Px(SELECT_PAD_RIGHT),
                                    top: Val::Px(INPUT_PAD.1),
                                    bottom: Val::Px(INPUT_PAD.1),
                                },
                                border: UiRect::all(Val::Px(1.0)),
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(theme.background),
                            BorderColor(if open { theme.highlight } else { theme.divider }),
                            self.control(Action::Toggle(Menu::Group)),
                        ))
                        .with_children(|select| {
                            let style = TextStyle {
                                size: BTN_SIZE,
                                line_height: BTN_LINE,
                                ..self.body()
                            };
                            select
                                .spawn(Node {
                                    min_width: Val::Px(widest as f32 * self.ch(BTN_SIZE)),
                                    ..default()
                                })
                                .with_children(|value| {
                                    styled_text(value, current, style);
                                });
                            select.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    // Insets resolve against the padding box,
                                    // CSS's against the wrapper's border box.
                                    right: Val::Px(CHEVRON_RIGHT - 1.0),
                                    top: Val::Percent(50.0),
                                    margin: UiRect::top(Val::Px(-CHEVRON_BOX.1 / 2.0)),
                                    width: Val::Px(CHEVRON_BOX.0),
                                    height: Val::Px(CHEVRON_BOX.1),
                                    ..default()
                                },
                                ImageNode::new(CHEVRON_IMAGE).with_color(theme.text),
                            ));
                        });
                    if open {
                        self.menu_body(holder, &Menu::Group);
                    }
                });
        });
    }

    fn group_menu(&self, body: &mut ChildSpawnerCommands) {
        let state = &self.instance.state;
        let options = std::iter::once((None, "none".to_owned())).chain(
            state
                .ordered(&self.instance.columns)
                .into_iter()
                .map(|column| (Some(column.id.clone()), column.title.clone())),
        );
        for (id, title) in options {
            let picked = state.group_by == id;
            self.check(body, &title, picked, Some(Action::Apply(TableEvent::Group(id))));
        }
    }

    fn values_menu(&self, body: &mut ChildSpawnerCommands, column: &str) {
        let picked = match self.instance.state.filters.get(column) {
            Some(ColumnFilter::Values(values)) => values.clone(),
            _ => Vec::new(),
        };
        let mut offered = self.instance.view.options.get(column).cloned().unwrap_or_default();
        for value in &picked {
            if !offered.contains(value) {
                offered.push(value.clone());
            }
        }
        if offered.is_empty() {
            styled_text(body, "no values", self.body().color(self.theme.text_muted));
        }
        for value in offered {
            let on = picked.contains(&value);
            self.check(body, &value, on, Some(Action::Pick(column.to_owned(), value.clone())));
        }
    }

    /// `.pk-dt-active`: one chip per active filter, then CLEAR ALL.
    fn active(&self, dt: &mut ChildSpawnerCommands, active: &[panel_kit_core::widgets::data_table::ActiveFilter]) {
        let theme = self.theme;
        dt.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(ACTIVE_GAP),
                row_gap: Val::Px(ACTIVE_GAP),
                padding: UiRect::vertical(Val::Px(ACTIVE_PAD_Y)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BorderColor(theme.divider),
        ))
        .with_children(|row| {
            let label_style = TextStyle {
                size: BTN_SIZE,
                ..self.body()
            }
            .tracked_caps(BTN_TRACKING)
            .color(theme.text_muted);
            for filter in active {
                row.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(CHIP_GAP),
                    ..default()
                })
                .with_children(|chip| {
                    styled_text(chip, filter.label.clone(), label_style);
                    self.button(chip, &format!("{} ×", filter.value), Some(Action::Apply(filter.remove.clone())), false, true);
                });
            }
            self.button(row, "clear all", Some(Action::Apply(TableEvent::Clear)), false, false);
        });
    }

    /// Visible columns with their CSS widths: a fixed-layout table keeps
    /// each declared width and shares the slack in proportion.
    fn widths(&self) -> Vec<(&DataColumnSpec, f32)> {
        let size = self.theme.metrics.body_size;
        self.instance
            .state
            .visible(&self.instance.columns)
            .into_iter()
            .map(|column| {
                let px = self
                    .instance
                    .state
                    .widths
                    .get(&column.id)
                    .map(|px| *px as f32)
                    .or_else(|| column.width.as_deref().and_then(|width| css_width(width, size, self.theme)))
                    .unwrap_or(0.0);
                (column, px)
            })
            .collect()
    }

    fn cell_node(&self, basis: f32, pad: (f32, f32), align: panel_kit_core::widgets::data_table::Align) -> Node {
        let justify = match align {
            panel_kit_core::widgets::data_table::Align::Left => JustifyContent::FlexStart,
            panel_kit_core::widgets::data_table::Align::Center => JustifyContent::Center,
            panel_kit_core::widgets::data_table::Align::Right => JustifyContent::FlexEnd,
        };
        Node {
            flex_basis: Val::Px(basis),
            flex_grow: if basis > 0.0 { basis } else { 1.0 },
            flex_shrink: 0.0,
            min_width: Val::ZERO,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: justify,
            padding: UiRect::axes(Val::Px(pad.0), Val::Px(pad.1)),
            border: UiRect::bottom(Val::Px(1.0)),
            overflow: Overflow::clip(),
            ..default()
        }
    }

    fn grid(&self, dt: &mut ChildSpawnerCommands) {
        let theme = self.theme;
        let widths = self.widths();
        let total: f32 = widths.iter().map(|(_, px)| px).sum();
        dt.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_shrink: 1.0,
                flex_basis: Val::ZERO,
                min_height: Val::ZERO,
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll(),
                ..default()
            },
            ScrollPosition {
                offset_x: self.instance.scroll.x,
                offset_y: self.instance.scroll.y,
            },
            RelativeCursorPosition::default(),
            TableRows,
            TableControl {
                table: self.table,
                action: Action::Toggle(Menu::Columns),
            },
        ))
        .with_children(|scroll| {
            scroll
                .spawn(Node {
                    width: Val::Percent(100.0),
                    min_width: Val::Px(total),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|grid| {
                    self.header(grid, &widths);
                    if self.instance.state.filters_open {
                        self.filter_row(grid, &widths);
                    }
                    self.rows(grid, &widths);
                });
            let view = &self.instance.view;
            if view.given == 0 {
                self.empty(scroll, "no rows", false);
            } else if view.total == 0 {
                self.empty(scroll, &empty_sentence(&self.instance.state.tag_filter), true);
            }
        });
        let _ = theme;
    }

    fn header(&self, grid: &mut ChildSpawnerCommands, widths: &[(&DataColumnSpec, f32)]) {
        let theme = self.theme;
        let state = &self.instance.state;
        let head = dt_head(self.density());
        let pad = dt_pad(self.density());
        grid.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            for (column, basis) in widths {
                let sorted = state.direction(&column.id);
                let ink = if sorted.is_some() || state.filters.contains_key(&column.id) {
                    theme.text
                } else {
                    theme.text_muted
                };
                let style = TextStyle {
                    size: HEAD_SIZE,
                    line_height: head / HEAD_SIZE,
                    ..self.body()
                }
                .bold()
                .tracked_caps(BTN_TRACKING)
                .color(ink);
                let mut cell = row.spawn((
                    Node {
                        height: Val::Px(head + 1.0),
                        ..self.cell_node(*basis, (pad.0, 0.0), column.align)
                    },
                    BackgroundColor(theme.background),
                    BorderColor(theme.divider),
                ));
                if column.sortable {
                    cell.insert(self.control(Action::Sort(column.id.clone())));
                }
                cell.with_children(|cell| {
                    cell.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(4.0),
                        min_width: Val::ZERO,
                        overflow: Overflow::clip(),
                        ..default()
                    })
                    .with_children(|label| {
                        styled_text(label, column.title.clone(), style);
                        let mark = |size: f32, color: Color| TextStyle {
                            size,
                            line_height: head / size,
                            ..self.body()
                        }
                        .color(color);
                        if state.filters.contains_key(&column.id) {
                            styled_text(label, "●", mark(MARK_SIZE, theme.highlight));
                        }
                        let arrow = match sorted {
                            Some(SortDir::Asc) => Some("▲"),
                            Some(SortDir::Desc) => Some("▼"),
                            None => None,
                        };
                        if let Some(arrow) = arrow {
                            styled_text(label, arrow, mark(MARK_SIZE, theme.text));
                        }
                        if let Some(rank) = state.sort_rank(&column.id).filter(|_| state.sort.len() > 1) {
                            styled_text(label, rank.to_string(), TextStyle { bold: false, ..style }.color(theme.text_muted));
                        }
                    });
                });
            }
        });
    }

    fn filter_row(&self, grid: &mut ChildSpawnerCommands, widths: &[(&DataColumnSpec, f32)]) {
        let theme = self.theme;
        grid.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            for (column, basis) in widths {
                row.spawn((
                    self.cell_node(*basis, FILTER_CELL_PAD, column.align),
                    BackgroundColor(theme.background),
                    BorderColor(theme.divider),
                ))
                .with_children(|cell| {
                    if column.filterable {
                        self.filter_control(cell, column);
                    }
                });
            }
        });
    }

    fn filter_control(&self, cell: &mut ChildSpawnerCommands, column: &DataColumnSpec) {
        let style = TextStyle {
            size: FILTER_SIZE,
            line_height: BTN_LINE,
            ..self.body()
        };
        let grow = Node {
            flex_grow: 1.0,
            flex_basis: Val::Px(4.0 * self.ch(FILTER_SIZE)),
            min_width: Val::Px(3.0 * self.ch(FILTER_SIZE)),
            ..default()
        };
        match column.kind {
            ColumnKind::Enum => {
                let summary = match self.instance.state.filters.get(&column.id) {
                    Some(ColumnFilter::Values(values)) if !values.is_empty() => values.join(", "),
                    _ => "any".to_owned(),
                };
                cell.spawn(Node {
                    flex_grow: 1.0,
                    min_width: Val::ZERO,
                    ..default()
                })
                .with_children(|holder| {
                    let menu = Menu::Values(column.id.clone());
                    let open = self.instance.open.as_ref() == Some(&menu);
                    self.plain_summary(holder, &summary, menu.clone(), open);
                    if open {
                        self.menu_body(holder, &menu);
                    }
                });
            }
            ColumnKind::Number => {
                cell.spawn(Node {
                    flex_grow: 1.0,
                    column_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|range| {
                    self.input(range, Field::Min(column.id.clone()), "min", style, (FILTER_PAD_X, 0.0), grow.clone());
                    self.input(range, Field::Max(column.id.clone()), "max", style, (FILTER_PAD_X, 0.0), grow.clone());
                });
            }
            ColumnKind::Text => {
                self.input(cell, Field::Text(column.id.clone()), "filter", style, (FILTER_PAD_X, 0.0), grow);
            }
        }
    }

    fn rows(&self, grid: &mut ChildSpawnerCommands, widths: &[(&DataColumnSpec, f32)]) {
        let view = &self.instance.view;
        let state = &self.instance.state;
        let entry_h = state.view.row_height(state.density) as f32;
        for entry in &view.entries {
            match entry {
                Entry::Group { value, count, folded } => self.group_row(grid, value, *count, *folded, entry_h),
                Entry::Row(row) => match state.view {
                    ViewMode::Rows => self.data_row(grid, *row, widths),
                    mode => self.card_row(grid, *row, widths, mode, entry_h),
                },
            }
        }
    }

    fn group_row(&self, grid: &mut ChildSpawnerCommands, value: &str, count: usize, folded: bool, height: f32) {
        let theme = self.theme;
        let pad = dt_pad(self.density());
        let shown = if value.is_empty() { "—" } else { value };
        grid.spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(height),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(pad.0), Val::Px(pad.1)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(theme.surface),
            BorderColor(theme.divider),
            self.control(Action::Apply(TableEvent::ToggleGroup(value.to_owned()))),
        ))
        .with_children(|fold| {
            let bold = self.body().bold().color(theme.text_muted);
            styled_text(fold, if folded { "▸" } else { "▾" }, self.body().color(theme.text_muted));
            styled_text(fold, shown, bold);
            styled_text(fold, count.to_string(), self.body().color(theme.text_muted));
        });
    }

    fn row_cells(&self, row: usize) -> Vec<(&DataColumnSpec, Option<&TableCell>, String)> {
        let columns = &self.instance.columns;
        self.instance
            .state
            .visible(columns)
            .into_iter()
            .map(|column| {
                let at = columns.iter().position(|c| c.id == column.id).unwrap_or(0);
                let cell = self.instance.model.rows.get(row).and_then(|r| r.cells.get(at));
                (column, cell, self.instance.data.text(row, at).into_owned())
            })
            .collect()
    }

    fn data_row(&self, grid: &mut ChildSpawnerCommands, row: usize, widths: &[(&DataColumnSpec, f32)]) {
        let theme = self.theme;
        let pad = dt_pad(self.density());
        let id = self.instance.data.id(row).to_owned();
        let selected = self.instance.state.selection.contains(&id);
        let cells = self.row_cells(row);
        grid.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_shrink: 0.0,
                ..default()
            },
            self.control(Action::Row(id)),
        ))
        .with_children(|line| {
            for ((column, cell, text), (_, basis)) in cells.iter().zip(widths) {
                line.spawn((
                    self.cell_node(*basis, pad, column.align),
                    BackgroundColor(if selected { theme.surface } else { theme.background }),
                    BorderColor(if selected { theme.highlight } else { theme.divider }),
                ))
                .with_children(|slot| self.cell(slot, *cell, text));
            }
        });
    }

    /// One cell's content: a model cell by kind, else its text.
    fn cell(&self, slot: &mut ChildSpawnerCommands, cell: Option<&TableCell>, text: &str) {
        let theme = self.theme;
        let body = self.body();
        match cell {
            Some(TableCell::Status { label, color }) => {
                let tint = Color::srgb_u8(color.0, color.1, color.2);
                styled_text(slot, "●", body.color(tint));
                styled_text(slot, format!(" {label}"), body);
            }
            Some(TableCell::Meter { ratio, text, color }) => {
                let tint = color.map_or(theme.highlight, |c| Color::srgb_u8(c.0, c.1, c.2));
                slot.spawn(Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(theme.metrics.rem(0.5)),
                    ..default()
                })
                .with_children(|meter| {
                    meter
                        .spawn((
                            Node {
                                flex_grow: 1.0,
                                min_width: Val::ZERO,
                                height: Val::Px(theme.metrics.rem(0.55)),
                                border: UiRect::all(Val::Px(1.0)),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BackgroundColor(theme.background),
                            BorderColor(theme.border),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                Node {
                                    width: Val::Percent((ratio.clamp(0.0, 1.0) * 100.0) as f32),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(tint),
                            ));
                        });
                    if !text.is_empty() {
                        styled_text(meter, text.clone(), body.color(theme.text_muted));
                    }
                });
            }
            _ => {
                if !text.is_empty() {
                    styled_text(slot, text, body);
                }
            }
        }
    }

    fn card_row(&self, grid: &mut ChildSpawnerCommands, row: usize, widths: &[(&DataColumnSpec, f32)], mode: ViewMode, height: f32) {
        let theme = self.theme;
        let compact = mode == ViewMode::CompactCards;
        let id = self.instance.data.id(row).to_owned();
        let cells = self.row_cells(row);
        let tight = self.density() == Density::Compact;
        let (pad, gap, line) = match (tight, compact) {
            (false, true) => ((8.0, 4.0), 8.0, 18.0),
            (false, false) => ((8.0, 4.0), 2.0, 18.0),
            (true, true) => ((6.0, 2.0), 6.0, 16.0),
            (true, false) => ((6.0, 2.0), 1.0, 16.0),
        };
        let meta_line = if tight { 14.0 } else { 16.0 };
        let total: f32 = widths.iter().map(|(_, px)| px).sum();
        grid.spawn((
            Node {
                width: Val::Percent(100.0),
                min_width: Val::Px(total),
                height: Val::Px(height),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(theme.background),
            BorderColor(theme.divider),
            self.control(Action::Row(id)),
        ))
        .with_children(|td| {
            td.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(height - 1.0),
                    flex_direction: if compact { FlexDirection::Row } else { FlexDirection::Column },
                    justify_content: JustifyContent::Center,
                    align_items: if compact { AlignItems::Center } else { AlignItems::Stretch },
                    row_gap: Val::Px(gap),
                    column_gap: Val::Px(gap),
                    padding: UiRect::axes(Val::Px(pad.0), Val::Px(pad.1)),
                    border: UiRect::all(Val::Px(1.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(theme.surface),
                BorderColor(theme.divider),
                BorderRadius::all(Val::Px(CARD_RADIUS)),
            ))
            .with_children(|card| {
                let body = TextStyle {
                    line_height: line / theme.metrics.body_size,
                    ..self.body()
                };
                if compact {
                    for (_, _, text) in &cells {
                        styled_text(card, text.clone(), body);
                    }
                    return;
                }
                if let Some((_, cell, text)) = cells.first() {
                    card.spawn(Node::default()).with_children(|title| match cell {
                        Some(cell @ TableCell::Status { .. }) => self.cell(title, Some(cell), text),
                        _ => {
                            styled_text(title, text.clone(), body);
                        }
                    });
                }
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(CARD_META_GAP),
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_children(|meta| {
                    let value = TextStyle {
                        line_height: meta_line / theme.metrics.body_size,
                        ..self.body()
                    };
                    let label = TextStyle {
                        size: BTN_SIZE,
                        line_height: meta_line / BTN_SIZE,
                        ..self.body()
                    }
                    .tracked_caps(BTN_TRACKING)
                    .color(theme.text_muted);
                    for (column, _, text) in cells.iter().skip(1) {
                        meta.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(CARD_FIELD_GAP),
                            ..default()
                        })
                        .with_children(|field| {
                            styled_text(field, column.title.clone(), label);
                            styled_text(field, text.clone(), value);
                        });
                    }
                });
            });
        });
    }

    fn empty(&self, scroll: &mut ChildSpawnerCommands, sentence: &str, clear: bool) {
        let pad = dt_pad(self.density());
        scroll
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(pad.0), Val::Px(pad.1)),
                column_gap: Val::Px(EMPTY_BUTTON_GAP),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|empty| {
                styled_text(empty, sentence, self.body().color(self.theme.text_muted));
                if clear {
                    self.button(empty, "clear", Some(Action::Apply(TableEvent::Clear)), false, false);
                }
            });
    }
}

/// A CSS length (`12ch`, `6rem`, `80px`) in logical pixels at font `size`.
fn css_width(width: &str, size: f32, theme: &BevyTheme) -> Option<f32> {
    let width = width.trim();
    let (number, unit) = width
        .find(|c: char| c.is_ascii_alphabetic())
        .map_or((width, "px"), |at| width.split_at(at));
    let number: f32 = number.trim().parse().ok()?;
    match unit {
        "ch" => Some(number * crate::fonts::ch_em() * size),
        "rem" => Some(theme.metrics.rem(number)),
        "em" => Some(number * size),
        "px" => Some(number),
        _ => None,
    }
}

/// Every row id the table currently shows, in order (for tests and hosts).
pub fn shown_rows(world: &World, table: Entity) -> Option<Vec<String>> {
    let tables = world.get_non_send_resource::<Tables>()?;
    let instance = tables.0.get(&table)?;
    Some(
        instance
            .view
            .rows
            .iter()
            .map(|row| instance.data.id(*row).to_owned())
            .collect(),
    )
}

/// The ids of a table's visible columns, in order.
pub fn shown_columns(world: &World, table: Entity) -> Option<Vec<String>> {
    let tables = world.get_non_send_resource::<Tables>()?;
    let instance = tables.0.get(&table)?;
    Some(
        instance
            .state
            .visible(&instance.columns)
            .into_iter()
            .map(|column| column.id.clone())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::keyboard::NativeKey;
    use panel_kit_core::widgets::data_table::ColumnKind as Kind;
    use panel_kit_core::widgets::table::{ColumnWidth, TableColumn, TableRow, TextAlign};

    fn model() -> TableModel {
        let column = |key: &str, value: u16| TableColumn {
            key: key.into(),
            title: key.into(),
            width: ColumnWidth::Fixed { value },
            align: TextAlign::Left,
            kind: Kind::Text,
        };
        let row = |id: &str, detail: &str| TableRow {
            id: id.into(),
            tags: Vec::new(),
            cells: vec![TableCell::Text(id.into()), TableCell::Text(detail.into())],
        };
        TableModel {
            columns: vec![column("node", 12), column("detail", 10)],
            rows: vec![row("pdx-01", "leader"), row("pdx-02", "ready"), row("gfr-01", "stale")],
        }
    }

    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_event::<KeyboardInput>()
            .add_event::<MouseWheel>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(BevyTheme::default())
            .add_plugins(DataTablePlugin);
        let table = app.world_mut().spawn((DataTable::new(model()), Node::default())).id();
        app.update();
        (app, table)
    }

    /// Press the first control under `table` whose action matches.
    fn press(app: &mut App, table: Entity, matches: impl Fn(&Action) -> bool) {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &TableControl)>();
        let target = query
            .iter(world)
            .find(|(_, control)| control.table == table && matches(&control.action))
            .map(|(entity, _)| entity)
            .expect("a matching control is painted");
        world.entity_mut(target).insert(Interaction::Pressed);
        app.update();
    }

    fn type_text(app: &mut App, text: &str) {
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
        let _ = NativeKey::Unidentified;
        app.update();
    }

    fn painted_texts(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        let mut query = world.query::<&Text>();
        query.iter(world).map(|text| text.0.clone()).collect()
    }

    #[test]
    fn typing_in_the_quick_filter_narrows_the_painted_rows() {
        let (mut app, table) = app();
        press(&mut app, table, |action| matches!(action, Action::Focus(Field::Quick)));
        type_text(&mut app, "pdx");
        assert_eq!(
            shown_rows(app.world(), table),
            Some(vec!["pdx-01".to_owned(), "pdx-02".to_owned()])
        );
        let texts = painted_texts(&mut app);
        assert!(texts.iter().any(|text| text == "pdx"), "the field shows what was typed");
        assert!(!texts.iter().any(|text| text == "gfr-01"), "the filtered row is not painted");
    }

    #[test]
    fn the_columns_menu_hides_a_column() {
        let (mut app, table) = app();
        press(&mut app, table, |action| matches!(action, Action::Toggle(Menu::Columns)));
        press(&mut app, table, |action| {
            matches!(action, Action::Apply(TableEvent::ToggleColumn(id)) if id == "detail")
        });
        assert_eq!(shown_columns(app.world(), table), Some(vec!["node".to_owned()]));
        assert!(!painted_texts(&mut app).iter().any(|text| text == "leader"));
    }

    #[test]
    fn a_header_press_sorts_and_marks_the_column() {
        let (mut app, table) = app();
        press(&mut app, table, |action| matches!(action, Action::Sort(id) if id == "node"));
        assert_eq!(
            shown_rows(app.world(), table),
            Some(vec!["gfr-01".to_owned(), "pdx-01".to_owned(), "pdx-02".to_owned()])
        );
        assert!(painted_texts(&mut app).iter().any(|text| text == "▲"));
    }

    #[test]
    fn density_and_view_buttons_drive_core_state() {
        let (mut app, table) = app();
        press(&mut app, table, |action| matches!(action, Action::Apply(TableEvent::ToggleDensity)));
        assert!(painted_texts(&mut app).iter().any(|text| text == "C"), "compact label painted");
        press(&mut app, table, |action| matches!(action, Action::Apply(TableEvent::View(_))));
        assert!(painted_texts(&mut app).iter().any(|text| text == "D"), "the next view's label (detail)");
    }

    #[test]
    fn csv_exports_core_csv_of_the_shown_rows() {
        let (mut app, table) = app();
        press(&mut app, table, |action| matches!(action, Action::Export(ExportFormat::Csv)));
        let events = app.world().resource::<Events<DataTableExport>>();
        let export = events.iter_current_update_events().next().expect("csv exported").clone();
        assert_eq!(export.format, ExportFormat::Csv);
        assert_eq!(
            export.text,
            "node,detail\r\npdx-01,leader\r\npdx-02,ready\r\ngfr-01,stale\r\n"
        );
    }

    #[test]
    fn cells_take_the_table_type_and_dt_padding() {
        let mut theme = BevyTheme::default();
        theme.metrics.body_size = 17.0;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_event::<KeyboardInput>()
            .add_event::<MouseWheel>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(theme)
            .add_plugins(DataTablePlugin);
        app.world_mut().spawn((DataTable::new(model()), Node::default()));
        app.update();
        let world = app.world_mut();
        let mut texts = world.query::<(&Text, &TextFont, &ChildOf)>();
        let (_, font, parent) = texts
            .iter(world)
            .find(|(text, ..)| text.0 == "leader")
            .expect("a body cell");
        assert_eq!(font.font_size, 17.0);
        let cell = world.get::<Node>(parent.0).expect("cell node");
        assert_eq!(cell.padding.left, Val::Px(dt_pad(Density::Normal).0));
        assert_eq!(cell.padding.top, Val::Px(dt_pad(Density::Normal).1));
    }
}
