//! Workspace rendering: core frame projection as `bevy_ui` entities.
//!
//! [`render_workspace`] is the single system the binding contributes. It
//! projects the current [`BevyPanelKit`] snapshot through
//! [`project_into`](panel_kit_core::frame::project_into) and reconciles a
//! stable set of UI entities against the result:
//!
//! - one root node per workspace containing the workspace band and the
//!   [`MinimizedDock`] dock band,
//! - one [`PanelFrame`] node per panel in the snapshot, reused across frames,
//!   hidden while minimized, and despawned only when the panel leaves the
//!   snapshot — so application content spawned into a [`PanelBody`] slot
//!   survives geometry and state changes,
//! - one [`PanelChrome`] header per panel (traffic lights + title text) and
//!   one [`PanelBody`] body slot per panel,
//! - one [`DockChip`] per minimized panel, and one [`DockExtras`] trailing
//!   slot the host fills with its own dock controls.
//!
//! The system runs only when the [`BevyPanelKit`] or [`BevyTheme`] resources
//! change; unchanged inputs cost nothing. Geometry is absolute positioning
//! taken from the core projection (floating, tiled, and maximized placements
//! all arrive as projected regions), and the chrome resolves every role —
//! backdrop, surface, dividers, borders, titles, and traffic lights — from the
//! [`BevyTheme`] resource, so inserting a different theme repaints it. Input
//! handling and persistence are out of scope.

use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

use bevy::asset::{weak_handle, RenderAssetUsages};
use bevy::ecs::component::{Component, Mutable, StorageType};
use bevy::ecs::world::FromWorld;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::UiTargetCamera;

use panel_kit_core::frame::{
    project_into, ChromeProjectionInput, PanelProjection, ProjectionBuffer, ProjectionInput,
    TileLayoutMetrics,
};
use panel_kit_core::tile_drag::{lift_opacity, lift_scale, TileRole};
use panel_kit_core::theme::{PanelPaint, GRIP_PX};
use panel_kit_core::{ChromeMetrics, PanelKey, Region, WinState};

use crate::theme::BevyTheme;
use crate::widgets::{styled_text, TextStyle};
use crate::{BevyPanelKey, BevyPanelKit, DockExtras, MinimizedDock, PanelBody, PanelChrome};

/// Marks a panel's root UI node and carries the panel key used to reconcile
/// the node against the workspace snapshot.
///
/// The renderer spawns one [`PanelFrame`] entity per panel and reuses it
/// across frames; it is despawned only when the panel leaves the snapshot.
/// Panel keys are unique workspace identities, so at most one node exists per
/// key.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PanelFrame<K: BevyPanelKey> {
    /// The panel this node renders.
    pub key: K,
}

// Implemented by hand rather than via the derive: the derive would require
// the app panel type itself to implement `Component`, but a panel key is a
// workspace identity, not a component.
impl<K: BevyPanelKey> Component for PanelFrame<K> {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    type Mutability = Mutable;
}

/// Marks one minimized-dock chip and carries the panel key it restores.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DockChip<K: BevyPanelKey> {
    /// The minimized panel this chip represents.
    pub key: K,
}

impl<K: BevyPanelKey> Component for DockChip<K> {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    type Mutability = Mutable;
}

/// Workspace backdrop paint.
///
/// Defaults to the theme background. Set [`PanelBackdrop(None)`] to keep the
/// workspace transparent so a 3D scene behind the panels stays visible.
#[derive(Resource, Debug, Clone, Copy)]
pub struct PanelBackdrop(pub Option<Color>);

impl Default for PanelBackdrop {
    fn default() -> Self {
        Self(Some(BevyTheme::default().background))
    }
}

/// The camera the workspace UI renders through.
///
/// Without it the root node follows Bevy's default UI camera, which is right
/// for a single-window app. A host with several windows names the camera of
/// the window the workspace belongs to; the renderer attaches
/// [`UiTargetCamera`] to the workspace root (descendants inherit it) and the
/// input systems accept events from that camera's window only. Naming an
/// entity that is not a window camera yet (`Entity::PLACEHOLDER` until the
/// camera spawns) draws the workspace nowhere and accepts no input.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelUiTargetCamera(pub Entity);

/// Stable access to each panel's [`PanelBody`] entity.
///
/// The renderer rebuilds this resource whenever the workspace changes, so
/// applications can spawn UI into a panel body without walking the scene.
/// Handles are removed when a panel leaves the snapshot.
pub struct PanelBodyHandles<K: BevyPanelKey> {
    handles: HashMap<K, Entity>,
}

impl<K: BevyPanelKey> Resource for PanelBodyHandles<K> {}

impl<K: BevyPanelKey> PanelBodyHandles<K> {
    /// Entity for one panel's body slot, if present.
    pub fn get(&self, key: K) -> Option<Entity> {
        self.handles.get(&key).copied()
    }

    /// Iterate all known `(panel, body)` handles.
    pub fn iter(&self) -> impl Iterator<Item = (K, Entity)> + '_ {
        self.handles.iter().map(|(key, entity)| (*key, *entity))
    }

    /// Number of known body handles.
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// Whether no body handles are currently tracked.
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }
}

/// Host systems that paint into panel body slots run in this [`PostUpdate`]
/// set: after [`render_workspace`] refreshed [`PanelBodyHandles`] and before
/// `bevy_ui` measures and lays out, so a repainted body is laid out in the
/// frame it is spawned. A painter outside it shows one blank frame per repaint.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BodyPaint;

/// Bevy plugin that renders the [`BevyPanelKit`] workspace into `bevy_ui`.
///
/// Registers [`render_workspace`] in [`Update`]. The host inserts the
/// [`BevyPanelKit`] resource (and may mutate its public fields at will);
/// the renderer reconciles the UI whenever the resource changes.
pub struct PanelUiPlugin<K: BevyPanelKey> {
    _marker: PhantomData<fn() -> K>,
}

impl<K: BevyPanelKey> Default for PanelUiPlugin<K> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<K: BevyPanelKey> Plugin for PanelUiPlugin<K> {
    fn build(&self, app: &mut App) {
        // Hosts may insert their own theme before or after adding the plugin;
        // `init_resource` never overwrites an existing one, and the renderer
        // re-reads the resource on every reconcile.
        app.init_resource::<BevyTheme>();
        app.add_systems(
            Startup,
            (insert_grip_image, crate::widgets::spinner::insert_ring_images),
        );
        app.add_systems(Update, render_workspace::<K>);
        app.configure_sets(PostUpdate, BodyPaint.before(bevy::ui::UiSystem::Prepare));
        app.add_plugins(crate::input::PanelInputPlugin::<K>::default());
        // Table content panels paint through the data table painter.
        if !app.is_plugin_added::<crate::widgets::data_table::DataTablePlugin>() {
            app.add_plugins(crate::widgets::data_table::DataTablePlugin);
        }
    }
}

/// Crate-internal marker for the top-level workspace container.
#[derive(Component)]
struct WorkspaceRoot;
/// Crate-internal marker for the panel band.
#[derive(Component)]
struct WorkspaceNode;
/// Crate-internal marker for the dock band (alongside the public
/// [`MinimizedDock`]).
#[derive(Component)]
struct DockNode;
/// Crate-internal marker for a panel's title, carrying the painted title.
#[derive(Component)]
struct PanelTitle(String);
/// Which traffic light a node paints, so visibility can follow the projected
/// hit regions.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum LightRole {
    Mode,
    Minimize,
    Maximize,
}
/// Crate-internal marker for a panel's resize grip.
#[derive(Component)]
struct ResizeGrip;
/// Crate-internal marker for the dock's empty-state text.
#[derive(Component)]
struct DockEmpty;

/// The resize-grip triangle, tinted per theme.
const GRIP_IMAGE: Handle<Image> = weak_handle!("9d3f7a12-4c6e-4b8a-b1f0-5e2d7c9a3b64");

/// Texels per logical pixel of the grip image.
const GRIP_TEXELS: f64 = 2.0;

/// Register [`GRIP_IMAGE`]: the lower-right half of a [`GRIP_PX`] square,
/// the hard 50% stop of `.resize::after`'s 135° gradient, in white so the
/// node colour tints it.
fn insert_grip_image(images: Option<ResMut<Assets<Image>>>) {
    let Some(mut images) = images else {
        return;
    };
    let side = (GRIP_PX * GRIP_TEXELS) as u32;
    let mut data = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let alpha = if x + y >= side { 255 } else { 0 };
            data.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    images.insert(&GRIP_IMAGE, image);
}

/// The header band inset into the top of the frame (`.panel-head`): it
/// starts one border above the padding box, holds the lights and title, and
/// its left padding is pulled back so the first light sits on the inset.
fn header_node(paint: &PanelPaint) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(paint.head_inset as f32),
        right: Val::Px(paint.head_inset as f32),
        top: Val::Px(-paint.border as f32),
        height: Val::Px(paint.head_h as f32),
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(paint.light_gap as f32),
        padding: UiRect::right(Val::Px(paint.head_pad as f32)),
        ..default()
    }
}

/// The title slot after the lights (`.panel-title`): the header gap to the
/// lights is `title_gap`, the rest of the row is the title's.
fn title_node(paint: &PanelPaint) -> Node {
    Node {
        flex_grow: 1.0,
        min_width: Val::ZERO,
        margin: UiRect::left(Val::Px((paint.title_gap - paint.light_gap) as f32)),
        overflow: Overflow::clip(),
        ..default()
    }
}

/// The resize grip anchored in the padding box's lower-right corner.
fn grip_node(paint: &PanelPaint) -> Node {
    Node {
        position_type: PositionType::Absolute,
        right: Val::ZERO,
        bottom: Val::ZERO,
        width: Val::Px(paint.grip as f32),
        height: Val::Px(paint.grip as f32),
        ..default()
    }
}

/// Paint the title glyphs into a [`PanelTitle`] slot.
fn spawn_title(slot: &mut EntityCommands, title: &str, theme: &BevyTheme) {
    slot.with_children(|parent| {
        styled_text(parent, title, TextStyle::label(theme));
    });
}

/// The chrome border colour for a projected panel, the terminal painter's
/// precedence: tile drag (lifted, landing, or swap target), then focus, then
/// maximized, then the plain border.
fn panel_border<K: PanelKey>(theme: &BevyTheme, proj: &PanelProjection<K>) -> Color {
    if matches!(
        proj.tile_role,
        TileRole::Lifted | TileRole::Landing | TileRole::Target
    ) {
        return theme.highlight;
    }
    if proj.focused {
        return theme.focus_ring;
    }
    if proj.state == WinState::Maximized {
        return theme.text;
    }
    theme.border
}

/// Drop shadow of a tile at `lift` elevation: the design's drag lift
/// (`0 6px 24px` of 46.67% shadow ink) scaled by elevation.
fn lift_shadow(lift: f64) -> Vec<ShadowStyle> {
    if lift <= 0.0 {
        return Vec::new();
    }
    let lift = lift as f32;
    vec![ShadowStyle {
        color: Color::srgba(0.0, 0.0, 0.0, 0.4667 * lift),
        x_offset: Val::ZERO,
        y_offset: Val::Px(6.0 * lift),
        spread_radius: Val::ZERO,
        blur_radius: Val::Px(24.0 * lift),
    }]
}

/// Reusable projection scratch, initialized once per system instance.
///
/// Public because it appears in [`render_workspace`]'s `Local` parameter;
/// callers never construct it directly.
pub struct ProjectionScratch<K: PanelKey>(ProjectionBuffer<K>);

impl<K: PanelKey> FromWorld for ProjectionScratch<K> {
    fn from_world(_world: &mut World) -> Self {
        Self(ProjectionBuffer::with_panel_capacity(0))
    }
}

fn val(v: f64) -> Val {
    Val::Px(v as f32)
}

fn size(v: f64) -> Val {
    Val::Px(v.max(0.0) as f32)
}

/// Position a node absolutely at an (x, y, w, h) rectangle.
fn place(node: &mut Node, x: f64, y: f64, w: f64, h: f64) {
    node.position_type = PositionType::Absolute;
    node.left = val(x);
    node.top = val(y);
    node.width = size(w);
    node.height = size(h);
}

fn title_of<K: BevyPanelKey>(kit: &BevyPanelKit<K>, key: K) -> String {
    kit.catalog
        .get(key)
        .map(|meta| meta.title.to_string())
        .unwrap_or_default()
}

/// Render (and reconcile) the panel workspace UI.
///
/// Runs whenever the [`BevyPanelKit`] resource changes. Panels are matched by
/// key: entities for known keys are updated in place, keys missing from the
/// snapshot are despawned (with their subtree), and new keys are spawned.
//
// The crate-private marker components and the scratch buffer appear in this
// signature only as Bevy query filters and `Local` state; callers never name
// them, so they stay out of the public API.
#[allow(private_interfaces)]
// The Bevy query tuples are the system's type signature; factoring them into
// aliases would hide the component set from every reader.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn render_workspace<K: BevyPanelKey>(
    kit: Res<BevyPanelKit<K>>,
    theme: Res<BevyTheme>,
    backdrop: Option<Res<PanelBackdrop>>,
    target: Option<Res<PanelUiTargetCamera>>,
    mut scratch: Local<ProjectionScratch<K>>,
    mut commands: Commands,
    mut shell: Query<
        (
            Entity,
            &mut Node,
            Option<&WorkspaceRoot>,
            Option<&WorkspaceNode>,
            Option<&DockNode>,
            Option<&mut BackgroundColor>,
            Option<&mut BorderColor>,
        ),
        (
            Without<PanelFrame<K>>,
            Without<PanelBody>,
            Without<DockChip<K>>,
        ),
    >,
    mut frames: Query<
        (
            Entity,
            &PanelFrame<K>,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut ZIndex,
            &mut Transform,
            &mut BoxShadow,
            &mut Visibility,
        ),
        (
            Without<WorkspaceRoot>,
            Without<WorkspaceNode>,
            Without<DockNode>,
            Without<PanelBody>,
            Without<DockChip<K>>,
            Without<LightRole>,
        ),
    >,
    mut bodies: Query<
        (Entity, &ChildOf, &mut Node),
        (
            With<PanelBody>,
            Without<WorkspaceRoot>,
            Without<WorkspaceNode>,
            Without<DockNode>,
            Without<PanelFrame<K>>,
            Without<DockChip<K>>,
        ),
    >,
    chrome_parents: Query<&ChildOf>,
    mut titles: Query<
        (Entity, &ChildOf, &mut PanelTitle),
        (Without<WorkspaceRoot>, Without<DockChip<K>>),
    >,
    mut lights: Query<
        (&ChildOf, &mut Visibility, &LightRole),
        (
            With<LightRole>,
            Without<WorkspaceRoot>,
            Without<PanelFrame<K>>,
            Without<DockChip<K>>,
        ),
    >,
    mut grips: Query<
        (&ChildOf, &mut Visibility, &mut ImageNode),
        (With<ResizeGrip>, Without<PanelFrame<K>>, Without<LightRole>),
    >,
    mut dock_empty: Query<
        &mut Visibility,
        (
            With<DockEmpty>,
            Without<PanelFrame<K>>,
            Without<LightRole>,
            Without<ResizeGrip>,
        ),
    >,
    mut chips: Query<
        (Entity, &DockChip<K>, &mut Node, &mut Text),
        (
            Without<WorkspaceRoot>,
            Without<WorkspaceNode>,
            Without<DockNode>,
            Without<PanelFrame<K>>,
            Without<PanelBody>,
            Without<PanelTitle>,
        ),
    >,
) {
    let target_changed = target.as_ref().is_some_and(|target| target.is_changed());
    if !kit.is_changed() && !theme.is_changed() && !target_changed {
        return;
    }

    let chrome_in = ChromeProjectionInput::full(ChromeMetrics::WEB);
    let tile = TileLayoutMetrics::from_tile_metrics(kit.tile, kit.surface);
    let input = ProjectionInput {
        snapshot: &kit.snapshot,
        surface: kit.surface,
        chrome: &chrome_in,
        clamp: &kit.clamp,
        tile: &tile,
    };
    let frame = project_into(input, &mut scratch.0);

    let ws = frame.chrome.workspace;
    let dock_region = frame.chrome.dock;
    let header_h = chrome_in.panel_header_h;

    let mut projected: HashMap<K, &PanelProjection<K>> = HashMap::with_capacity(frame.panels.len());
    for panel in frame.panels {
        projected.insert(panel.key, panel);
    }
    let snapshot_keys: HashSet<K> = kit.snapshot.panels.iter().map(|p| p.kind).collect();

    // Locate the shell; refresh its geometry from the projected chrome.
    let mut root_entity = None;
    let mut workspace_entity = None;
    let mut dock_entity = None;
    for (entity, _node, root, workspace, dock, _paint, _edge) in shell.iter() {
        if root.is_some() {
            root_entity = Some(entity);
        }
        if workspace.is_some() {
            workspace_entity = Some(entity);
        }
        if dock.is_some() {
            dock_entity = Some(entity);
        }
    }
    let (_root_entity, workspace_entity, dock_entity) = match root_entity {
        Some(root) => {
            if let Some(entity) = workspace_entity {
                if let Ok((_, mut node, ..)) = shell.get_mut(entity) {
                    place(&mut node, ws.x, ws.y, ws.w, ws.h);
                    node.overflow = Overflow::clip();
                }
            }
            if let Some(entity) = dock_entity {
                if let Ok((
                    _,
                    mut node,
                    _,
                    _,
                    _,
                    Some(mut background),
                    Some(mut border),
                )) = shell.get_mut(entity)
                {
                    place(
                        &mut node,
                        dock_region.x,
                        dock_region.y,
                        dock_region.w,
                        dock_region.h,
                    );
                    background.0 = theme.surface;
                    border.0 = theme.divider;
                }
            }
            if let Some(color) = backdrop.as_ref().and_then(|backdrop| backdrop.0) {
                if let Ok((_, _, _, _, _, Some(mut background), _)) = shell.get_mut(root) {
                    background.0 = color;
                }
            }
            if target_changed {
                if let Some(target) = target.as_ref() {
                    commands.entity(root).insert(UiTargetCamera(target.0));
                }
            }
            (root, workspace_entity, dock_entity)
        }
        None => {
            let root = commands
                .spawn((
                    WorkspaceRoot,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                ))
                .id();
            if let Some(target) = target.as_ref() {
                commands.entity(root).insert(UiTargetCamera(target.0));
            }
            let workspace = commands
                .spawn((
                    WorkspaceNode,
                    Node {
                        position_type: PositionType::Absolute,
                        left: val(ws.x),
                        top: val(ws.y),
                        width: size(ws.w),
                        height: size(ws.h),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    ChildOf(root),
                ))
                .id();
            let dock = commands
                .spawn((
                    DockNode,
                    MinimizedDock,
                    Node {
                        position_type: PositionType::Absolute,
                        left: val(dock_region.x),
                        top: val(dock_region.y),
                        width: size(dock_region.w),
                        height: size(dock_region.h),
                        border: UiRect {
                            top: Val::Px(1.0),
                            ..default()
                        },
                        display: Display::Flex,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(theme.metrics.rem(0.5)),
                        padding: UiRect::horizontal(Val::Px(theme.metrics.rem(0.75))),
                        ..default()
                    },
                    ChildOf(root),
                    BorderColor(theme.divider),
                    BackgroundColor(theme.surface),
                ))
                .with_children(|dock| {
                    let muted = TextStyle::rem(&theme, 0.7).color(theme.text_muted);
                    styled_text(dock, "dock:", muted);
                    let empty = styled_text(dock, "— nothing minimized —", muted);
                    dock.commands().entity(empty).insert(DockEmpty);
                    // `.dock-extras { margin-left: auto }`. Taffy drops the
                    // dock's `gap` beside an auto margin; growing is equivalent
                    // because nothing else in the dock grows.
                    dock.spawn((
                        DockExtras,
                        Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::FlexEnd,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                    ));
                })
                .id();
            let backdrop_color = match backdrop.as_ref() {
                Some(backdrop) => backdrop.0,
                None => Some(theme.background),
            };
            if let Some(color) = backdrop_color {
                commands.entity(root).insert(BackgroundColor(color));
            }
            (root, Some(workspace), Some(dock))
        }
    };
    let workspace_entity = workspace_entity.expect("workspace band exists");

    // Despawn panels that left the snapshot.
    for (entity, frame, ..) in frames.iter() {
        if !snapshot_keys.contains(&frame.key) {
            commands.entity(entity).despawn();
        }
    }

    // Spawn panels the UI does not know yet. Geometry comes from the
    // projection when the panel is visible and from the stored rect while it
    // is minimized (hidden anyway); the update passes keep it in sync after.
    let mut frame_map: HashMap<K, Entity> = HashMap::new();
    for (entity, frame, ..) in frames.iter() {
        frame_map.entry(frame.key).or_insert(entity);
    }
    let mut body_handles: HashMap<K, Entity> = HashMap::with_capacity(projected.len());
    for panel in &kit.snapshot.panels {
        if frame_map.contains_key(&panel.kind) {
            continue;
        }

        let (region, z, visible, body_rect, border) = match projected.get(&panel.kind) {
            Some(proj) => (
                Region::new(
                    proj.region.x - ws.x,
                    proj.region.y - ws.y,
                    proj.region.w,
                    proj.region.h,
                ),
                proj.z,
                true,
                Region::new(
                    proj.chrome.body.x - proj.chrome.outer.x,
                    proj.chrome.body.y - proj.chrome.outer.y,
                    proj.chrome.body.w,
                    proj.chrome.body.h,
                ),
                panel_border(&theme, proj),
            ),
            None => (
                Region::new(panel.x, panel.y, panel.w, panel.h),
                panel.z,
                false,
                Region::new(0.0, header_h, panel.w, (panel.h - header_h).max(0.0)),
                theme.border,
            ),
        };

        let root = commands
            .spawn((
                PanelFrame { key: panel.kind },
                Node {
                    position_type: PositionType::Absolute,
                    left: val(region.x),
                    top: val(region.y),
                    width: size(region.w),
                    height: size(region.h),
                    border: UiRect::all(Val::Px(theme.panel.border as f32)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                ChildOf(workspace_entity),
                BackgroundColor(theme.surface),
                BorderColor(border),
                ZIndex(z),
                BoxShadow(Vec::new()),
                if visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            ))
            .id();

        let paint = theme.panel;
        let chrome = commands
            .spawn((PanelChrome, header_node(&paint), ChildOf(root)))
            .id();
        for (role, color) in [
            (LightRole::Mode, theme.primary),
            (LightRole::Minimize, theme.tertiary),
            (LightRole::Maximize, theme.secondary),
        ] {
            commands.spawn((
                role,
                Node {
                    width: Val::Px(paint.light as f32),
                    height: Val::Px(paint.light as f32),
                    flex_shrink: 0.0,
                    ..default()
                },
                ChildOf(chrome),
                BackgroundColor(color),
            ));
        }
        let title = title_of(&kit, panel.kind);
        let mut slot = commands.spawn((PanelTitle(title.clone()), title_node(&paint), ChildOf(chrome)));
        spawn_title(&mut slot, &title, &theme);
        commands.spawn((
            ResizeGrip,
            grip_node(&paint),
            ImageNode::new(GRIP_IMAGE).with_color(theme.border),
            ZIndex(1),
            ChildOf(root),
        ));
        // `.panel-body`: the projected body below the header band, inside the
        // frame border, padded so content clears the band and the edges.
        let body_entity = commands
            .spawn((
                PanelBody,
                Node {
                    position_type: PositionType::Absolute,
                    left: val(body_rect.x),
                    top: val(body_rect.y),
                    width: size(body_rect.w - 2.0 * paint.border),
                    height: size(body_rect.h - 2.0 * paint.border),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect {
                        left: Val::Px(paint.body_pad_x as f32),
                        right: Val::Px(paint.body_pad_x as f32),
                        top: Val::Px((paint.body_pad_top - paint.head_h) as f32),
                        bottom: Val::Px(paint.body_pad_bottom as f32),
                    },
                    overflow: Overflow::clip(),
                    overflow_clip_margin: OverflowClipMargin::padding_box(),
                    ..default()
                },
                ChildOf(root),
            ))
            .id();
        body_handles.insert(panel.kind, body_entity);

        frame_map.insert(panel.kind, root);
    }

    // Update visible panels; hide minimized ones. A tile drag's elevation
    // drives scale, surface opacity, and the drop shadow.
    for (
        _entity,
        frame,
        mut node,
        mut background,
        mut border,
        mut z,
        mut transform,
        mut shadow,
        mut vis,
    ) in frames.iter_mut()
    {
        match projected.get(&frame.key) {
            Some(proj) => {
                place(
                    &mut node,
                    proj.region.x - ws.x,
                    proj.region.y - ws.y,
                    proj.region.w,
                    proj.region.h,
                );
                node.border = UiRect::all(Val::Px(theme.panel.border as f32));
                node.overflow = Overflow::clip();
                z.0 = proj.z;
                let opacity = lift_opacity(proj.lift) as f32;
                background.0 = theme.surface.with_alpha(opacity);
                border.0 = panel_border(&theme, proj).with_alpha(opacity);
                let scale = lift_scale(proj.lift) as f32;
                transform.scale = Vec3::new(scale, scale, 1.0);
                shadow.0 = lift_shadow(proj.lift);
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }

    // Body slots follow their panel's projected body region, and their
    // stable handles are published for application content.
    for (body_entity, parent, mut node) in bodies.iter_mut() {
        let Ok((_, frame, ..)) = frames.get(parent.0) else {
            continue;
        };
        if let Some(proj) = projected.get(&frame.key) {
            body_handles.insert(frame.key, body_entity);
            let outer = proj.chrome.outer;
            let border = theme.panel.border;
            place(
                &mut node,
                proj.chrome.body.x - outer.x,
                proj.chrome.body.y - outer.y,
                proj.chrome.body.w - 2.0 * border,
                proj.chrome.body.h - 2.0 * border,
            );
        }
    }
    commands.insert_resource(PanelBodyHandles {
        handles: body_handles,
    });

    // Traffic lights and titles live inside the chrome header; resolve the
    // panel through chrome -> root.
    for (chrome, mut vis, role) in lights.iter_mut() {
        let Ok(chrome_parent) = chrome_parents.get(chrome.0) else {
            continue;
        };
        let Ok((_, frame, ..)) = frames.get(chrome_parent.0) else {
            continue;
        };
        if let Some(proj) = projected.get(&frame.key) {
            let hit = match role {
                LightRole::Mode => proj.chrome.mode_hit,
                LightRole::Minimize => proj.chrome.minimize_hit,
                LightRole::Maximize => proj.chrome.maximize_hit,
            };
            *vis = if hit.is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    for (title_entity, chrome, mut painted) in titles.iter_mut() {
        let Ok(chrome_parent) = chrome_parents.get(chrome.0) else {
            continue;
        };
        let Ok((_, frame, ..)) = frames.get(chrome_parent.0) else {
            continue;
        };
        let title = title_of(&kit, frame.key);
        if painted.0 != title || theme.is_changed() {
            let mut slot = commands.entity(title_entity);
            slot.despawn_related::<Children>();
            spawn_title(&mut slot, &title, &theme);
            painted.0 = title;
        }
    }
    for (root, mut vis, mut image) in grips.iter_mut() {
        let Ok((_, frame, ..)) = frames.get(root.0) else {
            continue;
        };
        image.color = theme.border;
        *vis = match projected.get(&frame.key) {
            Some(proj) if proj.chrome.resize_hit.is_some() => Visibility::Inherited,
            _ => Visibility::Hidden,
        };
    }
    for mut vis in dock_empty.iter_mut() {
        *vis = if frame.dock.is_empty() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    // Dock chips: one per minimized panel, despawned on restore.
    if let Some(dock_entity) = dock_entity {
        let mut docked: HashSet<K> = HashSet::new();
        let mut chip_map: HashMap<K, Entity> = HashMap::new();
        for (entity, chip, ..) in chips.iter() {
            chip_map.insert(chip.key, entity);
        }
        for entry in frame.dock {
            docked.insert(entry.key);
            match chip_map.get(&entry.key) {
                Some(&entity) => {
                    if let Ok((_, _chip, mut node, mut text)) = chips.get_mut(entity) {
                        place(
                            &mut node,
                            entry.region.x - dock_region.x,
                            entry.region.y - dock_region.y,
                            entry.region.w,
                            entry.region.h,
                        );
                        *text = Text::new(title_of(&kit, entry.key));
                    }
                }
                None => {
                    commands.spawn((
                        DockChip { key: entry.key },
                        Node {
                            position_type: PositionType::Absolute,
                            left: val(entry.region.x - dock_region.x),
                            top: val(entry.region.y - dock_region.y),
                            width: size(entry.region.w),
                            height: size(entry.region.h),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        ChildOf(dock_entity),
                        BackgroundColor(theme.background),
                        BorderColor(theme.border),
                        Text::new(title_of(&kit, entry.key)),
                        TextStyle::rem(&theme, 0.72).font(),
                        TextColor(theme.text),
                    ));
                }
            }
        }
        for (entity, chip, ..) in chips.iter() {
            if !docked.contains(&chip.key) {
                commands.entity(entity).despawn();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BevyPanelKit, MinimizedDock, PanelBody, PanelBodyHandles, PanelChrome};
    use bevy::ecs::schedule::Schedule;
    use panel_kit_core::reducer::Viewport;
    use panel_kit_core::theme::ThemeTokens;
    use panel_kit_core::{LayoutBuilder, Mode, PanelCatalog, PanelKind, PanelWin, Units, WinState};

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    enum TestPanel {
        Scene,
        Inspector,
        Console,
    }

    impl PanelKind for TestPanel {
        fn title(self) -> &'static str {
            match self {
                Self::Scene => "Scene",
                Self::Inspector => "Inspector",
                Self::Console => "Console",
            }
        }
    }

    fn two_floating() -> (BevyPanelKit<TestPanel>, Vec<PanelWin<TestPanel>>) {
        let mut layout = LayoutBuilder::new();
        let panels = vec![
            layout.at(TestPanel::Scene, 40.0, 40.0, 400.0, 300.0),
            layout.at(TestPanel::Inspector, 480.0, 40.0, 300.0, 300.0),
        ];
        let kit = BevyPanelKit::new(
            PanelCatalog::from_panel_kind_layout(&panels).expect("test panels serialize"),
            &panels,
            Mode::Floating,
            Viewport {
                width: 1280.0,
                height: 800.0,
                units: Units::CssPx,
            },
        )
        .expect("catalog covers every panel");
        (kit, panels)
    }

    fn seeded_world(kit: BevyPanelKit<TestPanel>) -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(kit);
        world.insert_resource(BevyTheme::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(render_workspace::<TestPanel>);
        let _ = schedule.initialize(&mut world);
        (world, schedule)
    }

    #[test]
    fn renders_panels_chrome_bodies_and_dock() {
        let (kit, _) = two_floating();
        let (mut world, mut schedule) = seeded_world(kit);
        schedule.run(&mut world);

        let mut frames = world.query::<(Entity, &PanelFrame<TestPanel>, &Node, &ZIndex)>();
        let mut found = 0;
        for (_e, frame, node, z) in frames.iter(&world) {
            found += 1;
            match frame.key {
                TestPanel::Scene => {
                    assert_eq!(node.left, Val::Px(40.0));
                    assert_eq!(node.top, Val::Px(40.0));
                    assert_eq!(node.width, Val::Px(400.0));
                    assert_eq!(node.height, Val::Px(300.0));
                    assert_eq!(z.0, 1);
                }
                TestPanel::Inspector => {
                    assert_eq!(node.left, Val::Px(480.0));
                    assert_eq!(z.0, 2);
                }
                TestPanel::Console => panic!("Console was not in the snapshot"),
            }
        }
        assert_eq!(found, 2);

        // Chrome, body, title, and lights per panel.
        let mut chrome = world.query_filtered::<Entity, With<PanelChrome>>();
        assert_eq!(chrome.iter(&world).count(), 2);
        let mut bodies = world.query_filtered::<Entity, With<PanelBody>>();
        assert_eq!(bodies.iter(&world).count(), 2);
        let mut lights = world.query_filtered::<Entity, With<LightRole>>();
        assert_eq!(lights.iter(&world).count(), 6);

        let mut titles = world.query::<&PanelTitle>();
        let mut titles_seen = 0;
        for title in titles.iter(&world) {
            titles_seen += 1;
            assert!(["Scene", "Inspector"].contains(&title.0.as_str()));
        }
        assert_eq!(titles_seen, 2);

        // Nothing minimized: no chips, dock band present.
        let mut chips = world.query_filtered::<Entity, With<DockChip<TestPanel>>>();
        assert_eq!(chips.iter(&world).count(), 0);
        let mut docks = world.query_filtered::<Entity, With<MinimizedDock>>();
        assert_eq!(docks.iter(&world).count(), 1);
    }

    #[test]
    fn root_renders_through_the_named_camera_and_follows_a_change() {
        let (kit, _) = two_floating();
        let (mut world, mut schedule) = seeded_world(kit);
        let first = world.spawn_empty().id();
        let second = world.spawn_empty().id();
        world.insert_resource(PanelUiTargetCamera(first));
        schedule.run(&mut world);

        let mut roots = world.query_filtered::<&UiTargetCamera, With<WorkspaceRoot>>();
        assert_eq!(roots.single(&world).map(|t| t.0).ok(), Some(first));
        // Only the root carries the target; descendants inherit it.
        let mut targeted = world.query_filtered::<Entity, With<UiTargetCamera>>();
        assert_eq!(targeted.iter(&world).count(), 1);

        world.insert_resource(PanelUiTargetCamera(second));
        schedule.run(&mut world);
        let mut roots = world.query_filtered::<&UiTargetCamera, With<WorkspaceRoot>>();
        assert_eq!(roots.single(&world).map(|t| t.0).ok(), Some(second));
    }

    #[test]
    fn root_without_a_target_follows_the_default_ui_camera() {
        let (kit, _) = two_floating();
        let (mut world, mut schedule) = seeded_world(kit);
        schedule.run(&mut world);
        let mut targeted = world.query_filtered::<Entity, With<UiTargetCamera>>();
        assert_eq!(targeted.iter(&world).count(), 0);
    }

    #[test]
    fn minimize_moves_panel_to_dock_without_respawning() {
        let (kit, _) = two_floating();
        let (mut world, mut schedule) = seeded_world(kit);
        schedule.run(&mut world);

        let scene_entity = {
            let mut frames = world.query::<(Entity, &PanelFrame<TestPanel>)>();
            let mut found = None;
            for (e, frame) in frames.iter(&world) {
                if frame.key == TestPanel::Scene {
                    found = Some(e);
                }
            }
            found.expect("scene panel exists")
        };

        world.resource_scope::<BevyPanelKit<TestPanel>, _>(|_world, mut kit| {
            kit.snapshot.panels[0].state = WinState::Minimized;
        });
        schedule.run(&mut world);

        // Same entity, now hidden; a dock chip appeared.
        let mut frames = world.query::<(&PanelFrame<TestPanel>, &Visibility)>();
        let (frame, vis) = frames
            .get(&world, scene_entity)
            .expect("scene still exists");
        assert_eq!(frame.key, TestPanel::Scene);
        assert_eq!(*vis, Visibility::Hidden);

        let mut chips = world.query_filtered::<&DockChip<TestPanel>, With<DockChip<TestPanel>>>();
        let mut chip_keys = Vec::new();
        for chip in chips.iter(&world) {
            chip_keys.push(chip.key);
        }
        assert_eq!(chip_keys, vec![TestPanel::Scene]);

        world.resource_scope::<BevyPanelKit<TestPanel>, _>(|_world, mut kit| {
            kit.snapshot.panels[0].state = WinState::Floating;
        });
        schedule.run(&mut world);

        let mut chips = world.query_filtered::<Entity, With<DockChip<TestPanel>>>();
        assert_eq!(chips.iter(&world).count(), 0);
        let mut frames = world.query::<(&PanelFrame<TestPanel>, &Visibility)>();
        let (_, vis) = frames
            .get(&world, scene_entity)
            .expect("scene still exists");
        assert_eq!(*vis, Visibility::Visible);
    }

    #[test]
    fn updates_geometry_and_keeps_body_content_alive() {
        let (kit, _) = two_floating();
        let (mut world, mut schedule) = seeded_world(kit);
        schedule.run(&mut world);

        // Attach application content to the Scene body slot.
        let body_entity = {
            let mut bodies = world.query_filtered::<(Entity, &ChildOf), With<PanelBody>>();
            let mut found = None;
            for (e, parent) in bodies.iter(&world) {
                if world
                    .get::<PanelFrame<TestPanel>>(parent.0)
                    .is_some_and(|f| f.key == TestPanel::Scene)
                {
                    found = Some(e);
                }
            }
            found.expect("scene body slot exists")
        };
        #[derive(Component)]
        struct AppContent;
        let content = world.spawn((AppContent, ChildOf(body_entity))).id();

        // Move the Inspector and grow the Scene; both nodes must follow.
        world.resource_scope::<BevyPanelKit<TestPanel>, _>(|_world, mut kit| {
            kit.snapshot.panels[1].x += 25.0;
            kit.snapshot.panels[0].w += 60.0;
        });
        schedule.run(&mut world);

        assert!(world.get::<AppContent>(content).is_some());
        let mut frames = world.query::<(&PanelFrame<TestPanel>, &Node)>();
        let mut seen_scene = false;
        let mut seen_inspector = false;
        for (frame, node) in frames.iter(&world) {
            match frame.key {
                TestPanel::Scene => {
                    assert_eq!(node.width, Val::Px(460.0));
                    seen_scene = true;
                }
                TestPanel::Inspector => {
                    assert_eq!(node.left, Val::Px(505.0));
                    seen_inspector = true;
                }
                TestPanel::Console => {}
            }
        }
        assert!(seen_scene && seen_inspector);

        // Removing a panel despawns its node and its body content.
        world.resource_scope::<BevyPanelKit<TestPanel>, _>(|_world, mut kit| {
            kit.snapshot.panels.retain(|p| p.kind != TestPanel::Scene);
        });
        schedule.run(&mut world);

        let mut frames = world.query_filtered::<Entity, With<PanelFrame<TestPanel>>>();
        assert_eq!(frames.iter(&world).count(), 1);
        assert!(world.get::<AppContent>(content).is_none());
    }

    #[test]
    fn too_small_viewports_hide_every_panel() {
        let mut layout = LayoutBuilder::new();
        let panels = vec![layout.at(TestPanel::Scene, 40.0, 40.0, 400.0, 300.0)];
        let kit = BevyPanelKit::new(
            PanelCatalog::from_panel_kind_layout(&panels).expect("test panels serialize"),
            &panels,
            Mode::Floating,
            Viewport {
                width: 0.0,
                height: 0.0,
                units: Units::CssPx,
            },
        )
        .expect("catalog covers every panel");
        let (mut world, mut schedule) = seeded_world(kit);
        schedule.run(&mut world);

        let mut frames = world.query::<(&PanelFrame<TestPanel>, &Visibility)>();
        for (_frame, vis) in frames.iter(&world) {
            assert_eq!(*vis, Visibility::Hidden);
        }
        let mut chips = world.query_filtered::<Entity, With<DockChip<TestPanel>>>();
        assert_eq!(chips.iter(&world).count(), 0);
    }

    #[test]
    fn publishes_body_entity_handles_by_panel_key() {
        let (kit, _) = two_floating();
        let (mut world, mut schedule) = seeded_world(kit);
        schedule.run(&mut world);

        let (tracked, expected_scene, expected_inspector) = {
            let handles = world.resource::<PanelBodyHandles<TestPanel>>();
            (
                handles.iter().collect::<Vec<_>>(),
                handles.get(TestPanel::Scene),
                handles.get(TestPanel::Inspector),
            )
        };
        assert_eq!(tracked.len(), 2);
        for (_key, entity) in tracked {
            let mut bodies = world.query_filtered::<Entity, With<PanelBody>>();
            assert!(bodies.iter(&world).any(|body| body == entity));
        }
        assert!(expected_scene.is_some());
        assert!(expected_inspector.is_some());
    }

    /// Chrome colours come from an inserted non-default [`BevyTheme`]: seeding
    /// the paper palette before the first render paints the workspace
    /// backdrop, panel surfaces, dock band, traffic lights, and title text
    /// with the paper slots instead of the dark default.
    #[test]
    fn chrome_colours_come_from_an_inserted_theme() {
        let (kit, _) = two_floating();
        let (mut world, mut schedule) = seeded_world(kit);
        let theme = BevyTheme::from_tokens(&ThemeTokens::paper());
        world.insert_resource(theme.clone());
        schedule.run(&mut world);

        let mut frames = world.query::<(&PanelFrame<TestPanel>, &BackgroundColor, &BorderColor)>();
        for (_frame, background, border) in frames.iter(&world) {
            assert_eq!(background.0, theme.surface);
            assert_eq!(border.0, theme.border);
        }

        let mut docks = world.query::<(&DockNode, &BackgroundColor, &BorderColor)>();
        let (_, background, border) = docks.single(&world).expect("dock band exists");
        assert_eq!(background.0, theme.surface);
        assert_eq!(border.0, theme.divider);

        let mut lights = world.query::<(&LightRole, &BackgroundColor)>();
        for (role, background) in lights.iter(&world) {
            let expected = match role {
                LightRole::Mode => theme.primary,
                LightRole::Minimize => theme.tertiary,
                LightRole::Maximize => theme.secondary,
            };
            assert_eq!(background.0, expected, "light {role:?}");
        }

        let mut titles = world.query_filtered::<Entity, With<PanelTitle>>();
        let titles: Vec<Entity> = titles.iter(&world).collect();
        assert_eq!(titles.len(), 2);
        for title in titles {
            let mut glyphs = 0;
            for glyph in world.iter_entities().filter(|entity| entity.contains::<TextColor>()) {
                if !world.get::<ChildOf>(glyph.id()).is_some_and(|parent| {
                    world.get::<ChildOf>(parent.0).is_some_and(|row| row.0 == title)
                }) {
                    continue;
                }
                glyphs += 1;
                assert_eq!(glyph.get::<TextColor>().map(|tint| tint.0), Some(theme.text));
            }
            assert!(glyphs > 0, "title glyphs carry the theme's text colour");
        }
    }

    /// Header inset, light size and gap, title gap, body padding and grip size
    /// all come from the theme's core `PanelPaint`, so a different paint
    /// moves every chrome part with it.
    #[test]
    fn chrome_geometry_comes_from_core_panel_paint() {
        let (kit, _) = two_floating();
        let (mut world, mut schedule) = seeded_world(kit);
        let mut theme = BevyTheme::default();
        theme.panel.head_inset = 9.0;
        theme.panel.head_pad = 5.0;
        theme.panel.light = 15.0;
        theme.panel.light_gap = 7.0;
        theme.panel.title_gap = 10.0;
        theme.panel.body_pad_top = theme.panel.head_h + 4.0;
        theme.panel.body_pad_x = 11.0;
        theme.panel.body_pad_bottom = 12.0;
        theme.panel.grip = 17.0;
        world.insert_resource(theme);
        schedule.run(&mut world);

        let mut headers = world.query_filtered::<&Node, With<PanelChrome>>();
        for header in headers.iter(&world) {
            assert_eq!((header.left, header.right), (Val::Px(9.0), Val::Px(9.0)));
            assert_eq!(header.padding.right, Val::Px(5.0));
            assert_eq!(header.column_gap, Val::Px(7.0));
        }
        let mut lights = world.query_filtered::<&Node, With<LightRole>>();
        for light in lights.iter(&world) {
            assert_eq!((light.width, light.height), (Val::Px(15.0), Val::Px(15.0)));
        }
        let mut titles = world.query_filtered::<&Node, With<PanelTitle>>();
        for title in titles.iter(&world) {
            assert_eq!(title.margin.left, Val::Px(3.0), "title gap less the light gap");
        }
        let mut bodies = world.query_filtered::<&Node, With<PanelBody>>();
        for body in bodies.iter(&world) {
            assert_eq!(
                body.padding,
                UiRect {
                    left: Val::Px(11.0),
                    right: Val::Px(11.0),
                    top: Val::Px(4.0),
                    bottom: Val::Px(12.0),
                }
            );
        }
        let mut grips = world.query_filtered::<&Node, With<ResizeGrip>>();
        assert_eq!(grips.iter(&world).count(), 2);
        for grip in grips.iter(&world) {
            assert_eq!((grip.width, grip.height), (Val::Px(17.0), Val::Px(17.0)));
        }
    }
}
