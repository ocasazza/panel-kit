//! Input translation from Bevy events to `panel-kit-core` events.
//!
//! Bevy remains the event source; core remains the only state owner. Pointer
//! events are hit-tested against the same projected frame the renderer uses,
//! and keyboard chords are resolved through core's [`command_for`] mapping.

use bevy::input::keyboard::{Key as BevyKey, KeyCode, KeyboardInput};
use bevy::input::mouse::{MouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::camera::{Camera, RenderTarget};
use bevy::window::{CursorLeft, CursorMoved, PrimaryWindow, WindowRef, WindowResized};

use panel_kit_core::frame::{
    hit_test, project_into, ChromeProjectionInput, FrameStatus, ProjectionBuffer, ProjectionInput,
    TileLayoutMetrics,
};
use panel_kit_core::reducer::{
    reduce, ChangePhase, HitTarget, PanelPart, ReduceContext, Viewport, WheelDisposition,
    WorkspaceEvent,
};
use panel_kit_core::{
    ChromeMetrics, FocusContext, Key, KeyChord, PanelCommand, PointerButton, PointerEvent,
    PointerEventKind, SurfaceProfile,
};

use crate::render::PanelUiTargetCamera;
use crate::{BevyPanelKey, BevyPanelKit};

/// Projection scratch wrapped for Bevy's `Local` system parameter.
pub struct InputScratch<K: BevyPanelKey>(ProjectionBuffer<K>);

impl<K: BevyPanelKey> Default for InputScratch<K> {
    fn default() -> Self {
        Self(ProjectionBuffer::with_panel_capacity(0))
    }
}

/// Pointer and keyboard systems for a [`BevyPanelKit`] workspace.
///
/// Event resources are registered here so the input systems are usable with a
/// minimal app as well as alongside `DefaultPlugins`.
pub struct PanelInputPlugin<K: BevyPanelKey> {
    _marker: std::marker::PhantomData<fn() -> K>,
}

impl<K: BevyPanelKey> Default for PanelInputPlugin<K> {
    fn default() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<K: BevyPanelKey> Plugin for PanelInputPlugin<K> {
    fn build(&self, app: &mut App) {
        app.add_event::<CursorMoved>()
            .add_event::<CursorLeft>()
            .add_event::<WindowResized>()
            .add_event::<MouseButtonInput>()
            .add_event::<MouseWheel>()
            .add_event::<KeyboardInput>()
            .add_systems(
                Update,
                (
                    (
                        pointer_input_system::<K>,
                        keyboard_input_system::<K>,
                        tile_drag_tick_system::<K>,
                    )
                        .chain(),
                    resize_input_system::<K>,
                ),
            );
    }
}

/// Advance tile-drag animation by Bevy's frame time. Runs only while
/// [`TileDrag::animating`](panel_kit_core::tile_drag::TileDrag::animating)
/// holds, so an idle workspace is never marked changed.
pub fn tile_drag_tick_system<K: BevyPanelKey>(
    mut kit: ResMut<BevyPanelKit<K>>,
    time: Option<Res<Time>>,
) {
    let Some(time) = time else {
        return;
    };
    if !kit.snapshot.tile_drag.is_some_and(|tile| tile.animating()) {
        return;
    }
    let dt_ms = time.delta_secs_f64() * 1000.0;
    reduce_event(&mut kit, WorkspaceEvent::Tick { dt_ms });
}

/// Current Bevy modifier state, updated from keyboard events.
#[derive(Clone, Copy, Default)]
pub struct ModifierState {
    shift: bool,
    alt: bool,
    ctrl: bool,
    meta: bool,
}

impl ModifierState {
    fn observe(&mut self, event: &KeyboardInput) {
        if event.repeat {
            return;
        }
        let pressed = event.state == ButtonState::Pressed;
        match event.key_code {
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.shift = pressed,
            KeyCode::AltLeft | KeyCode::AltRight => self.alt = pressed,
            KeyCode::ControlLeft | KeyCode::ControlRight => self.ctrl = pressed,
            KeyCode::SuperLeft | KeyCode::SuperRight => self.meta = pressed,
            _ => {}
        }
    }
}

/// Translate one logical Bevy key to core's key.
fn translate_key(logical: &BevyKey) -> Option<Key> {
    match logical {
        BevyKey::ArrowLeft => Some(Key::Left),
        BevyKey::ArrowRight => Some(Key::Right),
        BevyKey::ArrowUp => Some(Key::Up),
        BevyKey::ArrowDown => Some(Key::Down),
        BevyKey::Enter => Some(Key::Enter),
        BevyKey::Escape => Some(Key::Escape),
        BevyKey::Tab => Some(Key::Tab),
        BevyKey::Character(text) => text
            .chars()
            .next()
            .map(|c| Key::Char(c.to_ascii_lowercase())),
        _ => None,
    }
}

/// Translate one Bevy keyboard event into a core chord.
fn translate_chord(event: &KeyboardInput, modifiers: ModifierState) -> Option<KeyChord> {
    if event.state != ButtonState::Pressed {
        return None;
    }
    let key = translate_key(&event.logical_key)?;
    Some(KeyChord {
        key,
        shift: modifiers.shift,
        alt: modifiers.alt,
        ctrl: modifiers.ctrl,
        meta: modifiers.meta,
    })
}

/// Whether a body hit must swallow motion: true only when no chrome gesture
/// is in flight. An active drag or resize owns motion across every region,
/// including panel bodies, so it can follow the pointer over content.
fn body_owns_motion<K: BevyPanelKey>(
    snapshot: &panel_kit_core::reducer::Snapshot<K>,
    target: &Option<HitTarget<K>>,
) -> bool {
    if snapshot.drag.is_some() || snapshot.tile_drag.is_some() {
        return false;
    }
    matches!(
        target,
        Some(HitTarget::Panel {
            part: PanelPart::Surface,
            ..
        })
    )
}

/// Map a traffic-light hit to its window-management command.
fn control_command<K: BevyPanelKey>(target: &Option<HitTarget<K>>) -> Option<PanelCommand> {
    let HitTarget::Panel { part, .. } = target.as_ref()? else {
        return None;
    };
    match part {
        PanelPart::ModeControl => Some(PanelCommand::ToggleMode),
        PanelPart::MinimizeControl => Some(PanelCommand::Minimize),
        PanelPart::MaximizeControl => Some(PanelCommand::Maximize),
        _ => None,
    }
}

/// Project the current snapshot and hit-test one logical-UI-pixel point.
fn hit_target<K: BevyPanelKey>(
    kit: &BevyPanelKit<K>,
    scratch: &mut ProjectionBuffer<K>,
    point: (f64, f64),
) -> Option<HitTarget<K>> {
    let chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
    let tile = TileLayoutMetrics::from_tile_metrics(kit.tile, kit.surface);
    let input = ProjectionInput {
        snapshot: &kit.snapshot,
        surface: kit.surface,
        chrome: &chrome,
        clamp: &kit.clamp,
        tile: &tile,
    };
    let frame = project_into(input, scratch);
    (frame.status == FrameStatus::Ready).then(|| hit_test(&frame, point))?
}

/// The window whose events the workspace accepts: `None` accepts every window
/// (the single-window behaviour); with a [`PanelUiTargetCamera`] only that
/// camera's window. A named camera that does not render to a window (not
/// spawned yet, or rendering to an image) matches no window at all, which
/// pairs with the renderer drawing such a workspace nowhere.
fn target_window(
    target: Option<&PanelUiTargetCamera>,
    cameras: &Query<&Camera>,
    primary: &Query<Entity, With<PrimaryWindow>>,
) -> Option<Entity> {
    let target = target?;
    let window = cameras
        .get(target.0)
        .ok()
        .and_then(|camera| match camera.target {
            RenderTarget::Window(WindowRef::Entity(window)) => Some(window),
            RenderTarget::Window(WindowRef::Primary) => primary.single().ok(),
            _ => None,
        });
    Some(window.unwrap_or(Entity::PLACEHOLDER))
}

fn accepts(target: Option<Entity>, window: Entity) -> bool {
    target.is_none_or(|target| target == window)
}

/// Reduce one event while keeping all unit metrics in core's context shape.
fn reduce_event<K: BevyPanelKey>(
    kit: &mut BevyPanelKit<K>,
    event: WorkspaceEvent<K>,
) -> ChangePhase {
    let (surface, clamp, command_step, tile, snap) =
        (kit.surface, kit.clamp, kit.command_step, kit.tile, kit.snap);
    let reduction = reduce(
        &mut kit.snapshot,
        event,
        ReduceContext {
            surface,
            clamp: &clamp,
            command_step,
            tile: &tile,
            snap,
        },
    );
    reduction.phase.unwrap_or(ChangePhase::Settled)
}

/// Translate cursor motion and mouse buttons/wheel into core pointer events.
///
/// Body hits are left to application content: a [`PanelBody`] owns its own
/// interactions. Chrome gestures (header, controls, grips) and dock chips are
/// reduced by core. Once a chrome gesture is in flight, motion is forwarded to
/// core even when the pointer crosses a body so the gesture can complete; a
/// held or lifted tile drag resolves motion against resting slots through
/// core's hit test. The cursor leaving the window cancels the gesture.
#[allow(clippy::too_many_arguments)]
pub fn pointer_input_system<K: BevyPanelKey>(
    mut kit: ResMut<BevyPanelKit<K>>,
    windows: Query<&Window>,
    target: Option<Res<PanelUiTargetCamera>>,
    cameras: Query<&Camera>,
    primary: Query<Entity, With<PrimaryWindow>>,
    mut cursors: EventReader<CursorMoved>,
    mut left: EventReader<CursorLeft>,
    mut buttons: EventReader<MouseButtonInput>,
    mut wheels: EventReader<MouseWheel>,
    mut scratch: Local<InputScratch<K>>,
    mut dragging: Local<bool>,
) {
    let accepted = target_window(target.as_deref(), &cameras, &primary);
    for event in buttons.read() {
        if !accepts(accepted, event.window) {
            continue;
        }
        let Ok(window) = windows.get(event.window) else {
            continue;
        };
        let Some(point) = window.cursor_position() else {
            continue;
        };
        let point = (point.x as f64, point.y as f64);

        if event.button != MouseButton::Left {
            continue;
        }
        match event.state {
            ButtonState::Pressed => {
                *dragging = true;
                let target = hit_target(&kit, &mut scratch.0, point);
                // Traffic lights are command origins, not focus gestures: the
                // reducer treats control parts as focus-only, so hosts translate
                // a light hit into the corresponding command (web-controller
                // `execute_command` semantics).
                if let Some(command) = control_command(&target) {
                    *dragging = false;
                    reduce_event(
                        &mut kit,
                        WorkspaceEvent::Command {
                            target: match target {
                                Some(HitTarget::Panel { key, .. }) => Some(key),
                                _ => None,
                            },
                            command,
                        },
                    );
                    continue;
                }
                if matches!(
                    target,
                    Some(HitTarget::Panel {
                        part: PanelPart::Surface,
                        ..
                    })
                ) {
                    continue;
                }
                reduce_event(
                    &mut kit,
                    WorkspaceEvent::Pointer {
                        target: target.unwrap_or(HitTarget::Workspace),
                        event: PointerEvent {
                            kind: PointerEventKind::Down(PointerButton::Primary),
                            x: point.0,
                            y: point.1,
                        },
                    },
                );
            }
            ButtonState::Released => {
                *dragging = false;
                reduce_event(
                    &mut kit,
                    WorkspaceEvent::Pointer {
                        target: HitTarget::Workspace,
                        event: PointerEvent {
                            kind: PointerEventKind::Up(PointerButton::Primary),
                            x: point.0,
                            y: point.1,
                        },
                    },
                );
            }
        }
    }

    for event in cursors.read() {
        if !*dragging || !accepts(accepted, event.window) {
            continue;
        }
        let point = (event.position.x as f64, event.position.y as f64);
        let target = hit_target(&kit, &mut scratch.0, point);
        if body_owns_motion(&kit.snapshot, &target) {
            continue;
        }
        reduce_event(
            &mut kit,
            WorkspaceEvent::Pointer {
                target: target.unwrap_or(HitTarget::Workspace),
                event: PointerEvent {
                    kind: PointerEventKind::Drag(PointerButton::Primary),
                    x: point.0,
                    y: point.1,
                },
            },
        );
    }

    if left.read().any(|event| accepts(accepted, event.window)) && *dragging {
        reduce_event(
            &mut kit,
            WorkspaceEvent::Pointer {
                target: HitTarget::Workspace,
                event: PointerEvent {
                    kind: PointerEventKind::Cancel,
                    x: 0.0,
                    y: 0.0,
                },
            },
        );
    }

    for event in wheels.read() {
        if !accepts(accepted, event.window) {
            continue;
        }
        let Ok(window) = windows.get(event.window) else {
            continue;
        };
        let Some(point) = window.cursor_position() else {
            continue;
        };
        let point = (point.x as f64, point.y as f64);
        let target = hit_target(&kit, &mut scratch.0, point);
        if matches!(
            target,
            Some(HitTarget::Panel {
                part: PanelPart::Surface,
                ..
            })
        ) {
            continue;
        }
        let delta_y = match event.unit {
            MouseScrollUnit::Line => event.y as f64 * 24.0,
            MouseScrollUnit::Pixel => event.y as f64,
        };
        reduce_event(
            &mut kit,
            WorkspaceEvent::Wheel {
                delta_y,
                disposition: WheelDisposition::BubbleToWorkspace,
            },
        );
    }
}

/// Translate Bevy keyboard events into core chords and apply the resulting
/// commands through [`reduce`].
pub fn keyboard_input_system<K: BevyPanelKey>(
    mut kit: ResMut<BevyPanelKit<K>>,
    target: Option<Res<PanelUiTargetCamera>>,
    cameras: Query<&Camera>,
    primary: Query<Entity, With<PrimaryWindow>>,
    mut keys: EventReader<KeyboardInput>,
    mut modifiers: Local<ModifierState>,
) {
    let accepted = target_window(target.as_deref(), &cameras, &primary);
    for event in keys.read() {
        if !accepts(accepted, event.window) {
            continue;
        }
        modifiers.observe(event);
        if event.repeat {
            continue;
        }
        let chord = translate_chord(event, *modifiers);
        let Some(chord) = chord else {
            continue;
        };
        reduce_event(
            &mut kit,
            WorkspaceEvent::Key {
                chord,
                focus: FocusContext::Workspace,
            },
        );
    }
}

/// Keep the core viewport and surface profile synchronized with the Bevy
/// window. Stored floating intent is preserved; the renderer projects it
/// against the new viewport.
pub fn resize_input_system<K: BevyPanelKey>(
    mut kit: ResMut<BevyPanelKit<K>>,
    target: Option<Res<PanelUiTargetCamera>>,
    cameras: Query<&Camera>,
    primary: Query<Entity, With<PrimaryWindow>>,
    mut resized: EventReader<WindowResized>,
) {
    let accepted = target_window(target.as_deref(), &cameras, &primary);
    for event in resized.read() {
        if !accepts(accepted, event.window) {
            continue;
        }
        if !event.width.is_finite()
            || !event.height.is_finite()
            || event.width <= 0.0
            || event.height <= 0.0
        {
            continue;
        }
        let viewport = Viewport {
            width: event.width as f64,
            height: event.height as f64,
            units: panel_kit_core::Units::CssPx,
        };
        kit.surface = SurfaceProfile::from_logical_width(
            viewport.width,
            panel_kit_core::WEB_COMPACT_MAX,
            panel_kit_core::WEB_TABLET_MAX,
            panel_kit_core::SurfaceCapabilities {
                coarse_pointer: false,
                hover: true,
                keyboard: true,
            },
        );
        let policy = kit.resize_policy;
        reduce_event(
            &mut kit,
            WorkspaceEvent::ViewportChanged {
                size: viewport,
                policy,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;
    use bevy::ecs::event::Events;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    enum TestPanel {
        A,
    }

    impl panel_kit_core::PanelKind for TestPanel {
        fn title(self) -> &'static str {
            "A"
        }
    }

    fn kit() -> BevyPanelKit<TestPanel> {
        let mut layout = panel_kit_core::LayoutBuilder::new();
        let panels = vec![layout.at(TestPanel::A, 40.0, 40.0, 640.0, 480.0)];
        let catalog = panel_kit_core::PanelCatalog::from_panel_kind_layout(&panels).unwrap();
        BevyPanelKit::new(
            catalog,
            &panels,
            panel_kit_core::Mode::Floating,
            Viewport {
                width: 1280.0,
                height: 800.0,
                units: panel_kit_core::Units::CssPx,
            },
        )
        .unwrap()
    }

    fn app_with_kit() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(PanelInputPlugin::<TestPanel>::default());
        app.insert_resource(kit());

        let window = app
            .world_mut()
            .spawn(Window {
                resolution: (1280.0, 800.0).into(),
                ..default()
            })
            .id();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(0.0, 0.0)));
        (app, window)
    }

    fn set_cursor(app: &mut App, window: Entity, point: Vec2) {
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(point));
    }

    fn send_mouse(app: &mut App, window: Entity, state: ButtonState) {
        app.world_mut()
            .resource_mut::<Events<MouseButtonInput>>()
            .send(MouseButtonInput {
                button: MouseButton::Left,
                state,
                window,
            });
    }

    fn send_cursor(app: &mut App, window: Entity, point: Vec2) {
        app.world_mut()
            .resource_mut::<Events<CursorMoved>>()
            .send(CursorMoved {
                window,
                position: point,
                delta: None,
            });
    }

    #[test]
    fn header_pointer_down_focuses_panel() {
        let (mut app, window) = app_with_kit();
        set_cursor(&mut app, window, Vec2::new(100.0, 50.0));
        send_mouse(&mut app, window, ButtonState::Pressed);
        app.update();

        let kit = app.world().resource::<BevyPanelKit<TestPanel>>();
        assert_eq!(kit.snapshot.focused, Some(TestPanel::A));
    }

    #[test]
    fn a_target_camera_that_is_not_a_window_camera_accepts_no_window() {
        let (mut app, window) = app_with_kit();
        app.insert_resource(PanelUiTargetCamera(Entity::PLACEHOLDER));
        set_cursor(&mut app, window, Vec2::new(100.0, 50.0));
        send_mouse(&mut app, window, ButtonState::Pressed);
        app.update();
        let kit = app.world().resource::<BevyPanelKit<TestPanel>>();
        assert_eq!(kit.snapshot.focused, None);
    }

    #[test]
    fn with_a_target_camera_only_its_window_drives_the_workspace() {
        let (mut app, window) = app_with_kit();
        let other = app
            .world_mut()
            .spawn(Window {
                resolution: (1280.0, 800.0).into(),
                ..default()
            })
            .id();
        let camera = app
            .world_mut()
            .spawn(Camera {
                target: RenderTarget::Window(WindowRef::Entity(window)),
                ..default()
            })
            .id();
        app.insert_resource(PanelUiTargetCamera(camera));

        set_cursor(&mut app, other, Vec2::new(100.0, 50.0));
        send_mouse(&mut app, other, ButtonState::Pressed);
        app.update();
        let kit = app.world().resource::<BevyPanelKit<TestPanel>>();
        assert_eq!(
            kit.snapshot.focused, None,
            "a press in another window is ignored"
        );

        set_cursor(&mut app, window, Vec2::new(100.0, 50.0));
        send_mouse(&mut app, window, ButtonState::Pressed);
        app.update();
        let kit = app.world().resource::<BevyPanelKit<TestPanel>>();
        assert_eq!(kit.snapshot.focused, Some(TestPanel::A));
    }

    #[test]
    fn with_a_target_camera_only_its_window_resizes_the_viewport() {
        let (mut app, window) = app_with_kit();
        let other = app.world_mut().spawn_empty().id();
        let camera = app
            .world_mut()
            .spawn(Camera {
                target: RenderTarget::Window(WindowRef::Entity(window)),
                ..default()
            })
            .id();
        app.insert_resource(PanelUiTargetCamera(camera));
        let resize = |window, width, height| WindowResized {
            window,
            width,
            height,
        };

        app.world_mut()
            .resource_mut::<Events<WindowResized>>()
            .send(resize(other, 300.0, 200.0));
        app.update();
        let viewport = app
            .world()
            .resource::<BevyPanelKit<TestPanel>>()
            .snapshot
            .viewport;
        assert_eq!((viewport.width, viewport.height), (1280.0, 800.0));

        app.world_mut()
            .resource_mut::<Events<WindowResized>>()
            .send(resize(window, 640.0, 360.0));
        app.update();
        let viewport = app
            .world()
            .resource::<BevyPanelKit<TestPanel>>()
            .snapshot
            .viewport;
        assert_eq!((viewport.width, viewport.height), (640.0, 360.0));
    }

    #[test]
    fn body_pointer_down_is_owned_by_body() {
        let (mut app, window) = app_with_kit();
        set_cursor(&mut app, window, Vec2::new(100.0, 100.0));
        send_mouse(&mut app, window, ButtonState::Pressed);
        app.update();

        let kit = app.world().resource::<BevyPanelKit<TestPanel>>();
        assert_eq!(kit.snapshot.focused, None);
    }

    #[test]
    fn header_drag_moves_panel_through_core() {
        let (mut app, window) = app_with_kit();
        set_cursor(&mut app, window, Vec2::new(300.0, 50.0));
        send_mouse(&mut app, window, ButtonState::Pressed);
        send_cursor(&mut app, window, Vec2::new(320.0, 50.0));
        app.update();

        let kit = app.world().resource::<BevyPanelKit<TestPanel>>();
        assert_eq!(kit.snapshot.panels[0].x, 64.0);
    }

    #[test]
    fn keyboard_event_maps_a_core_command() {
        let (mut app, window) = app_with_kit();
        set_cursor(&mut app, window, Vec2::new(300.0, 50.0));
        send_mouse(&mut app, window, ButtonState::Pressed);
        app.world_mut()
            .resource_mut::<Events<KeyboardInput>>()
            .send(KeyboardInput {
                key_code: KeyCode::KeyM,
                logical_key: BevyKey::Character("m".into()),
                state: ButtonState::Pressed,
                text: Some("m".into()),
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        app.update();

        let kit = app.world().resource::<BevyPanelKit<TestPanel>>();
        assert_eq!(
            kit.snapshot.panels[0].state,
            panel_kit_core::WinState::Minimized
        );
    }

    #[test]
    fn tiling_resize_follows_the_pointer_across_the_body() {
        use panel_kit_core::frame::{
            project_into, ChromeProjectionInput, ProjectionBuffer, ProjectionInput,
            TileLayoutMetrics,
        };
        use panel_kit_core::{ChromeMetrics, Mode};

        let (mut app, window) = app_with_kit();
        app.world_mut()
            .resource_mut::<BevyPanelKit<TestPanel>>()
            .snapshot
            .preferred_mode = Mode::Tiling;

        let grip = {
            let kit = app.world().resource::<BevyPanelKit<TestPanel>>();
            let chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
            let tile = TileLayoutMetrics::from_tile_metrics(kit.tile, kit.surface);
            let mut scratch = ProjectionBuffer::with_panel_capacity(1);
            let frame = project_into(
                ProjectionInput {
                    snapshot: &kit.snapshot,
                    surface: kit.surface,
                    chrome: &chrome,
                    clamp: &kit.clamp,
                    tile: &tile,
                },
                &mut scratch,
            );
            frame.panels[0]
                .chrome
                .resize_hit
                .expect("tiling panels expose a resize grip")
        };

        let start = Vec2::new((grip.x + grip.w - 3.0) as f32, (grip.y + grip.h - 3.0) as f32);
        {
            let kit = app.world().resource::<BevyPanelKit<TestPanel>>();
            let mut scratch = ProjectionBuffer::with_panel_capacity(1);
            let hit = super::hit_target(&kit, &mut scratch, (start.x as f64, start.y as f64));
            assert_eq!(
                hit,
                Some(HitTarget::Panel {
                    key: TestPanel::A,
                    part: PanelPart::ResizeGrip
                }),
                "grip point {start:?} must hit the resize grip"
            );
        }
        set_cursor(&mut app, window, start);
        send_mouse(&mut app, window, ButtonState::Pressed);
        app.update();
        let after_press = app.world().resource::<BevyPanelKit<TestPanel>>().snapshot.drag;
        assert!(after_press.is_some(), "press must start a tile resize");
        // Move well past the grip and across the panel body.
        send_cursor(&mut app, window, start + Vec2::new(250.0, 0.0));
        app.update();

        let kit = app.world().resource::<BevyPanelKit<TestPanel>>();
        assert!(
            kit.snapshot.panels[0].tile_w > 1,
            "tile span should grow when the pointer leaves the grip: {:?}",
            kit.snapshot.panels[0].tile_w
        );
    }

    #[test]
    fn active_gestures_keep_motion_over_bodies() {
        let mut kit = kit();
        let target = Some(HitTarget::Panel {
            key: TestPanel::A,
            part: PanelPart::Surface,
        });
        assert!(body_owns_motion(&kit.snapshot, &target));
        kit.snapshot.drag = Some(panel_kit_core::Drag {
            idx: 0,
            kind: panel_kit_core::DragKind::Resize,
            start_mx: 0.0,
            start_my: 0.0,
            start_x: 40.0,
            start_y: 40.0,
            start_w: 640.0,
            start_h: 480.0,
        });
        assert!(!body_owns_motion(&kit.snapshot, &target));
    }

    #[test]
    fn modifier_state_is_tracked_from_events() {
        let mut modifiers = ModifierState::default();
        let shift = KeyboardInput {
            key_code: KeyCode::ShiftLeft,
            logical_key: BevyKey::Shift,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        };
        modifiers.observe(&shift);
        assert!(modifiers.shift);
        let chord_event = KeyboardInput {
            key_code: KeyCode::ArrowLeft,
            logical_key: BevyKey::ArrowLeft,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        };
        let chord = translate_chord(&chord_event, modifiers).unwrap();
        assert_eq!(chord.key, Key::Left);
        assert!(chord.shift);
    }

    #[test]
    fn resize_updates_viewport_without_rescaling_intent() {
        let (mut app, _window) = app_with_kit();
        let (expected_x, expected_w) = (kit().snapshot.panels[0].x, kit().snapshot.panels[0].w);
        app.world_mut()
            .resource_mut::<Events<WindowResized>>()
            .send(WindowResized {
                width: 900.0,
                height: 700.0,
                window: Entity::PLACEHOLDER,
            });
        app.update();

        let kit = app.world().resource::<BevyPanelKit<TestPanel>>();
        assert_eq!(kit.snapshot.viewport.width, 900.0);
        assert_eq!(kit.snapshot.viewport.height, 700.0);
        assert_eq!(kit.snapshot.panels[0].x, expected_x);
        assert_eq!(kit.snapshot.panels[0].w, expected_w);
    }

    mod tile_drag {
        use super::{send_cursor, send_mouse, set_cursor};
        use crate::input::PanelInputPlugin;
        use crate::BevyPanelKit;
        use bevy::app::App;
        use bevy::ecs::event::Events;
        use bevy::input::ButtonState;
        use bevy::prelude::*;
        use bevy::window::CursorLeft;
        use panel_kit_core::frame::{
            project_into, ChromeProjectionInput, ProjectionBuffer, ProjectionInput,
            TileLayoutMetrics,
        };
        use panel_kit_core::reducer::Viewport;
        use panel_kit_core::tile_drag::TileDragPhase;
        use panel_kit_core::{ChromeMetrics, LayoutBuilder, Mode, PanelCatalog, Region, Units};

        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        enum Tile {
            Left,
            Right,
        }

        impl panel_kit_core::PanelKind for Tile {
            fn title(self) -> &'static str {
                match self {
                    Tile::Left => "Left",
                    Tile::Right => "Right",
                }
            }
        }

        /// Two side-by-side 2x2 tiles in a 1280x800 window.
        fn tiling_app() -> (App, Entity) {
            let mut layout = LayoutBuilder::new();
            let panels = vec![
                layout
                    .at(Tile::Left, 0.0, 0.0, 300.0, 200.0)
                    .with_tile(2, 2),
                layout
                    .at(Tile::Right, 0.0, 0.0, 300.0, 200.0)
                    .with_tile(2, 2),
            ];
            let catalog = PanelCatalog::from_panel_kind_layout(&panels).unwrap();
            let kit = BevyPanelKit::new(
                catalog,
                &panels,
                Mode::Tiling,
                Viewport {
                    width: 1280.0,
                    height: 800.0,
                    units: Units::CssPx,
                },
            )
            .unwrap();
            let mut app = App::new();
            app.add_plugins(PanelInputPlugin::<Tile>::default());
            app.insert_resource(kit);
            let window = app
                .world_mut()
                .spawn(Window {
                    resolution: (1280.0, 800.0).into(),
                    ..default()
                })
                .id();
            (app, window)
        }

        fn regions(app: &App) -> (Region, Region) {
            let kit = app.world().resource::<BevyPanelKit<Tile>>();
            let chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
            let tile = TileLayoutMetrics::from_tile_metrics(kit.tile, kit.surface);
            let mut scratch = ProjectionBuffer::with_panel_capacity(2);
            let frame = project_into(
                ProjectionInput {
                    snapshot: &kit.snapshot,
                    surface: kit.surface,
                    chrome: &chrome,
                    clamp: &kit.clamp,
                    tile: &tile,
                },
                &mut scratch,
            );
            let find = |key| frame.panels.iter().find(|panel| panel.key == key).unwrap();
            (
                find(Tile::Left).chrome.header_hit,
                find(Tile::Right).chrome.body,
            )
        }

        /// Press Left's header and move into Right's body.
        fn lift_left_over_right(app: &mut App, window: Entity) -> Vec2 {
            let (header, right_body) = regions(app);
            let press = Vec2::new((header.x + header.w / 2.0) as f32, (header.y + 4.0) as f32);
            set_cursor(app, window, press);
            send_mouse(app, window, ButtonState::Pressed);
            app.update();
            let over = Vec2::new(
                (right_body.x + right_body.w / 2.0) as f32,
                (right_body.y + right_body.h / 2.0) as f32,
            );
            set_cursor(app, window, over);
            send_cursor(app, window, over);
            app.update();
            over
        }

        fn order(app: &App) -> Vec<Tile> {
            let kit = app.world().resource::<BevyPanelKit<Tile>>();
            kit.snapshot.panels.iter().map(|panel| panel.kind).collect()
        }

        fn phase(app: &App) -> Option<TileDragPhase> {
            let kit = app.world().resource::<BevyPanelKit<Tile>>();
            kit.snapshot.tile_drag.map(|tile| tile.phase)
        }

        #[test]
        fn motion_over_a_body_lifts_and_release_swaps() {
            let (mut app, window) = tiling_app();
            lift_left_over_right(&mut app, window);
            assert_eq!(phase(&app), Some(TileDragPhase::Lifted));
            assert_eq!(
                app.world()
                    .resource::<BevyPanelKit<Tile>>()
                    .snapshot
                    .tile_drag
                    .and_then(|tile| tile.target),
                Some(Tile::Right)
            );
            assert_eq!(order(&app), [Tile::Left, Tile::Right]);

            send_mouse(&mut app, window, ButtonState::Released);
            app.update();
            assert_eq!(order(&app), [Tile::Right, Tile::Left]);
            assert_eq!(phase(&app), Some(TileDragPhase::Settling));
        }

        #[test]
        fn the_cursor_leaving_the_window_cancels_the_lift() {
            let (mut app, window) = tiling_app();
            lift_left_over_right(&mut app, window);
            app.world_mut()
                .resource_mut::<Events<CursorLeft>>()
                .send(CursorLeft { window });
            app.update();
            assert_eq!(phase(&app), Some(TileDragPhase::Cancelling));

            send_mouse(&mut app, window, ButtonState::Released);
            app.update();
            assert_eq!(order(&app), [Tile::Left, Tile::Right]);
        }

        #[test]
        fn bevy_time_advances_the_lift() {
            let (mut app, window) = tiling_app();
            app.insert_resource(Time::<()>::default());
            lift_left_over_right(&mut app, window);
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(70));
            app.update();
            let lift = app
                .world()
                .resource::<BevyPanelKit<Tile>>()
                .snapshot
                .tile_drag
                .map(|tile| tile.lift)
                .unwrap();
            assert!(lift > 0.0, "lift progress {lift}");
        }
    }
}
