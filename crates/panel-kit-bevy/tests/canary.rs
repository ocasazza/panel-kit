//! The Bevy canary paints the shared `workspace-canary` content: the spec
//! resolves against the shared provider manifest, and every panel's body
//! carries its provider's data rather than a placeholder.

#[allow(dead_code)]
#[path = "../examples/workspace_canary.rs"]
mod workspace_canary;

use bevy::prelude::*;
use panel_kit_bevy::widgets::graph::{GraphNodeDot, GraphSelectionRing};
use panel_kit_bevy::BevyTheme;
use panel_kit_core::badge::display_label;
use panel_kit_core::spec::{BackendKind, ResolvedWorkspace, WorkspaceSpec};
use panel_kit_core::theme::ThemeTokens;
use panel_kit_core::widgets::table::TableCell;
use panel_kit_core::widgets::ContentSpec;

use workspace_canary::{CanaryData, WorkspaceFacts};

const SPEC_JSON: &str = include_str!(env!("PANEL_KIT_WORKSPACE_SPEC"));

fn resolved() -> ResolvedWorkspace {
    WorkspaceSpec::from_json_str(SPEC_JSON)
        .expect("workspace-canary decodes")
        .resolve(&workspace_canary::provider_manifest(BackendKind::Bevy))
        .expect("workspace-canary resolves against the shared Bevy manifest")
}

/// Paint one body into a fresh world; return every text it spawned and the
/// world for further queries.
fn paint(theme: &BevyTheme, binding: &str, content: &ContentSpec) -> (Vec<String>, App) {
    paint_with(theme, binding, content, CanaryData::default())
}

fn paint_with(
    theme: &BevyTheme,
    binding: &str,
    content: &ContentSpec,
    data: CanaryData,
) -> (Vec<String>, App) {
    let mut app = App::new();
    app.add_event::<bevy::input::keyboard::KeyboardInput>()
        .add_event::<bevy::input::mouse::MouseWheel>()
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(theme.clone())
        .add_plugins(panel_kit_bevy::widgets::data_table::DataTablePlugin);
    let root = app.world_mut().spawn(Node::default()).id();
    let (theme, binding, content) = (theme.clone(), binding.to_owned(), content.clone());
    app.add_systems(Startup, move |mut commands: Commands| {
        let data = &data;
        let facts = WorkspaceFacts {
            title: "Workspace",
            maximized: false,
        };
        commands.entity(root).with_children(|parent| {
            workspace_canary::paint(parent, &theme, data, facts, &binding, &content);
        });
    });
    app.update();
    let texts = texts_under(app.world(), root);
    (texts, app)
}

/// Every text run under `entity`, depth first. A tracked run is one text
/// node per glyph; its glyphs join back into one run.
fn texts_under(world: &World, entity: Entity) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(text) = world.get::<Text>(entity) {
        out.push(text.0.clone());
    }
    let children: Vec<Entity> = world
        .get::<Children>(entity)
        .map(|children| children.iter().collect())
        .unwrap_or_default();
    let glyphs: Option<String> = children
        .iter()
        .map(|child| world.get::<Text>(*child).filter(|t| t.0.chars().count() == 1).map(|t| t.0.clone()))
        .collect();
    match glyphs {
        Some(run) if children.len() > 1 => out.push(run),
        _ => {
            for child in children {
                out.extend(texts_under(world, child));
            }
        }
    }
    out
}

/// The text each canary binding must show, read from the shared content so
/// a backend cannot pass with a private copy.
fn expected_marker(binding: &str) -> String {
    let data = CanaryData::default();
    match binding {
        "canary.workspace" => workspace_canary::WORKSPACE_CARD_HEADING.to_owned(),
        "canary.activity" => data.metrics.series()[0].name.to_owned(),
        "canary.flame" => data.metrics.flame()[0].label.clone(),
        "canary.notes" => workspace_canary::NOTES[0].0.to_owned(),
        "canary.badges" => {
            let badge = &data.badges[data.badges.len() - 1];
            display_label(&badge.kind, &badge.value)
        }
        "canary.nodes" => match &workspace_canary::node_rows().rows[0].cells[0] {
            TableCell::Status { label, .. } => label.clone(),
            other => panic!("first node cell is a status, got {other:?}"),
        },
        "canary.capacity" => workspace_canary::capacity_items()[0].label.clone(),
        "canary.distribution" => workspace_canary::STAGES[0].to_owned(),
        "canary.theme" => workspace_canary::theme_preset_label(false).to_owned(),
        other => panic!("unexpected canary binding {other}"),
    }
}

#[test]
fn canary_spec_resolves_every_shared_provider() {
    let resolved = resolved();
    let bindings: Vec<&str> = resolved
        .panels
        .iter()
        .filter_map(|panel| panel.content.binding_id())
        .collect();
    let shared: Vec<&str> = workspace_canary::CANARY_PROVIDERS
        .iter()
        .map(|(_, _, binding)| *binding)
        .collect();
    assert_eq!(bindings, shared);
}

#[test]
fn every_canary_panel_paints_its_provider_content() {
    let theme = BevyTheme::default();
    for panel in resolved().panels {
        let binding = panel.content.binding_id().expect("canary panels are bound");
        let (texts, mut _app) = paint(&theme, binding, &panel.content);
        assert!(
            !texts.iter().any(|text| text.starts_with("binding:")),
            "{binding} painted the unbound placeholder: {texts:?}"
        );
        if binding == "canary.graph" {
            // A graph has no text to read: every model node is a painted disc.
            let mut dots = _app.world_mut().query::<&GraphNodeDot>();
            assert_eq!(
                dots.iter(_app.world()).count(),
                workspace_canary::canary_graph().nodes.len(),
                "canary.graph must paint one disc per node"
            );
            continue;
        }
        let marker = expected_marker(binding);
        assert!(
            texts
                .iter()
                .any(|text| text.to_lowercase().contains(&marker.to_lowercase())),
            "{binding} must show {marker:?}; painted {texts:?}"
        );
    }
}

#[test]
fn theme_swatches_follow_the_active_theme() {
    let content = ContentSpec::Custom {
        binding: "canary.theme".to_owned(),
    };
    for tokens in [ThemeTokens::dark(), ThemeTokens::paper()] {
        let theme = BevyTheme::from_tokens(&tokens);
        let (_, mut app) = paint(&theme, "canary.theme", &content);
        let mut swatches = app.world_mut().query::<(&Text, &TextColor)>();
        let painted: Vec<Color> = swatches
            .iter(app.world())
            .filter(|(text, _)| text.0.trim() == "##")
            .map(|(_, ink)| ink.0)
            .collect();
        let expected: Vec<Color> = workspace_canary::THEME_SWATCHES
            .iter()
            .map(|&(slot, _)| theme.color(slot))
            .collect();
        assert_eq!(painted, expected);
    }
}

#[test]
fn the_canary_graph_rings_only_the_node_the_host_selected() {
    let theme = BevyTheme::default();
    let panel = resolved()
        .panels
        .into_iter()
        .find(|panel| panel.content.binding_id() == Some("canary.graph"))
        .expect("the canary has a graph panel");
    let rings = |app: &mut App| app.world_mut().query::<&GraphSelectionRing>().iter(app.world()).count();

    let (_, mut none) = paint(&theme, "canary.graph", &panel.content);
    assert_eq!(rings(&mut none), 0, "nothing is selected until a node is pressed");

    let first = workspace_canary::canary_graph().nodes[0].id.clone();
    let data = CanaryData {
        graph_selected: Some(first),
        ..CanaryData::default()
    };
    let (_, mut selected) = paint_with(&theme, "canary.graph", &panel.content, data);
    assert_eq!(rings(&mut selected), 1);
}
