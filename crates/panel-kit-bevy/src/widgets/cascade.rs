//! Bevy UI port of panel-kit core's Miller-columns `cascade`.
//!
//! [`CascadeModel`] owns the item list, the core [`CascadeState`], and the
//! committed selection. [`cascade`] paints one column per descended segment;
//! rows carry [`Interaction`] plus a [`CascadeChoice`], so clicking a category
//! opens its column and clicking a leaf commits the selection.
//!
//! The host adds [`CascadePlugin`] (or the two systems directly) and puts
//! [`CascadeRoot`] on the container whose children [`rebuild_cascade_ui`]
//! repaints whenever the model changes.

use bevy::prelude::*;

use super::label_text;
use crate::theme::BevyTheme;

pub use panel_kit_core::widgets::cascade::{
    column, columns, CascadeAction, CascadeEntry, CascadeItem, CascadeState,
};

/// Marks the container whose children the cascade repaints on model changes.
#[derive(Component)]
pub struct CascadeRoot;

/// Marks a rendered Miller column and exposes its depth (root is `0`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CascadeColumn {
    /// Zero-based column depth; equals the descended path length when active.
    pub index: usize,
}

/// Clickable row payload: which column owns it and what choosing it means.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct CascadeChoice {
    /// Zero-based index of the column that rendered this row.
    pub column: usize,
    /// The core entry the row was painted from.
    pub entry: CascadeEntry,
}

/// Visual role of a rendered row, in precedence order.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeRowRole {
    /// Keyboard-highlighted row in the deepest open column.
    Highlighted,
    /// Row on the descended navigation path.
    Expanded,
    /// Row on the committed selection's ancestor path.
    Selected,
    /// Plain option.
    Idle,
}

/// Marks the popup trigger, which toggles [`CascadeState::open`] when clicked.
#[derive(Component)]
pub struct CascadeToggle;

/// Host-owned items plus cascade navigation and selection state.
///
/// Mirrors the web painter's host contract: the popup state and the committed
/// selection stay here so rerenders cannot reset either one.
#[derive(Resource, Clone, Debug, Default)]
pub struct CascadeModel {
    /// Leaf items carrying their ancestor category paths.
    pub items: Vec<CascadeItem>,
    /// Popup and navigation state.
    pub state: CascadeState,
    /// Ancestor path of the committed selection.
    pub selected_path: Vec<String>,
    /// Stable machine value of the committed selection.
    pub selected_value: Option<String>,
    /// Actions emitted by interactions since the host last drained them.
    pub actions: Vec<CascadeAction>,
}

impl CascadeModel {
    /// Build a model over `items` with the popup open at its root column.
    pub fn new(items: Vec<CascadeItem>) -> Self {
        Self {
            items,
            state: CascadeState::open(),
            ..default()
        }
    }

    /// Full `path / label` breadcrumb of the committed selection.
    ///
    /// The path comes from the selected item so the breadcrumb stays truthful
    /// even if the host mutated [`Self::selected_path`] independently.
    pub fn breadcrumb(&self) -> Option<String> {
        let value = self.selected_value.as_deref()?;
        let item = self.items.iter().find(|item| item.value == value)?;
        let mut crumbs = item.path.clone();
        crumbs.push(item.label.clone());
        Some(crumbs.join(" / "))
    }

    /// Apply a row click.
    ///
    /// A category click truncates the navigation path to its own column and
    /// descends, which keeps clicks in an earlier open column from appending a
    /// stale branch. A leaf click commits the selection, queues `Select` then
    /// `OpenChanged { open: false }`, and closes the popup.
    pub fn choose(&mut self, choice: &CascadeChoice) {
        self.state.path.truncate(choice.column);

        if choice.entry.has_children {
            self.state.descend(choice.entry.key.clone());
            return;
        }

        let Some(value) = choice.entry.value.clone() else {
            return;
        };

        self.selected_path = self.state.path.clone();
        self.selected_value = Some(value.clone());
        self.state = CascadeState::default();
        self.actions.push(CascadeAction::Select {
            path: self.selected_path.clone(),
            value,
        });
        self.actions.push(CascadeAction::OpenChanged { open: false });
    }

    /// Toggle the popup, resetting navigation when opening or closing.
    pub fn toggle(&mut self) {
        self.state = if self.state.open {
            CascadeState::default()
        } else {
            CascadeState::open()
        };
        self.actions.push(CascadeAction::OpenChanged {
            open: self.state.open,
        });
    }

    /// Remove and return every queued action for the host to consume.
    pub fn drain_actions(&mut self) -> Vec<CascadeAction> {
        std::mem::take(&mut self.actions)
    }
}

/// Paint the trigger and every open Miller column for `model`.
///
/// The root column always exists while the popup is open, even when `items` is
/// empty; an empty column renders the muted `empty` marker so the popup keeps
/// its shape instead of collapsing.
pub fn cascade(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, model: &CascadeModel) {
    spawn_trigger(parent, theme, model);
    if !model.state.open {
        return;
    }

    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|board| {
            for (index, entries) in columns(&model.items, &model.state.path).iter().enumerate() {
                spawn_column(board, theme, model, index, entries);
            }
        });
}

/// Repaint every [`CascadeRoot`] whenever [`CascadeModel`] changes.
pub fn rebuild_cascade_ui(
    mut commands: Commands,
    theme: Res<BevyTheme>,
    model: Res<CascadeModel>,
    roots: Query<Entity, With<CascadeRoot>>,
) {
    if !model.is_changed() && !theme.is_changed() {
        return;
    }

    for root in &roots {
        commands
            .entity(root)
            .despawn_related::<Children>()
            .with_children(|parent| cascade(parent, &theme, &model));
    }
}

/// Translate pressed row and trigger interactions into model transitions.
pub fn handle_cascade_interaction(
    mut model: ResMut<CascadeModel>,
    choices: Query<(&Interaction, &CascadeChoice), Changed<Interaction>>,
    toggles: Query<&Interaction, (Changed<Interaction>, With<CascadeToggle>)>,
) {
    for (interaction, choice) in &choices {
        if *interaction == Interaction::Pressed {
            model.choose(choice);
        }
    }

    for interaction in &toggles {
        if *interaction == Interaction::Pressed {
            model.toggle();
        }
    }
}

/// Adds the cascade repaint and interaction systems to a Bevy app.
pub struct CascadePlugin;

impl Plugin for CascadePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CascadeModel>()
            .add_systems(Update, (handle_cascade_interaction, rebuild_cascade_ui).chain());
    }
}

fn spawn_trigger(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, model: &CascadeModel) {
    let label = model.breadcrumb().unwrap_or_else(|| "select…".to_string());
    let caret = if model.state.open { "▴" } else { "▾" };

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                ..default()
            },
            Interaction::default(),
            CascadeToggle,
            BackgroundColor(theme.surface),
        ))
        .with_children(|trigger| {
            label_text(trigger, label, 11.0, theme.text);
            label_text(trigger, caret, 10.0, theme.text_muted);
        });
}

fn spawn_column(
    board: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    model: &CascadeModel,
    index: usize,
    entries: &[CascadeEntry],
) {
    board
        .spawn((
            Node {
                min_width: Val::Px(150.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            CascadeColumn { index },
            BackgroundColor(theme.text.with_alpha(0.03)),
        ))
        .with_children(|column| {
            if entries.is_empty() {
                label_text(column, "empty", 10.0, theme.text_muted);
                return;
            }

            for (entry_index, entry) in entries.iter().enumerate() {
                spawn_row(column, theme, model, index, entry_index, entry);
            }
        });
}

fn spawn_row(
    column: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    model: &CascadeModel,
    column_index: usize,
    entry_index: usize,
    entry: &CascadeEntry,
) {
    let role = row_role(model, column_index, entry_index, &entry.key);

    column
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                ..default()
            },
            Interaction::default(),
            CascadeChoice {
                column: column_index,
                entry: entry.clone(),
            },
            role,
            BackgroundColor(row_tint(theme, role)),
        ))
        .with_children(|row| {
            label_text(row, entry.label.clone(), 11.0, theme.text);
            if entry.has_children {
                label_text(row, "▸", 10.0, theme.text_muted);
            }
        });
}

/// Classify a row; highlight wins over navigation, then committed selection.
fn row_role(model: &CascadeModel, column: usize, index: usize, key: &str) -> CascadeRowRole {
    let is_deepest = column == model.state.path.len();

    if is_deepest && model.state.highlighted == Some(index) {
        return CascadeRowRole::Highlighted;
    }
    if model.state.path.get(column).is_some_and(|segment| segment == key) {
        return CascadeRowRole::Expanded;
    }
    if model.selected_path.get(column).is_some_and(|segment| segment == key) {
        return CascadeRowRole::Selected;
    }

    CascadeRowRole::Idle
}

/// Row tints: the keyboard-highlighted and descended-path rows use the
/// primary selection tone, and the committed selection's ancestors the success
/// tone, each at the painter's original strengths.
fn row_tint(theme: &BevyTheme, role: CascadeRowRole) -> Color {
    match role {
        CascadeRowRole::Highlighted => theme.primary.with_alpha(0.18),
        CascadeRowRole::Expanded => theme.primary.with_alpha(0.10),
        CascadeRowRole::Selected => theme.success.with_alpha(0.10),
        CascadeRowRole::Idle => Color::NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;

    fn item(path: &[&str], value: &str, label: &str) -> CascadeItem {
        CascadeItem {
            path: path.iter().map(|segment| segment.to_string()).collect(),
            value: value.into(),
            label: label.into(),
        }
    }

    fn fixture() -> Vec<CascadeItem> {
        vec![
            item(&["spot", "v74"], "spot-walk-pbt-v74-000", "000 — real trot"),
            item(&["spot", "v74"], "spot-walk-pbt-v74-001", "001 — no gait"),
            item(&["spot", "v72"], "spot-walk-pbt-v72-011", "011 — baseline"),
            item(&[], "spider", "spider (sandbox)"),
        ]
    }

    fn app_with(model: CascadeModel) -> App {
        let mut app = App::new();
        app.insert_resource(model);
        app.insert_resource(BevyTheme::default());
        app.add_plugins(CascadePlugin);
        app.world_mut().spawn((Node::default(), CascadeRoot));
        app.update();
        app
    }

    fn model_of(app: &App) -> &CascadeModel {
        app.world().resource::<CascadeModel>()
    }

    fn columns_of(app: &mut App) -> Vec<Entity> {
        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<CascadeColumn>>();
        let found: Vec<Entity> = query.iter(app.world()).collect();

        let mut keyed: Vec<(usize, Entity)> = found
            .into_iter()
            .map(|entity| (app.world().get::<CascadeColumn>(entity).unwrap().index, entity))
            .collect();
        keyed.sort_by_key(|(index, _)| *index);
        keyed.into_iter().map(|(_, entity)| entity).collect()
    }

    fn entries_in(app: &App, column: Entity) -> Vec<CascadeEntry> {
        let world = app.world();
        let Some(children) = world.get::<Children>(column) else {
            return Vec::new();
        };

        children
            .iter()
            .filter_map(|child| {
                world
                    .get::<CascadeChoice>(child)
                    .map(|choice| choice.entry.clone())
            })
            .collect()
    }

    fn texts_under(world: &World, entity: Entity) -> Vec<String> {
        let mut texts = Vec::new();
        if let Some(text) = world.get::<Text>(entity) {
            texts.push(text.0.clone());
        }
        if let Some(children) = world.get::<Children>(entity) {
            for child in children.iter() {
                texts.extend(texts_under(world, child));
            }
        }
        texts
    }

    fn entity_of(app: &mut App, key: &str) -> Entity {
        let mut query = app.world_mut().query::<(Entity, &CascadeChoice)>();
        query
            .iter(app.world())
            .find(|(_, choice)| choice.entry.key == key)
            .map(|(entity, _)| entity)
            .unwrap_or_else(|| panic!("no cascade row for {key:?}"))
    }

    fn trigger_of(app: &mut App) -> Entity {
        let mut query = app.world_mut().query_filtered::<Entity, With<CascadeToggle>>();
        query
            .iter(app.world())
            .next()
            .expect("cascade trigger exists")
    }

    fn role_of(app: &App, entity: Entity) -> CascadeRowRole {
        *app.world()
            .get::<CascadeRowRole>(entity)
            .expect("row carries its visual role")
    }

    fn press(app: &mut App, entity: Entity) {
        *app.world_mut()
            .get_mut::<Interaction>(entity)
            .expect("node is interactive") = Interaction::Pressed;
        app.update();
    }

    fn press_key(app: &mut App, key: &str) {
        let entity = entity_of(app, key);
        press(app, entity);
    }

    #[test]
    fn root_column_lists_categories_first_seen_plus_root_leaves() {
        let mut app = app_with(CascadeModel::new(fixture()));
        let rendered = columns_of(&mut app);
        assert_eq!(rendered.len(), 1);

        let entries = entries_in(&app, rendered[0]);
        let keys: Vec<&str> = entries.iter().map(|entry| entry.key.as_str()).collect();
        assert_eq!(keys, vec!["spot", "spider"]);
        assert!(entries[0].has_children);
        assert!(!entries[1].has_children);
        assert_eq!(entries[1].value.as_deref(), Some("spider"));

        let texts = texts_under(app.world(), rendered[0]);
        assert_eq!(texts, vec!["spot", "▸", "spider (sandbox)"]);
    }

    #[test]
    fn category_wins_when_a_key_is_also_a_leaf() {
        let items = vec![
            item(&[], "spot", "spot leaf"),
            item(&["spot"], "policy", "policy"),
        ];
        let mut app = app_with(CascadeModel::new(items));
        let rendered = columns_of(&mut app);
        assert_eq!(rendered.len(), 1);

        let entries = entries_in(&app, rendered[0]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "spot");
        assert!(entries[0].has_children);
        assert_eq!(entries[0].value, None);
    }

    #[test]
    fn descended_column_lists_leaves() {
        let mut model = CascadeModel::new(fixture());
        model.state.descend("spot".into());
        let mut app = app_with(model);

        let rendered = columns_of(&mut app);
        assert_eq!(rendered.len(), 2);
        let categories = entries_in(&app, rendered[1]);
        let keys: Vec<&str> = categories
            .iter()
            .map(|entry| entry.key.as_str())
            .collect();
        assert_eq!(keys, vec!["v74", "v72"]);
        assert!(categories.iter().all(|entry| entry.has_children));

        app.world_mut()
            .resource_mut::<CascadeModel>()
            .state
            .descend("v74".into());
        app.update();

        let rendered = columns_of(&mut app);
        assert_eq!(rendered.len(), 3);
        let leaves = entries_in(&app, rendered[2]);
        assert_eq!(leaves.len(), 2);
        assert!(leaves.iter().all(|entry| !entry.has_children));
        assert_eq!(leaves[0].value.as_deref(), Some("spot-walk-pbt-v74-000"));
        assert_eq!(texts_under(app.world(), rendered[2]), vec![
            "000 — real trot",
            "001 — no gait"
        ]);
    }

    #[test]
    fn descend_and_ascend_drive_columns() {
        let mut model = CascadeModel::new(fixture());
        model.state.descend("spot".into());
        model.state.descend("v74".into());
        model.state.highlighted = Some(1);
        let mut app = app_with(model);

        let rendered = columns_of(&mut app);
        assert_eq!(rendered.len(), 3);
        assert_eq!(entries_in(&app, rendered[2]).len(), 2);

        app.world_mut().resource_mut::<CascadeModel>().state.ascend();
        app.update();

        assert_eq!(columns_of(&mut app).len(), 2);
        assert_eq!(model_of(&app).state.highlighted, None);
    }

    #[test]
    fn unrelated_items_never_leak_into_a_column() {
        let mut model = CascadeModel::new(fixture());
        model.state.descend("nope".into());
        let mut app = app_with(model);

        let rendered = columns_of(&mut app);
        assert_eq!(rendered.len(), 2);
        assert!(entries_in(&app, rendered[1]).is_empty());
        assert_eq!(texts_under(app.world(), rendered[1]), vec!["empty"]);

        app.world_mut().resource_mut::<CascadeModel>().state.path =
            vec!["spot".into(), "v72".into()];
        app.update();

        let rendered = columns_of(&mut app);
        assert_eq!(rendered.len(), 3);
        let leaves = entries_in(&app, rendered[2]);
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].key, "spot-walk-pbt-v72-011");
    }

    #[test]
    fn empty_tree_renders_a_single_empty_column() {
        let mut app = app_with(CascadeModel::new(Vec::new()));
        let rendered = columns_of(&mut app);
        assert_eq!(rendered.len(), 1);
        assert!(entries_in(&app, rendered[0]).is_empty());
        assert_eq!(texts_under(app.world(), rendered[0]), vec!["empty"]);

        let trigger = trigger_of(&mut app);
        assert_eq!(texts_under(app.world(), trigger), vec!["select…", "▴"]);
    }

    #[test]
    fn category_click_opens_column_and_leaf_click_commits_selection() {
        let mut app = app_with(CascadeModel::new(fixture()));

        press_key(&mut app, "spot");
        assert_eq!(model_of(&app).state.path, vec!["spot".to_string()]);
        assert_eq!(columns_of(&mut app).len(), 2);

        press_key(&mut app, "v74");
        assert_eq!(model_of(&app).state.path, vec!["spot".to_string(), "v74".to_string()]);
        assert_eq!(columns_of(&mut app).len(), 3);

        press_key(&mut app, "spot-walk-pbt-v74-001");
        let model = model_of(&app);
        assert_eq!(model.selected_path, vec!["spot".to_string(), "v74".to_string()]);
        assert_eq!(
            model.selected_value.as_deref(),
            Some("spot-walk-pbt-v74-001")
        );
        assert!(!model.state.open);
        assert_eq!(model.actions, vec![
            CascadeAction::Select {
                path: vec!["spot".to_string(), "v74".to_string()],
                value: "spot-walk-pbt-v74-001".to_string(),
            },
            CascadeAction::OpenChanged { open: false },
        ]);

        assert_eq!(columns_of(&mut app).len(), 0);
        let trigger = trigger_of(&mut app);
        assert_eq!(
            texts_under(app.world(), trigger),
            vec!["spot / v74 / 001 — no gait", "▾"]
        );
    }

    #[test]
    fn clicking_an_earlier_column_truncates_the_path() {
        let items = vec![
            item(&["spot", "v74"], "spot-v74", "spot v74"),
            item(&["spider", "v1"], "spider-v1", "spider v1"),
        ];
        let mut model = CascadeModel::new(items);
        model.state.descend("spider".into());
        model.state.descend("v1".into());
        let mut app = app_with(model);
        assert_eq!(columns_of(&mut app).len(), 3);

        press_key(&mut app, "spot");
        assert_eq!(model_of(&app).state.path, vec!["spot".to_string()]);
        assert_eq!(columns_of(&mut app).len(), 2);
    }

    #[test]
    fn highlighted_index_marks_only_the_deepest_column() {
        let mut model = CascadeModel::new(fixture());
        model.state.descend("spot".into());
        model.state.highlighted = Some(0);
        let mut app = app_with(model);

        let expanded = entity_of(&mut app, "spot");
        let highlighted = entity_of(&mut app, "v74");
        let idle = entity_of(&mut app, "v72");

        assert_eq!(role_of(&app, expanded), CascadeRowRole::Expanded);
        assert_eq!(role_of(&app, highlighted), CascadeRowRole::Highlighted);
        assert_eq!(role_of(&app, idle), CascadeRowRole::Idle);

        let tint = |app: &App, entity: Entity| {
            app.world()
                .get::<BackgroundColor>(entity)
                .expect("row has a background")
                .0
        };
        assert_ne!(tint(&app, highlighted), Color::NONE);
        assert_eq!(tint(&app, idle), Color::NONE);
    }

    #[test]
    fn selected_path_marks_committed_ancestors_and_survives_toggle() {
        let mut model = CascadeModel::new(fixture());
        model.selected_path = vec!["spot".into()];
        model.selected_value = Some("spot-walk-pbt-v74-000".into());
        let mut app = app_with(model);

        let selected = entity_of(&mut app, "spot");
        let plain = entity_of(&mut app, "spider");
        assert_eq!(role_of(&app, selected), CascadeRowRole::Selected);
        assert_eq!(role_of(&app, plain), CascadeRowRole::Idle);

        let trigger = trigger_of(&mut app);
        press(&mut app, trigger);
        assert!(!model_of(&app).state.open);
        assert_eq!(columns_of(&mut app).len(), 0);
        let closed_trigger = trigger_of(&mut app);
        assert_eq!(
            texts_under(app.world(), closed_trigger),
            vec!["spot / v74 / 000 — real trot", "▾"]
        );
        assert_eq!(model_of(&app).selected_path, vec!["spot".to_string()]);

        let actions = app
            .world_mut()
            .resource_mut::<CascadeModel>()
            .drain_actions();
        assert_eq!(actions, vec![CascadeAction::OpenChanged { open: false }]);

        let reopened_trigger = trigger_of(&mut app);
        press(&mut app, reopened_trigger);
        assert!(model_of(&app).state.open);
        assert_eq!(columns_of(&mut app).len(), 1);
        let selected = entity_of(&mut app, "spot");
        assert_eq!(role_of(&app, selected), CascadeRowRole::Selected);
    }
}
