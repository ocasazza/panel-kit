//! The canary's dock controls: move/resize snap toggles and layout reset,
//! painted into the renderer's [`DockExtras`] slot (web `.snap-toggle`).
//! Shared by the Bevy workspace example and its tests.

use bevy::prelude::*;
use panel_kit_bevy::widgets::{styled_text, TextStyle};
use panel_kit_bevy::{BevyPanelKit, BevyTheme, DockExtras, FileLayoutStore};
use panel_kit_core::persist::{apply_save_decision, SavePolicy};
use panel_kit_core::reducer::{ResizePolicy, Snapshot, WorkspaceEvent};
use panel_kit_core::SpecPanelId;

/// Where the canary persists its layout.
#[derive(Resource)]
pub struct CanaryStore(pub FileLayoutStore);

/// Authored defaults for "reset layout" and the spec's save policy.
#[derive(Resource)]
pub struct CanaryDefaults {
    pub initial: Snapshot<SpecPanelId>,
    pub save_policy: SavePolicy,
    /// Set by a reset, whose change must not re-save the defaults.
    pub skip_save: bool,
}

/// The web canary's dock `.snap-toggle` controls.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockControl {
    MoveSnap,
    ResizeSnap,
    ResetLayout,
}

/// `.snap-toggle` metrics in rem: button padding, the glyph–label and
/// button gaps, and the reset button's extra left margin; radius in px.
const DOCK_BUTTON_PAD_REM: (f32, f32) = (0.35, 0.15);
const DOCK_GAP_REM: f32 = 0.25;
const DOCK_FONT_REM: f32 = 0.68;
const DOCK_BUTTON_RADIUS: f32 = 3.0;
/// `--hit-min`.
const HIT_MIN: f32 = 24.0;

/// Paint the snap toggles and layout reset into the dock's host slot, again
/// whenever the snap policy or theme changes.
pub fn paint_dock_controls(
    mut commands: Commands,
    slots: Query<Entity, With<DockExtras>>,
    kit: Res<BevyPanelKit<SpecPanelId>>,
    theme: Res<BevyTheme>,
    mut painted: Local<Option<(bool, bool)>>,
) {
    let Ok(slot) = slots.single() else {
        return;
    };
    let state = (kit.snap.move_, kit.snap.resize);
    if *painted == Some(state) && !theme.is_changed() {
        return;
    }
    *painted = Some(state);
    let theme = theme.into_inner();
    commands
        .entity(slot)
        .despawn_related::<Children>()
        .with_children(|slot| {
            slot.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(theme.metrics.rem(DOCK_GAP_REM)),
                ..default()
            })
            .with_children(|group| {
                dock_button(group, theme, DockControl::MoveSnap, Some(state.0), "move");
                dock_button(group, theme, DockControl::ResizeSnap, Some(state.1), "resize");
                dock_button(group, theme, DockControl::ResetLayout, None, "reset layout");
            });
        });
}

/// One `.snap-toggle button`: `on` carries the toggle state, `None` is the
/// reset button.
fn dock_button(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    control: DockControl,
    on: Option<bool>,
    label: &str,
) {
    let style = TextStyle::rem(theme, DOCK_FONT_REM);
    let gap = theme.metrics.rem(DOCK_GAP_REM);
    let ink = if on == Some(false) {
        theme.text_muted
    } else {
        theme.text
    };
    parent
        .spawn((
            control,
            Button,
            Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(gap),
                min_height: Val::Px(HIT_MIN),
                padding: UiRect::axes(
                    Val::Px(theme.metrics.rem(DOCK_BUTTON_PAD_REM.0)),
                    Val::Px(theme.metrics.rem(DOCK_BUTTON_PAD_REM.1)),
                ),
                margin: UiRect::left(Val::Px(if on.is_none() { gap } else { 0.0 })),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor(Color::NONE),
            BorderRadius::all(Val::Px(DOCK_BUTTON_RADIUS)),
        ))
        .with_children(|button| {
            if let Some(on) = on {
                let glyph = if on { "●" } else { "○" };
                let tint = if on { theme.highlight } else { theme.text_muted };
                styled_text(button, glyph, style.color(tint));
            }
            styled_text(button, label, style.color(ink));
        });
}

/// Dock control presses: flip a snap flag, or reset to the authored layout
/// and clear the saved record without re-saving it.
pub fn press_dock_controls(
    controls: Query<(&Interaction, &DockControl, &mut BorderColor), Changed<Interaction>>,
    mut kit: ResMut<BevyPanelKit<SpecPanelId>>,
    mut defaults: ResMut<CanaryDefaults>,
    store: Res<CanaryStore>,
    theme: Res<BevyTheme>,
) {
    for (interaction, control, mut edge) in controls {
        edge.0 = match interaction {
            Interaction::None => Color::NONE,
            Interaction::Hovered | Interaction::Pressed => theme.border,
        };
        if *interaction != Interaction::Pressed {
            continue;
        }
        match control {
            DockControl::MoveSnap => kit.snap.move_ = !kit.snap.move_,
            DockControl::ResizeSnap => kit.snap.resize = !kit.snap.resize,
            DockControl::ResetLayout => {
                let viewport = kit.snapshot.viewport;
                if let Err(error) = apply_save_decision(
                    defaults.save_policy.reset_decision(),
                    &store.0,
                    &defaults.initial,
                    &kit.catalog,
                ) {
                    eprintln!("clearing the saved layout failed: {error}");
                }
                kit.snapshot = defaults.initial.clone();
                kit.apply_workspace_event(WorkspaceEvent::ViewportChanged {
                    size: viewport,
                    policy: ResizePolicy::ScaleFloating,
                });
                defaults.skip_save = true;
            }
        }
    }
}
