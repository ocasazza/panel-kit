//! Bevy UI port of panel-kit core's `dropdown` component.
//!
//! The renderer-neutral state machine lives in
//! [`panel_kit_core::widgets::dropdown`]: this module only paints the trigger,
//! popup, search row, groups, and option buttons with `bevy_ui`, then turns
//! [`Interaction`] clicks into core [`DropdownAction`]s. Add
//! [`DropdownPlugin`] once so clicks update [`DropdownState`] and emit
//! [`DropdownActionEvent`]s for the host's selection callback.
//!
//! Item lists are read at spawn time. To replace the options, despawn the
//! returned root and spawn a new dropdown with the new list.

use bevy::prelude::*;
pub use panel_kit_core::widgets::dropdown::{DropdownAction, DropdownItem};
use panel_kit_core::widgets::dropdown::{
    filter_items, group_items, highlight_next, DropdownState as CoreDropdownState,
};

use crate::theme::BevyTheme;

/// Host-owned state for one dropdown.
///
/// Insert it as a component on the dropdown root, as [`spawn`] does. The popup
/// fields come from core's state machine; `selected` is the host's current
/// value and is reported through [`DropdownAction::Select`].
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct DropdownState {
    /// Open flag, filter query, and keyboard highlight from core.
    pub popup: CoreDropdownState,
    /// Selected item value, or `None` when nothing is selected.
    pub selected: Option<String>,
}

impl DropdownState {
    /// Create a closed dropdown with no selection.
    pub fn with_selection(value: impl Into<String>) -> Self {
        Self {
            selected: Some(value.into()),
            ..default()
        }
    }

    /// Create an open popup with no query or highlight.
    pub fn opened() -> Self {
        Self {
            popup: CoreDropdownState::open(),
            ..default()
        }
    }

    /// Whether the popup is currently open.
    pub fn is_open(&self) -> bool {
        self.popup.open
    }

    /// Open the popup and clear the previous query and highlight.
    pub fn open(&mut self) {
        self.popup = CoreDropdownState::open();
    }

    /// Close the popup and clear the query and highlight.
    pub fn close(&mut self) {
        self.popup = CoreDropdownState::default();
    }

    /// Toggle the popup; opening and closing both reset the popup state.
    pub fn toggle(&mut self) {
        if self.is_open() {
            self.close();
        } else {
            self.open();
        }
    }

    /// Replace the filter query and clear the keyboard highlight.
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.popup.query = query.into();
        self.popup.highlighted = None;
    }

    /// Advance the keyboard highlight through `visible_len` filtered items.
    ///
    /// Wraps in both directions and clears the highlight for an empty list.
    pub fn move_highlight(&mut self, visible_len: usize, forward: bool) {
        self.popup.highlighted = highlight_next(self.popup.highlighted, visible_len, forward);
    }

    /// The selected value, treating an empty string as no selection.
    pub fn selected_value(&self) -> Option<&str> {
        self.selected.as_deref().filter(|value| !value.is_empty())
    }

    /// Select `value` and close the popup.
    pub fn select(&mut self, value: impl Into<String>) {
        self.selected = Some(value.into());
        self.close();
    }

    /// Filter and return the visible items, preserving their list order.
    pub fn visible_items(&self, items: &[DropdownItem]) -> Vec<DropdownItem> {
        filter_items(items, &self.popup.query)
    }

    /// Group the visible items in first-seen group order.
    pub fn visible_groups(&self, items: &[DropdownItem]) -> Vec<(String, Vec<DropdownItem>)> {
        group_items(&self.visible_items(items))
    }
}

/// Marks the root entity of a spawned dropdown.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct DropdownRoot;

/// User intent emitted by a dropdown after a click.
///
/// This wraps core's [`DropdownAction`] so an app can react to selection
/// without inspecting UI entities.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct DropdownActionEvent {
    /// Root entity of the dropdown that produced the action.
    pub dropdown: Entity,
    /// The core action describing the user's intent.
    pub action: DropdownAction,
}

impl DropdownActionEvent {
    /// The selected value when this event is a [`DropdownAction::Select`].
    pub fn selected_value(&self) -> Option<&str> {
        match &self.action {
            DropdownAction::Select { value } => Some(value.as_str()),
            DropdownAction::OpenChanged { .. } => None,
        }
    }
}

/// Adds dropdown interaction handling and action events to a Bevy app.
#[derive(Default)]
pub struct DropdownPlugin;

impl Plugin for DropdownPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DropdownActionEvent>()
            .add_systems(Update, (handle_interactions, sync_ui).chain());
    }
}

/// Spawn a dropdown under `parent` and return its root entity.
///
/// `state` is copied onto the root. Register [`DropdownPlugin`] and read the
/// live state back with `world.get::<DropdownState>(root)`.
pub fn spawn(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    items: &[DropdownItem],
    state: &DropdownState,
    placeholder: &str,
    searchable: bool,
) -> Entity {
    let view = DropdownView {
        items: items.to_vec(),
        placeholder: placeholder.to_string(),
        searchable,
    };
    let mut root = parent.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        },
        DropdownRoot,
        view.clone(),
        state.clone(),
    ));
    let entity = root.id();
    root.with_children(|root| {
        spawn_trigger(root, theme, entity, &view, state);
        spawn_popup(root, theme, entity, &view, state);
    });
    entity
}

/// Presentation data that lives with the rendered entities.
#[derive(Component, Clone, Debug)]
struct DropdownView {
    items: Vec<DropdownItem>,
    placeholder: String,
    searchable: bool,
}

/// Marks every entity the dropdown sync system knows how to paint.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
enum DropdownPart {
    Trigger {
        dropdown: Entity,
    },
    TriggerLabel {
        dropdown: Entity,
    },
    Popup {
        dropdown: Entity,
    },
    Search {
        dropdown: Entity,
    },
    List {
        dropdown: Entity,
    },
    Group {
        dropdown: Entity,
        group: String,
    },
    Option {
        dropdown: Entity,
        value: String,
    },
    Empty {
        dropdown: Entity,
    },
}

impl DropdownPart {
    fn dropdown(&self) -> Entity {
        match self {
            Self::Trigger { dropdown }
            | Self::TriggerLabel { dropdown }
            | Self::Popup { dropdown }
            | Self::Search { dropdown }
            | Self::List { dropdown }
            | Self::Group { dropdown, .. }
            | Self::Option { dropdown, .. }
            | Self::Empty { dropdown } => *dropdown,
        }
    }
}

fn spawn_trigger(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    dropdown: Entity,
    view: &DropdownView,
    state: &DropdownState,
) {
    parent
        .spawn((
            Button,
            Interaction::default(),
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(theme.surface),
            DropdownPart::Trigger { dropdown },
        ))
        .with_children(|trigger| {
            trigger.spawn((
                Text::new(trigger_label(view, state)),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(theme.text),
                DropdownPart::TriggerLabel { dropdown },
            ));
            trigger.spawn((
                Text::new("▾"),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(theme.text_muted),
            ));
        });
}

fn spawn_popup(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    dropdown: Entity,
    view: &DropdownView,
    state: &DropdownState,
) {
    let visible = state.visible_items(&view.items);
    // Every group is spawned once; filtering only toggles visibility. That
    // keeps a search query able to reveal entries it filtered out earlier.
    let groups = group_items(&view.items);
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(4.0)),
                row_gap: Val::Px(2.0),
                display: display_if(state.is_open()),
                ..default()
            },
            BackgroundColor(theme.surface),
            DropdownPart::Popup { dropdown },
        ))
        .with_children(|popup| {
            popup.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                    display: display_if(view.searchable),
                    ..default()
                },
                BackgroundColor(theme.background),
                Text::new(search_label(state)),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(theme.text_muted),
                DropdownPart::Search { dropdown },
            ));
            popup
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(1.0),
                        ..default()
                    },
                    DropdownPart::List { dropdown },
                ))
                .with_children(|list| {
                    list.spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                            display: display_if(state.is_open() && visible.is_empty()),
                            ..default()
                        },
                        Text::new("no matches"),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(theme.text_muted),
                        DropdownPart::Empty { dropdown },
                    ));
                    for (group, entries) in groups {
                        spawn_group(list, theme, dropdown, &group, &entries, state, &visible);
                    }
                });
        });
}

fn spawn_group(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    dropdown: Entity,
    group: &str,
    entries: &[DropdownItem],
    state: &DropdownState,
    visible: &[DropdownItem],
) {
    let group_visible = visible.iter().any(|item| item.group == group);
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                display: display_if(group_visible),
                ..default()
            },
            DropdownPart::Group {
                dropdown,
                group: group.to_string(),
            },
        ))
        .with_children(|group_parent| {
            if !group.is_empty() {
                group_parent.spawn((
                    Text::new(group),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(theme.text_muted),
                ));
            }
            for item in entries {
                spawn_option(group_parent, theme, dropdown, item, state, visible);
            }
        });
}

fn spawn_option(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    dropdown: Entity,
    item: &DropdownItem,
    state: &DropdownState,
    visible: &[DropdownItem],
) {
    let index = visible
        .iter()
        .position(|visible| visible.value == item.value);
    let highlighted = index.is_some() && index == state.popup.highlighted;
    let selected = state.selected_value() == Some(item.value.as_str());
    parent.spawn((
        Button,
        Interaction::default(),
        Node {
            width: Val::Percent(100.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            display: display_if(index.is_some()),
            ..default()
        },
        BackgroundColor(option_background(theme, highlighted, selected)),
        Text::new(item.label.clone()),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(theme.text),
        DropdownPart::Option {
            dropdown,
            value: item.value.clone(),
        },
    ));
}

fn handle_interactions(
    interactions: Query<(&Interaction, &DropdownPart), (Changed<Interaction>, With<Button>)>,
    mut states: Query<&mut DropdownState>,
    mut actions: EventWriter<DropdownActionEvent>,
) {
    for (interaction, part) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match part {
            DropdownPart::Trigger { dropdown } => {
                let Ok(mut state) = states.get_mut(*dropdown) else {
                    continue;
                };
                state.toggle();
                actions.write(DropdownActionEvent {
                    dropdown: *dropdown,
                    action: DropdownAction::OpenChanged {
                        open: state.is_open(),
                    },
                });
            }
            DropdownPart::Option { dropdown, value } => {
                let Ok(mut state) = states.get_mut(*dropdown) else {
                    continue;
                };
                state.select(value.clone());
                actions.write(DropdownActionEvent {
                    dropdown: *dropdown,
                    action: DropdownAction::Select {
                        value: value.clone(),
                    },
                });
                actions.write(DropdownActionEvent {
                    dropdown: *dropdown,
                    action: DropdownAction::OpenChanged { open: false },
                });
            }
            _ => {}
        }
    }
}

fn sync_ui(
    theme: Res<BevyTheme>,
    roots: Query<(Entity, &DropdownView, &DropdownState), Changed<DropdownState>>,
    mut parts: Query<(
        &DropdownPart,
        &mut Node,
        Option<&mut Text>,
        Option<&mut BackgroundColor>,
        Option<&mut TextColor>,
    )>,
) {
    for (root, view, state) in &roots {
        let visible = state.visible_items(&view.items);
        for (part, mut node, text, background, tint) in &mut parts {
            if part.dropdown() != root {
                continue;
            }
            match part {
                DropdownPart::Trigger { .. } | DropdownPart::List { .. } => {}
                DropdownPart::TriggerLabel { .. } => {
                    if let Some(mut text) = text {
                        text.0 = trigger_label(view, state);
                    }
                }
                DropdownPart::Popup { .. } => node.display = display_if(state.is_open()),
                DropdownPart::Search { .. } => {
                    node.display = display_if(view.searchable);
                    if let Some(mut text) = text {
                        text.0 = search_label(state);
                    }
                }
                DropdownPart::Group { group, .. } => {
                    let group_visible = visible
                        .iter()
                        .any(|item| item.group.as_str() == group.as_str());
                    node.display = display_if(group_visible);
                }
                DropdownPart::Option { value, .. } => {
                    let index = visible
                        .iter()
                        .position(|item| item.value.as_str() == value.as_str());
                    node.display = display_if(index.is_some());
                    let highlighted = index.is_some() && index == state.popup.highlighted;
                    let selected = state.selected_value() == Some(value.as_str());
                    if let Some(mut background) = background {
                        background.0 = option_background(&theme, highlighted, selected);
                    }
                    if let Some(mut tint) = tint {
                        tint.0 = option_ink(&theme, highlighted);
                    }
                }
                DropdownPart::Empty { .. } => {
                    node.display = display_if(state.is_open() && visible.is_empty())
                }
            }
        }
    }
}

fn trigger_label(view: &DropdownView, state: &DropdownState) -> String {
    let Some(selected) = state.selected_value() else {
        return view.placeholder.clone();
    };
    view.items
        .iter()
        .find(|item| item.value == selected)
        .map(|item| item.label.clone())
        .unwrap_or_else(|| selected.to_string())
}

fn search_label(state: &DropdownState) -> String {
    if state.popup.query.is_empty() {
        "filter…".to_string()
    } else {
        state.popup.query.clone()
    }
}

fn display_if(visible: bool) -> Display {
    if visible {
        Display::Flex
    } else {
        Display::None
    }
}

/// Option row backgrounds: the cursor row uses the theme highlight (the TUI
/// cursor row's slot), the committed selection a primary tint, plain rows none.
fn option_background(theme: &BevyTheme, highlighted: bool, selected: bool) -> Color {
    if highlighted {
        theme.highlight
    } else if selected {
        theme.primary.with_alpha(0.18)
    } else {
        Color::NONE
    }
}

/// Option row ink: on a highlighted row the highlight background needs the
/// inverse ink, matching the TUI cursor row's `bg(highlight).fg(background)`.
fn option_ink(theme: &BevyTheme, highlighted: bool) -> Color {
    if highlighted {
        theme.background
    } else {
        theme.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::event::Events;
    use bevy::prelude::{App, Children, Display, Entity, Interaction, Node, Text};

    fn item(value: &str, label: &str, group: &str) -> DropdownItem {
        DropdownItem {
            value: value.into(),
            label: label.into(),
            group: group.into(),
        }
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(DropdownPlugin);
        app.insert_resource(BevyTheme::default());
        app
    }

    fn spawn_dropdown(
        app: &mut App,
        items: &[DropdownItem],
        state: &DropdownState,
        placeholder: &str,
        searchable: bool,
    ) -> Entity {
        let world = app.world_mut();
        let parent = world.spawn(Node::default()).id();
        let root = {
            let mut commands = world.commands();
            let mut root = Entity::PLACEHOLDER;
            commands.entity(parent).with_children(|children| {
                let theme = BevyTheme::default();
                root = spawn(children, &theme, items, state, placeholder, searchable);
            });
            root
        };
        world.flush();
        root
    }

    fn parts(app: &mut App) -> Vec<(Entity, DropdownPart)> {
        let mut query = app.world_mut().query::<(Entity, &DropdownPart)>();
        let parts = query
            .iter(app.world())
            .map(|(entity, part)| (entity, part.clone()))
            .collect();
        parts
    }

    fn find_part(
        app: &mut App,
        root: Entity,
        predicate: impl Fn(&DropdownPart) -> bool,
    ) -> Entity {
        parts(app)
            .into_iter()
            .find_map(|(entity, part)| {
                (part.dropdown() == root && predicate(&part)).then_some(entity)
            })
            .expect("dropdown part was spawned")
    }

    fn text_of(app: &App, entity: Entity) -> String {
        app.world()
            .get::<Text>(entity)
            .expect("text component")
            .0
            .clone()
    }

    fn display_of(app: &App, entity: Entity) -> Display {
        app.world().get::<Node>(entity).expect("node").display
    }

    fn background_of(app: &App, entity: Entity) -> Color {
        app.world()
            .get::<BackgroundColor>(entity)
            .expect("background colour")
            .0
    }

    fn press(app: &mut App, entity: Entity) {
        *app.world_mut()
            .get_mut::<Interaction>(entity)
            .expect("interactive dropdown part") = Interaction::Pressed;
        app.update();
    }

    fn release(app: &mut App, entity: Entity) {
        *app.world_mut()
            .get_mut::<Interaction>(entity)
            .expect("interactive dropdown part") = Interaction::None;
        app.update();
    }

    fn actions(app: &mut App) -> Vec<DropdownActionEvent> {
        app.world_mut()
            .resource_mut::<Events<DropdownActionEvent>>()
            .drain()
            .collect()
    }

    fn rendered_groups(app: &mut App, root: Entity) -> Vec<(String, Vec<String>)> {
        let list = find_part(app, root, |part| matches!(part, DropdownPart::List { .. }));
        let groups = app.world().get::<Children>(list).expect("list children").to_vec();
        groups
            .iter()
            .filter_map(|group| {
                let DropdownPart::Group { group: name, .. } =
                    app.world().get::<DropdownPart>(*group)?
                else {
                    return None;
                };
                if display_of(app, *group) == Display::None {
                    return None;
                }
                let entries = app
                    .world()
                    .get::<Children>(*group)
                    .map(|children| {
                        children
                            .iter()
                            .filter_map(|entry| {
                                if !matches!(
                                    app.world().get::<DropdownPart>(entry),
                                    Some(DropdownPart::Option { .. })
                                ) {
                                    return None;
                                }
                                (display_of(app, entry) != Display::None)
                                    .then(|| text_of(app, entry))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Some((name.clone(), entries))
            })
            .collect()
    }

    fn visible_option_labels(app: &mut App, root: Entity) -> Vec<String> {
        parts(app)
            .into_iter()
            .filter_map(|(entity, part)| {
                let is_option = matches!(part, DropdownPart::Option { .. });
                (part.dropdown() == root && is_option && display_of(app, entity) != Display::None)
                    .then(|| text_of(app, entity))
            })
            .collect()
    }

    #[test]
    fn trigger_shows_selected_label_and_placeholder() {
        let items = vec![
            item("v74-000", "real trot", "c1"),
            item("v74-001", "velocity tracker", "c1"),
        ];
        let mut app = test_app();
        let selected = spawn_dropdown(
            &mut app,
            &items,
            &DropdownState::with_selection("v74-001"),
            "pick a policy",
            true,
        );
        let empty = spawn_dropdown(&mut app, &items, &DropdownState::default(), "pick a policy", true);
        let missing = spawn_dropdown(
            &mut app,
            &items,
            &DropdownState::with_selection("v74-999"),
            "pick a policy",
            true,
        );
        app.update();

        let selected_label = find_part(&mut app, selected, |part| {
            matches!(part, DropdownPart::TriggerLabel { .. })
        });
        let empty_label = find_part(&mut app, empty, |part| {
            matches!(part, DropdownPart::TriggerLabel { .. })
        });
        let missing_label = find_part(&mut app, missing, |part| {
            matches!(part, DropdownPart::TriggerLabel { .. })
        });
        assert_eq!(text_of(&app, selected_label), "velocity tracker");
        assert_eq!(text_of(&app, empty_label), "pick a policy");
        assert_eq!(text_of(&app, missing_label), "v74-999");
    }

    #[test]
    fn trigger_click_toggles_open_and_reports_actions() {
        let items = vec![item("a", "alpha", "g1")];
        let mut app = test_app();
        let root = spawn_dropdown(&mut app, &items, &DropdownState::default(), "select…", true);
        app.update();
        let trigger = find_part(&mut app, root, |part| matches!(part, DropdownPart::Trigger { .. }));
        let popup = find_part(&mut app, root, |part| matches!(part, DropdownPart::Popup { .. }));

        assert_eq!(display_of(&app, popup), Display::None);
        press(&mut app, trigger);
        assert!(app.world().get::<DropdownState>(root).unwrap().is_open());
        assert_eq!(display_of(&app, popup), Display::Flex);
        assert_eq!(
            actions(&mut app),
            vec![DropdownActionEvent {
                dropdown: root,
                action: DropdownAction::OpenChanged { open: true },
            }]
        );

        release(&mut app, trigger);
        press(&mut app, trigger);
        assert!(!app.world().get::<DropdownState>(root).unwrap().is_open());
        assert_eq!(display_of(&app, popup), Display::None);
        assert_eq!(
            actions(&mut app),
            vec![DropdownActionEvent {
                dropdown: root,
                action: DropdownAction::OpenChanged { open: false },
            }]
        );
    }

    #[test]
    fn option_click_selects_closes_and_reports_selection() {
        let items = vec![
            item("v74-000", "real trot", "c1"),
            item("v74-001", "velocity tracker", "c1"),
        ];
        let mut app = test_app();
        let root = spawn_dropdown(&mut app, &items, &DropdownState::opened(), "select…", true);
        app.update();
        let option = find_part(&mut app, root, |part| {
            matches!(part, DropdownPart::Option { value, .. } if value.as_str() == "v74-001")
        });
        let trigger_label = find_part(&mut app, root, |part| {
            matches!(part, DropdownPart::TriggerLabel { .. })
        });

        press(&mut app, option);

        let state = app.world().get::<DropdownState>(root).unwrap();
        assert_eq!(state.selected.as_deref(), Some("v74-001"));
        assert!(!state.is_open());
        assert_eq!(text_of(&app, trigger_label), "velocity tracker");
        assert_eq!(background_of(&app, option), option_background(&BevyTheme::default(), false, true));
        assert_eq!(
            actions(&mut app),
            vec![
                DropdownActionEvent {
                    dropdown: root,
                    action: DropdownAction::Select {
                        value: "v74-001".into(),
                    },
                },
                DropdownActionEvent {
                    dropdown: root,
                    action: DropdownAction::OpenChanged { open: false },
                },
            ]
        );
    }

    #[test]
    fn query_filters_case_insensitively_over_label_and_value() {
        let items = vec![
            item("v74-000", "real trot", "c1"),
            item("v74-001", "velocity tracker", "c1"),
            item("v74-002", "idle", "c2"),
        ];
        let mut state = DropdownState::opened();
        state.set_query("TROT");
        let mut app = test_app();
        let root = spawn_dropdown(&mut app, &items, &state, "select…", true);
        app.update();

        assert_eq!(visible_option_labels(&mut app, root), vec!["real trot"]);
        assert_eq!(
            rendered_groups(&mut app, root),
            vec![("c1".to_string(), vec!["real trot".to_string()])]
        );
        let c2 = find_part(&mut app, root, |part| {
            matches!(part, DropdownPart::Group { group, .. } if group.as_str() == "c2")
        });
        assert_eq!(display_of(&app, c2), Display::None);

        app.world_mut()
            .get_mut::<DropdownState>(root)
            .unwrap()
            .set_query("V74-001");
        app.update();
        assert_eq!(
            visible_option_labels(&mut app, root),
            vec!["velocity tracker"]
        );
    }

    #[test]
    fn filter_with_no_match_renders_no_matches_and_hides_groups() {
        let items = vec![item("a", "alpha", "g1")];
        let mut state = DropdownState::opened();
        state.set_query("zzz");
        let mut app = test_app();
        let root = spawn_dropdown(&mut app, &items, &state, "select…", true);
        app.update();

        let empty = find_part(&mut app, root, |part| matches!(part, DropdownPart::Empty { .. }));
        let group = find_part(&mut app, root, |part| matches!(part, DropdownPart::Group { .. }));
        assert_eq!(display_of(&app, empty), Display::Flex);
        assert_eq!(text_of(&app, empty), "no matches");
        assert_eq!(display_of(&app, group), Display::None);
        assert!(visible_option_labels(&mut app, root).is_empty());
    }

    #[test]
    fn empty_item_list_renders_no_matches_when_open() {
        let mut app = test_app();
        let root = spawn_dropdown(&mut app, &[], &DropdownState::opened(), "select…", true);
        app.update();

        let empty = find_part(&mut app, root, |part| matches!(part, DropdownPart::Empty { .. }));
        assert_eq!(display_of(&app, empty), Display::Flex);
        assert_eq!(text_of(&app, empty), "no matches");
    }

    #[test]
    fn groups_render_in_first_seen_order() {
        let items = vec![
            item("a", "alpha", "g2"),
            item("b", "beta", "g1"),
            item("c", "gamma", "g2"),
        ];
        let mut app = test_app();
        let root = spawn_dropdown(&mut app, &items, &DropdownState::opened(), "select…", true);
        app.update();

        assert_eq!(
            rendered_groups(&mut app, root),
            vec![
                ("g2".to_string(), vec!["alpha".to_string(), "gamma".to_string()]),
                ("g1".to_string(), vec!["beta".to_string()]),
            ]
        );

        let g2 = find_part(&mut app, root, |part| {
            matches!(part, DropdownPart::Group { group, .. } if group.as_str() == "g2")
        });
        let first_child = app.world().get::<Children>(g2).expect("group children")[0];
        assert_eq!(text_of(&app, first_child), "g2");
    }

    #[test]
    fn ungrouped_items_render_without_a_group_label() {
        let items = vec![item("a", "alpha", ""), item("b", "beta", "")];
        let mut app = test_app();
        let root = spawn_dropdown(&mut app, &items, &DropdownState::opened(), "select…", true);
        app.update();

        let group = find_part(&mut app, root, |part| matches!(part, DropdownPart::Group { .. }));
        let first_child = app.world().get::<Children>(group).expect("group children")[0];
        assert!(matches!(
            app.world().get::<DropdownPart>(first_child),
            Some(DropdownPart::Option { .. })
        ));
    }

    #[test]
    fn highlight_moves_wraps_and_clears_on_empty() {
        let items = vec![item("a", "alpha", "g1"), item("b", "beta", "g1")];
        let mut state = DropdownState::opened();
        state.move_highlight(2, true);
        assert_eq!(state.popup.highlighted, Some(0));
        state.move_highlight(2, true);
        assert_eq!(state.popup.highlighted, Some(1));
        state.move_highlight(2, true);
        assert_eq!(state.popup.highlighted, Some(0));
        state.move_highlight(2, false);
        assert_eq!(state.popup.highlighted, Some(1));
        state.move_highlight(0, true);
        assert_eq!(state.popup.highlighted, None);

        let mut state = DropdownState::opened();
        state.move_highlight(2, true);
        let mut app = test_app();
        let root = spawn_dropdown(&mut app, &items, &state, "select…", true);
        app.update();

        let first = find_part(&mut app, root, |part| {
            matches!(part, DropdownPart::Option { value, .. } if value.as_str() == "a")
        });
        let second = find_part(&mut app, root, |part| {
            matches!(part, DropdownPart::Option { value, .. } if value.as_str() == "b")
        });
        assert_eq!(background_of(&app, first), option_background(&BevyTheme::default(), true, false));
        assert_eq!(background_of(&app, second), Color::NONE);
    }

    #[test]
    fn search_row_tracks_query_and_searchable_flag() {
        let items = vec![item("a", "alpha", "g1")];
        let mut state = DropdownState::opened();
        state.set_query("ALP");
        let mut app = test_app();
        let searchable = spawn_dropdown(&mut app, &items, &state, "select…", true);
        let plain = spawn_dropdown(&mut app, &items, &state, "select…", false);
        app.update();

        let search = find_part(&mut app, searchable, |part| {
            matches!(part, DropdownPart::Search { .. })
        });
        let hidden = find_part(&mut app, plain, |part| {
            matches!(part, DropdownPart::Search { .. })
        });
        assert_eq!(display_of(&app, search), Display::Flex);
        assert_eq!(text_of(&app, search), "ALP");
        assert_eq!(display_of(&app, hidden), Display::None);
    }
}
