//! The canary's dock controls: snap toggles flip the session snap policy and
//! repaint, and layout reset restores the authored layout and clears the
//! saved record without re-saving it.

#[allow(dead_code)]
#[path = "../examples/workspace_canary.rs"]
mod workspace_canary;
#[path = "../examples/dock_controls.rs"]
mod dock_controls;

use bevy::prelude::*;
use panel_kit_bevy::{save_layout, BevyPanelKit, BevyTheme, DockExtras, FileLayoutStore};
use panel_kit_core::spec::{BackendKind, WorkspaceSpec};
use panel_kit_core::SpecPanelId;

use dock_controls::{paint_dock_controls, press_dock_controls, CanaryDefaults, CanaryStore, DockControl};

const SPEC_JSON: &str = include_str!(env!("PANEL_KIT_WORKSPACE_SPEC"));

/// A world with the canary kit, an empty dock slot, both dock systems, and
/// a layout store at `store`.
fn app(store: &std::path::Path) -> App {
    let resolved = WorkspaceSpec::from_json_str(SPEC_JSON)
        .expect("workspace-canary decodes")
        .resolve(&workspace_canary::provider_manifest(BackendKind::Bevy))
        .expect("workspace-canary resolves")
        .into_logical_px(1440.0, 900.0);
    let kit = BevyPanelKit::new(
        resolved.catalog,
        &resolved.initial.panels,
        resolved.initial.preferred_mode,
        resolved.initial.viewport,
    )
    .expect("canary kit");
    let mut app = App::new();
    app.insert_resource(kit)
        .insert_resource(BevyTheme::default())
        .insert_resource(CanaryStore(FileLayoutStore::new(store)))
        .insert_resource(CanaryDefaults {
            initial: resolved.initial,
            save_policy: resolved.persistence.save_policy,
            skip_save: false,
        })
        .add_systems(Update, (press_dock_controls, paint_dock_controls).chain());
    app.world_mut().spawn((DockExtras, Node::default()));
    app.update();
    app
}

fn button(app: &mut App, control: DockControl) -> Entity {
    let mut query = app.world_mut().query::<(Entity, &DockControl)>();
    query
        .iter(app.world())
        .find(|(_, c)| **c == control)
        .map(|(entity, _)| entity)
        .unwrap_or_else(|| panic!("{control:?} button painted"))
}

/// The glyph and label texts under one button, in spawn order.
fn texts(app: &App, entity: Entity) -> Vec<String> {
    let world = app.world();
    let mut out = Vec::new();
    for child in world.get::<Children>(entity).into_iter().flatten() {
        if let Some(text) = world.get::<Text>(*child) {
            out.push(text.0.clone());
        }
    }
    out
}

fn press(app: &mut App, control: DockControl) {
    let entity = button(app, control);
    app.world_mut().entity_mut(entity).insert(Interaction::Pressed);
    app.update();
    // The press repaints the slot; let the next frame settle it.
    app.update();
}

fn store_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("panel-kit-dock-controls-{name}-{}.json", std::process::id()))
}

#[test]
fn paints_both_snap_toggles_on_and_the_reset_button() {
    let mut app = app(&store_path("paint"));
    let move_snap = button(&mut app, DockControl::MoveSnap);
    let resize_snap = button(&mut app, DockControl::ResizeSnap);
    let reset = button(&mut app, DockControl::ResetLayout);
    assert_eq!(texts(&app, move_snap), ["●", "move"]);
    assert_eq!(texts(&app, resize_snap), ["●", "resize"]);
    assert_eq!(texts(&app, reset), ["reset layout"]);
}

#[test]
fn a_snap_toggle_flips_only_its_flag_and_repaints_its_glyph() {
    let mut app = app(&store_path("snap"));
    press(&mut app, DockControl::MoveSnap);
    let snap = app.world().resource::<BevyPanelKit<SpecPanelId>>().snap;
    assert!(!snap.move_ && snap.resize);
    let move_snap = button(&mut app, DockControl::MoveSnap);
    assert_eq!(texts(&app, move_snap), ["○", "move"]);

    press(&mut app, DockControl::ResizeSnap);
    press(&mut app, DockControl::MoveSnap);
    let snap = app.world().resource::<BevyPanelKit<SpecPanelId>>().snap;
    assert!(snap.move_ && !snap.resize);
}

#[test]
fn reset_restores_the_authored_layout_and_clears_the_saved_record() {
    let path = store_path("reset");
    let mut app = app(&path);
    let authored = app.world().resource::<CanaryDefaults>().initial.panels.clone();
    {
        let mut kit = app.world_mut().resource_mut::<BevyPanelKit<SpecPanelId>>();
        kit.snapshot.panels[0].x += 40.0;
        kit.snapshot.panels[0].h += 25.0;
        let store = FileLayoutStore::new(&path);
        save_layout(&kit, &store).expect("layout saves");
    }
    assert!(path.exists(), "a moved layout is on disk");

    press(&mut app, DockControl::ResetLayout);

    let kit = app.world().resource::<BevyPanelKit<SpecPanelId>>();
    assert!(kit.snapshot.panels == authored, "panels return to the authored layout");
    assert!(!path.exists(), "reset clears the saved record");
    assert!(
        app.world().resource::<CanaryDefaults>().skip_save,
        "the reset frame is not re-saved"
    );
}
