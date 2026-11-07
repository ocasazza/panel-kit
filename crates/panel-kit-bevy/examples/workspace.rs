//! Spec-driven workspace canary on the native Bevy backend.
//!
//! Executable documentation, the Bevy twin of the TUI and web canaries: it
//! decodes the Nix-authored `workspace-canary` spec from
//! `PANEL_KIT_WORKSPACE_SPEC`, resolves it against the shared canary provider
//! manifest, themes the UI from the spec's tokens, and paints every
//! `canary.*` binding through `panel-kit-bevy` widgets fed by the shared
//! canary content (`examples/support/canary_content.rs`).
//!
//! The spec's geometry is authored in cells; core's
//! `ResolvedWorkspace::into_logical_px` maps it onto the window's logical
//! pixels, the same conversion the web host applies to the browser window.
//! `p` switches between the spec theme and the paper preset; `q`
//! quits. Set `PANEL_KIT_BEVY_SCREENSHOT=<path.png>` to render the workspace
//! offscreen at 1440×900 logical pixels and scale factor 2 (independent of the
//! display the window lands on) with the initial metric samples, save it to
//! that path plus every UI node's rect, text and paint to
//! `<path.png>.nodes.jsonl`, and exit. Run inside the dev shell, which sets
//! the spec path: `nix develop -c cargo run -p panel-kit-bevy --example workspace`.

use std::collections::HashMap;

use bevy::app::AppExit;
use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::math::FloatOrd;
use bevy::prelude::*;
use bevy::render::camera::{ImageRenderTarget, RenderTarget};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{save_to_disk, Screenshot, ScreenshotCaptured};
use bevy::ui::IsDefaultUiCamera;
use bevy::window::{ExitCondition, PrimaryWindow, WindowPlugin};

use panel_kit_bevy::fonts::{install_theme_font, BOLD_FONT};
use panel_kit_bevy::widgets::scroll::ScrollPlugin;
use panel_kit_bevy::widgets::data_table::DataTableExport;
use panel_kit_bevy::widgets::time_series::TimeSeriesPlugin;
use panel_kit_core::widgets::data_table::ExportFormat;
use panel_kit_bevy::{
    restore_layout, save_layout, BevyPanelKit, BevyTheme, FileLayoutStore, PanelBodyHandles,
    PanelUiPlugin,
};
use panel_kit_core::reducer::{ResizePolicy, Viewport, WorkspaceEvent};
use panel_kit_core::spec::{BackendKind, ResolvedWorkspace, WorkspaceSpec};
use panel_kit_core::theme::ThemeTokens;
use panel_kit_core::widgets::ContentSpec;
use panel_kit_core::{SpecPanelId, SurfaceProfile, Units, WinState};

#[path = "workspace_canary.rs"]
mod workspace_canary;
#[path = "dock_controls.rs"]
mod dock_controls;

use dock_controls::{paint_dock_controls, press_dock_controls, CanaryDefaults, CanaryStore};
use workspace_canary::{CanaryData, WorkspaceFacts, LIVE_BINDINGS};

/// Logical window size the cell-authored canary maps onto.
const WINDOW_PX: (f64, f64) = (1440.0, 900.0);

/// Scale factor of the offscreen capture target.
const CAPTURE_SCALE: f32 = 2.0;

/// Path named by `PANEL_KIT_BEVY_SCREENSHOT`, when capturing.
fn capture_path() -> Option<String> {
    std::env::var("PANEL_KIT_BEVY_SCREENSHOT").ok()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resolved = decode_workspace_spec()?
        .resolve(&workspace_canary::provider_manifest(BackendKind::Bevy))?
        .into_logical_px(WINDOW_PX.0, WINDOW_PX.1);
    let ResolvedWorkspace {
        catalog,
        initial,
        layout,
        surface,
        input,
        persistence,
        panels,
        theme,
        ..
    } = resolved;

    let mut bodies = CanaryBodies {
        spec_theme: theme.clone(),
        panels: HashMap::new(),
        painted: HashMap::new(),
        repaint_all: false,
        repaint_live: false,
        resize_policy: surface.resize_policy,
    };
    for panel in &panels {
        if let Some(entry) = catalog.get_by_stable_id(&panel.id) {
            let binding = panel.content.binding_id().unwrap_or_default().to_owned();
            bodies.panels.insert(entry.key, (binding, panel.content.clone()));
        }
    }

    let mut kit = BevyPanelKit::new(
        catalog,
        &initial.panels,
        initial.preferred_mode,
        initial.viewport,
    )?;
    kit.clamp = layout.clamp;
    kit.tile = layout.tile.resize;
    kit.command_step = input.steps;
    // Scale while the window settles; `fit_to_window` hands over to the
    // spec's policy afterwards.
    kit.resize_policy = ResizePolicy::ScaleFloating;
    kit.surface = SurfaceProfile::from_logical_width(
        WINDOW_PX.0,
        surface.compact_max,
        surface.tablet_max,
        kit.surface.caps,
    );

    // A capture renders the authored layout offscreen only: an OS window would
    // be resized to fit the display and rescale the workspace away from
    // `WINDOW_PX`, and the operator's saved layout is neither read nor written.
    let capturing = capture_path().is_some();
    let store = FileLayoutStore::new(
        std::env::temp_dir().join(format!("{}-bevy.json", persistence.key)),
    );
    if persistence.enabled && persistence.restore && !capturing {
        restore_layout(&mut kit, &store)?;
    }
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: (!capturing).then(|| Window {
                    title: "panel-kit · bevy workspace canary".into(),
                    resolution: (WINDOW_PX.0 as f32, WINDOW_PX.1 as f32).into(),
                    ..default()
                }),
                exit_condition: if capturing {
                    ExitCondition::DontExit
                } else {
                    ExitCondition::OnPrimaryClosed
                },
                close_when_requested: true,
            }),
            PanelUiPlugin::<SpecPanelId>::default(),
            ScrollPlugin,
            TimeSeriesPlugin,
        ))
        .insert_resource(BevyTheme::from_tokens(&theme))
        .insert_resource(kit)
        .insert_resource(CanaryDefaults {
            initial,
            save_policy: persistence.save_policy,
            skip_save: false,
        })
        .insert_resource(CanaryStore(store))
        .insert_resource(bodies)
        .init_resource::<CanaryData>()
        .insert_resource(TickTimer(Timer::new(CanaryData::TICK, TimerMode::Repeating)))
        .add_systems(Startup, (setup_camera, install_font))
        .add_systems(
            Update,
            (
                fit_to_window,
                tick_canary,
                demo_keys,
                save_on_change,
                paint_dock_controls,
                press_dock_controls.before(save_on_change),
                write_exports,
                capture_then_exit,
                dump_ui_nodes,
            ),
        )
        // PostUpdate runs after the renderer's Update pass, so body handles
        // are current for this frame.
        .add_systems(PostUpdate, paint_bodies)
        .run();
    Ok(())
}

/// Decode the Nix-authored spec from the path named at compile time, the
/// same loading path the TUI canary uses.
fn decode_workspace_spec() -> Result<WorkspaceSpec, Box<dyn std::error::Error>> {
    let Some(path) = option_env!("PANEL_KIT_WORKSPACE_SPEC") else {
        return Err(
            "PANEL_KIT_WORKSPACE_SPEC must point at the Nix-authored workspace spec JSON".into(),
        );
    };
    let json = std::fs::read_to_string(path)?;
    Ok(WorkspaceSpec::from_json_str(&json)?)
}

#[derive(Resource)]
struct TickTimer(Timer);

/// Render text in the spec theme's font family, as the web shell does.
fn install_font(mut fonts: ResMut<Assets<Font>>, bodies: Res<CanaryBodies>) {
    match install_theme_font(&mut fonts, &bodies.spec_theme) {
        Some(faces) => eprintln!(
            "theme font: {} regular {}, bold {:?}, glyph fallback {:?}",
            faces.regular.family,
            faces.regular.path.display(),
            faces.bold.map(|face| face.path),
            faces
                .fallback
                .iter()
                .map(|(face, glyphs)| (face.family.as_str(), glyphs.len()))
                .collect::<Vec<_>>(),
        ),
        None => eprintln!(
            "no installed font for {:?}; using the embedded font",
            bodies.spec_theme.typography.family
        ),
    }
}

/// Frames during which the OS may still resize the new window.
const SETTLE_FRAMES: u32 = 60;

/// Scale the cell-authored geometry to the window size the OS grants while
/// the window settles, which can differ from the requested resolution; then
/// apply the spec's resize policy to later resizes.
fn fit_to_window(
    windows: Query<&Window, With<PrimaryWindow>>,
    capture: Option<Res<CaptureTarget>>,
    mut kit: ResMut<BevyPanelKit<SpecPanelId>>,
    bodies: Res<CanaryBodies>,
    mut frames: Local<u32>,
) {
    if capture.is_some() || *frames > SETTLE_FRAMES {
        return;
    }
    *frames += 1;
    if *frames > SETTLE_FRAMES {
        kit.resize_policy = bodies.resize_policy;
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let (width, height) = (f64::from(window.width()), f64::from(window.height()));
    if (width, height) != (kit.snapshot.viewport.width, kit.snapshot.viewport.height) {
        kit.apply_workspace_event(WorkspaceEvent::ViewportChanged {
            size: Viewport {
                width,
                height,
                units: Units::CssPx,
            },
            policy: ResizePolicy::ScaleFloating,
        });
    }
}

/// Per-panel binding and content, the bodies painted so far, and pending
/// repaints.
#[derive(Resource)]
struct CanaryBodies {
    spec_theme: ThemeTokens,
    panels: HashMap<SpecPanelId, (String, ContentSpec)>,
    painted: HashMap<Entity, SpecPanelId>,
    repaint_all: bool,
    repaint_live: bool,
    resize_policy: ResizePolicy,
}

/// Offscreen image target the capture camera renders the workspace into.
#[derive(Resource)]
struct CaptureTarget(RenderTarget);

fn setup_camera(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    if capture_path().is_none() {
        commands.spawn(Camera2d);
        return;
    }
    let size = Extent3d {
        width: (WINDOW_PX.0 as f32 * CAPTURE_SCALE) as u32,
        height: (WINDOW_PX.1 as f32 * CAPTURE_SCALE) as u32,
        ..default()
    };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_SRC
        | TextureUsages::COPY_DST
        | TextureUsages::RENDER_ATTACHMENT;
    let handle = images.add(image);
    let target = RenderTarget::Image(ImageRenderTarget {
        handle,
        scale_factor: FloatOrd(CAPTURE_SCALE),
    });
    commands.spawn((
        Camera2d,
        Camera {
            target: target.clone(),
            ..default()
        },
        IsDefaultUiCamera,
    ));
    commands.insert_resource(CaptureTarget(target));
}

/// Sample live metrics. A capture keeps the initial samples, the state the
/// web canary paints.
fn tick_canary(
    time: Res<Time>,
    capture: Option<Res<CaptureTarget>>,
    mut timer: ResMut<TickTimer>,
    mut data: ResMut<CanaryData>,
    mut bodies: ResMut<CanaryBodies>,
) {
    if capture.is_none() && timer.0.tick(time.delta()).just_finished() {
        data.tick();
        bodies.repaint_live = true;
    }
}

fn demo_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut data: ResMut<CanaryData>,
    mut bodies: ResMut<CanaryBodies>,
    mut theme: ResMut<BevyTheme>,
    mut exit: EventWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyP) {
        data.paper = !data.paper;
        *theme = if data.paper {
            BevyTheme::from_tokens(&ThemeTokens::paper())
        } else {
            BevyTheme::from_tokens(&bodies.spec_theme)
        };
        bodies.repaint_all = true;
    }
    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
    }
}

/// Paint new bodies, and repaint live or theme-dependent bodies in place.
fn paint_bodies(
    handles: Option<Res<PanelBodyHandles<SpecPanelId>>>,
    kit: Res<BevyPanelKit<SpecPanelId>>,
    theme: Res<BevyTheme>,
    data: Res<CanaryData>,
    mut bodies: ResMut<CanaryBodies>,
    mut commands: Commands,
) {
    let Some(handles) = handles else {
        return;
    };
    let mut painted = Vec::new();
    for (key, body) in handles.iter() {
        let Some((binding, content)) = bodies.panels.get(&key) else {
            continue;
        };
        let fresh = bodies.painted.get(&body) != Some(&key);
        let live = bodies.repaint_live && LIVE_BINDINGS.contains(&binding.as_str());
        // The Workspace card reports its panel's window state.
        let stateful = kit.is_changed() && binding == "canary.workspace";
        if !(fresh || live || stateful || bodies.repaint_all) {
            continue;
        }
        let facts = WorkspaceFacts {
            title: kit.catalog.get(key).map_or("", |meta| meta.title.as_ref()),
            maximized: kit
                .snapshot
                .panels
                .iter()
                .any(|panel| panel.kind == key && panel.state == WinState::Maximized),
        };
        let mut body_commands = commands.entity(body);
        if !fresh {
            body_commands.despawn_related::<Children>();
        }
        body_commands.with_children(|parent| {
            workspace_canary::paint(parent, &theme, &data, facts, binding, content);
        });
        painted.push((body, key));
    }
    bodies.painted.extend(painted);
    bodies.repaint_all = false;
    bodies.repaint_live = false;
}

/// Persist the workspace after any frame that changed it, except the frame
/// a reset cleared the record.
fn save_on_change(
    kit: Res<BevyPanelKit<SpecPanelId>>,
    store: Res<CanaryStore>,
    capture: Option<Res<CaptureTarget>>,
    mut defaults: ResMut<CanaryDefaults>,
) {
    if std::mem::take(&mut defaults.skip_save) || capture.is_some() {
        return;
    }
    if kit.is_changed() {
        let _ = save_layout(&kit, &store.0);
    }
}

/// Hand table exports to the platform: COPY's TSV to the macOS pasteboard
/// when `pbcopy` runs, and CSV (or TSV without a pasteboard) to a file in the
/// temp dir, whose path is printed.
fn write_exports(mut exports: EventReader<DataTableExport>) {
    for export in exports.read() {
        if export.format == ExportFormat::Tsv && copy_to_pasteboard(&export.text) {
            eprintln!("table rows copied to the pasteboard");
            continue;
        }
        let extension = match export.format {
            ExportFormat::Tsv => "tsv",
            ExportFormat::Csv => "csv",
        };
        let path = std::env::temp_dir().join(format!("panel-kit-canary-table.{extension}"));
        match std::fs::write(&path, &export.text) {
            Ok(()) => eprintln!("table rows written to {}", path.display()),
            Err(error) => eprintln!("writing {} failed: {error}", path.display()),
        }
    }
}

fn copy_to_pasteboard(text: &str) -> bool {
    use std::io::Write;
    let Ok(mut child) = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
    else {
        return false;
    };
    let written = child
        .stdin
        .take()
        .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
    child.wait().is_ok_and(|status| status.success()) && written
}

/// Frames to keep running after a capture lands, so `save_to_disk` finishes
/// writing before exit.
const EXIT_DELAY_FRAMES: u32 = 30;

/// Set once the screenshot is captured; counts frames down to exit.
#[derive(Resource)]
struct ExitCountdown(u32);

/// Capture the offscreen workspace to `PANEL_KIT_BEVY_SCREENSHOT` and exit,
/// once the workspace has rendered for a moment.
fn capture_then_exit(
    mut frames: Local<u64>,
    mut requested: Local<bool>,
    target: Option<Res<CaptureTarget>>,
    countdown: Option<ResMut<ExitCountdown>>,
    mut exit: EventWriter<AppExit>,
    mut commands: Commands,
) {
    if let Some(mut countdown) = countdown {
        countdown.0 = countdown.0.saturating_sub(1);
        if countdown.0 == 0 {
            exit.write(AppExit::Success);
        }
        return;
    }
    if *requested {
        return;
    }
    *frames += 1;
    if *frames < 90 {
        return;
    }
    *requested = true;
    let (Some(path), Some(target)) = (capture_path(), target) else {
        return;
    };
    commands
        .spawn(Screenshot(target.0.clone()))
        .observe(save_to_disk(path))
        .observe(|_captured: Trigger<ScreenshotCaptured>, mut commands: Commands| {
            commands.insert_resource(ExitCountdown(EXIT_DELAY_FRAMES));
        });
}

/// Text and paint of one UI node, in logical pixels, for the parity report.
type NodeDumpItem<'a> = (
    Entity,
    &'a ComputedNode,
    &'a GlobalTransform,
    &'a InheritedVisibility,
    Option<&'a Text>,
    Option<&'a TextFont>,
    Option<&'a TextColor>,
    Option<&'a BackgroundColor>,
    Option<&'a BorderColor>,
    Option<&'a ChildOf>,
    Option<&'a bevy::text::TextLayoutInfo>,
);

/// Write every visible UI node's rect, text, font size and paint to
/// `<PANEL_KIT_BEVY_SCREENSHOT>.nodes.jsonl` on the capture frame, so a
/// parity check can compare geometry with the web DOM.
fn dump_ui_nodes(
    mut frames: Local<u64>,
    capture: Option<Res<CaptureTarget>>,
    nodes: Query<NodeDumpItem>,
    spans: Query<(&TextSpan, Option<&TextColor>)>,
    children: Query<&Children>,
) {
    let (Some(path), Some(_)) = (capture_path(), capture) else {
        return;
    };
    *frames += 1;
    if *frames != 90 {
        return;
    }
    let srgb = |color: Color| {
        let c = color.to_srgba();
        format!(
            "[{},{},{},{:.3}]",
            (c.red * 255.0).round(),
            (c.green * 255.0).round(),
            (c.blue * 255.0).round(),
            c.alpha
        )
    };
    let mut out = String::new();
    for (entity, node, transform, visible, text, font, color, background, border, parent, layout) in &nodes {
        if !visible.get() {
            continue;
        }
        let scale = node.inverse_scale_factor();
        let size = node.size() * scale;
        let center = transform.translation().truncate() * scale;
        let mut value = text.map(|t| t.0.clone()).unwrap_or_default();
        for child in children.iter_descendants(entity) {
            if let Ok((span, _)) = spans.get(child) {
                value.push_str(&span.0);
            }
        }
        let border_w = node.border().top * scale;
        out.push_str(&format!(
            "{{\"id\":{},\"parent\":{},\"x\":{:.2},\"y\":{:.2},\"w\":{:.2},\"h\":{:.2},\"tw\":{},\"text\":{:?},\"font\":{},\"bold\":{},\"color\":{},\"bg\":{},\"border\":{},\"border_w\":{:.2}}}\n",
            entity.index(),
            parent.map_or("null".into(), |p| p.0.index().to_string()),
            center.x - size.x / 2.0,
            center.y - size.y / 2.0,
            size.x,
            size.y,
            layout.map_or("null".into(), |l| format!("{:.2}", l.size.x)),
            value,
            font.map_or("null".into(), |f| format!("{:.3}", f.font_size)),
            font.is_some_and(|f| f.font == BOLD_FONT),
            color.map_or("null".into(), |c| srgb(c.0)),
            background.map_or("null".into(), |c| srgb(c.0)),
            border.map_or("null".into(), |c| srgb(c.0)),
            border_w,
        ));
    }
    let _ = std::fs::write(format!("{path}.nodes.jsonl"), out);
}
