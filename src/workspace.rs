//! Host-owned workspace plumbing shared by every web consumer.
//!
//! panel-kit exposes the reducer, the frame projection, and the widgets, but a
//! host still has to own a snapshot, a projection scratch buffer, a catalog,
//! and layout persistence. Every app wrote that glue itself until this module
//! existed; the copies drifted. [`PanelWorkspace`] is that glue, once.
//!
//! ```ignore
//! let workspace = use_panel_workspace(
//!     "my-app-layout-v1",
//!     default_layout,
//!     app_catalog,
//!     WorkspaceOptions { default_mode: Mode::Tiling, ..Default::default() },
//! );
//! mount_viewport_observer(&workspace);
//! let snapshot = workspace.snapshot.read();
//! let mut scratch = workspace.scratch.borrow_mut();
//! let frame = project_workspace(&workspace, &snapshot, &mut scratch);
//! ```
//!
//! Workspaces that offer named views (preset panel sets with their own stored
//! layout) are built with [`use_panel_views`] and a [`ViewsSpec`]; switching,
//! saving, resetting, and deleting views goes through [`PanelWorkspace`].

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::events::{KeyboardEvent, PointerEvent as DioxusPointerEvent, WheelEvent};
use dioxus::prelude::*;
use panel_kit_core::frame::{
    project_into, ChromeProjectionInput, Placement, ProjectedFrame, ProjectionBuffer,
    ProjectionInput, TileFillOrder, TileLayoutMetrics,
};
use panel_kit_core::persist::{
    apply_save_decision, persist_snapshot, restore_snapshot, LayoutError, RestoreContext,
    SavePolicy,
};
use panel_kit_core::reducer::{
    reduce, HitTarget, ReduceContext, ResizePolicy, Snapshot, Viewport, WorkspaceEvent,
};
use panel_kit_core::views::{view_layout_key, views_registry_key, SavedViews};
use panel_kit_core::{
    ChromeMetrics, Clamp, CommandStep, FocusContext, Mode, PanelCatalog, PanelKind, PanelWin,
    PointerButton, PointerEventKind, SnapPolicy, TileMetrics, Units, WinState,
};

use crate::input::{clear_selection, keyboard_event, pointer_event, release_pointer, wheel_event};
use crate::store::LocalStorageLayoutStore;
use crate::surface::{observe_viewport, surface_profile, viewport_size};

/// Host-chosen workspace policy that is not part of persisted layout state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkspaceOptions {
    /// Mode used when no saved layout exists.
    pub default_mode: Mode,
    /// Order auto-placed tiled panels fill the grid in.
    pub fill_order: TileFillOrder,
    /// Pointer snapping for move and resize gestures.
    pub snap: SnapPolicy,
}

impl Default for WorkspaceOptions {
    fn default() -> Self {
        Self {
            default_mode: Mode::Floating,
            fill_order: TileFillOrder::default(),
            snap: SnapPolicy::default(),
        }
    }
}

impl WorkspaceOptions {
    /// Options for a tiling-by-default workspace.
    pub fn tiling() -> Self {
        Self {
            default_mode: Mode::Tiling,
            ..Self::default()
        }
    }

    /// Options for continuous (unsnapped) floating move and resize.
    pub fn continuous() -> Self {
        Self {
            snap: SnapPolicy {
                resize: false,
                move_: false,
                ..SnapPolicy::default()
            },
            ..Self::default()
        }
    }
}

/// One code-defined view: visible panels in tiling order with their spans.
///
/// Panels not listed are minimized into the dock when the view is active.
pub struct ViewPreset<K: PanelKind> {
    /// View name shown in the switcher; also the stored layout's key suffix.
    pub name: &'static str,
    /// Mode the view starts in.
    pub mode: Mode,
    /// Visible panels as `(panel, tile_w, tile_h)`.
    pub panels: &'static [(K, u8, u8)],
}

/// Named views configuration for one workspace.
pub struct ViewsSpec<K: PanelKind> {
    /// Storage-key base: the registry lives at `{base}:views`, each view's
    /// layout at `{base}:view:{name}`.
    pub base: &'static str,
    /// Views the app ships; they cannot be deleted.
    pub presets: &'static [ViewPreset<K>],
}

impl<K: PanelKind> ViewsSpec<K> {
    /// Registry naming every preset, first preset active.
    pub fn fresh_registry(&self) -> SavedViews {
        let names: Vec<&str> = self.presets.iter().map(|preset| preset.name).collect();
        SavedViews::new(&names)
    }

    /// Re-add presets missing from a stored registry, keeping custom views.
    pub fn sanitize(&self, registry: SavedViews) -> SavedViews {
        let mut registry = registry.sanitize();
        for preset in self.presets {
            if !registry.views.iter().any(|view| view == preset.name) {
                let _ = registry.add(preset.name);
            }
        }
        registry
    }

    /// Storage key of the views registry.
    pub fn registry_key(&self) -> String {
        views_registry_key(self.base)
    }

    /// Storage key of one view's layout.
    pub fn layout_key(&self, view: &str) -> String {
        view_layout_key(self.base, view)
    }

    /// The preset named `view`, if any.
    pub fn preset(&self, view: &str) -> Option<&ViewPreset<K>> {
        self.presets.iter().find(|preset| preset.name == view)
    }

    /// Whether `view` is an app preset (custom views are deletable).
    pub fn is_preset(&self, view: &str) -> bool {
        self.preset(view).is_some()
    }

    /// Panels for `view`: the preset's panels first, then every other shipped
    /// panel minimized. Custom or unknown views use the first preset.
    pub fn panels_for(&self, view: &str, defaults: &[PanelWin<K>]) -> (Vec<PanelWin<K>>, Mode) {
        let preset = self.preset(view).unwrap_or(&self.presets[0]);
        let mut panels = Vec::with_capacity(defaults.len());
        for &(kind, w, h) in preset.panels {
            if let Some(default) = defaults.iter().find(|default| default.kind == kind) {
                let mut win = default.with_tile(w, h);
                win.state = WinState::Floating;
                panels.push(win);
            }
        }
        for default in defaults {
            if !preset.panels.iter().any(|(kind, _, _)| *kind == default.kind) {
                let mut win = *default;
                win.state = WinState::Minimized;
                panels.push(win);
            }
        }
        (panels, preset.mode)
    }
}

/// One workspace's snapshot, catalog, projection scratch, and persistence.
///
/// Created by [`use_panel_workspace`] or [`use_panel_views`]; cheap to, and
/// expected to be, cloned into event handlers and child components.
pub struct PanelWorkspace<K: PanelKind> {
    /// The current layout snapshot; read to render, write through the reducer.
    pub snapshot: Signal<Snapshot<K>>,
    /// Immutable panel identity and presentation metadata.
    pub catalog: Rc<PanelCatalog<K>>,
    /// Reused projection buffer, borrowed mutably for one render pass.
    pub scratch: Rc<RefCell<ProjectionBuffer<K>>>,
    /// Named-view registry; holds a single `"Main"` view when unused.
    pub registry: Signal<SavedViews>,
    /// Named views this workspace offers, if any.
    pub views: Option<&'static ViewsSpec<K>>,
    store_key: Signal<String>,
    save_policy: SavePolicy,
    fill_order: TileFillOrder,
    snap: SnapPolicy,
    defaults: fn() -> Vec<PanelWin<K>>,
    default_mode: Mode,
    floors: Rc<Vec<(K, u8)>>,
}

impl<K: PanelKind> Clone for PanelWorkspace<K> {
    fn clone(&self) -> Self {
        Self {
            snapshot: self.snapshot,
            catalog: self.catalog.clone(),
            scratch: self.scratch.clone(),
            registry: self.registry,
            views: self.views,
            store_key: self.store_key,
            save_policy: self.save_policy.clone(),
            fill_order: self.fill_order,
            snap: self.snap,
            defaults: self.defaults,
            default_mode: self.default_mode,
            floors: self.floors.clone(),
        }
    }
}

impl<K: PanelKind> PartialEq for PanelWorkspace<K> {
    fn eq(&self, other: &Self) -> bool {
        self.snapshot == other.snapshot
            && Rc::ptr_eq(&self.catalog, &other.catalog)
            && Rc::ptr_eq(&self.scratch, &other.scratch)
            && self.registry == other.registry
            && self.store_key == other.store_key
            && Rc::ptr_eq(&self.floors, &other.floors)
            && self.fill_order == other.fill_order
            && self.snap == other.snap
    }
}

impl<K: PanelKind> PanelWorkspace<K> {
    /// Storage key the active layout persists under.
    pub fn layout_key(&self) -> String {
        self.store_key.peek().clone()
    }

    /// The active view's name (`"Main"` when the workspace has no views).
    pub fn active_view(&self) -> String {
        self.registry.peek().active.clone()
    }

    fn store(&self) -> LocalStorageLayoutStore {
        LocalStorageLayoutStore::new(self.layout_key().as_str())
    }

    /// Default snapshot for `view`, keeping the live viewport.
    fn snapshot_for_view(&self, view: &str) -> Snapshot<K> {
        let (panels, mode) = match self.views {
            Some(spec) => spec.panels_for(view, &(self.defaults)()),
            None => ((self.defaults)(), self.default_mode),
        };
        let mut snapshot = Snapshot::from_defaults(panels, mode, current_viewport());
        snapshot.viewport = self.snapshot.peek().viewport;
        snapshot
    }

    fn load_view(&self, view: &str) {
        let key = match self.views {
            Some(spec) => spec.layout_key(view),
            None => return,
        };
        let mut store_key = self.store_key;
        store_key.set(key.clone());
        let defaults = self.snapshot_for_view(view);
        let mut next = restore_or_default(&key, defaults, &self.catalog);
        apply_tile_floors(&mut next, self.floors.as_slice());
        let mut snapshot = self.snapshot;
        snapshot.set(next);
    }

    /// Persist the current layout as the active view, then activate `name`.
    pub fn switch_view(&self, name: &str) {
        let Some(spec) = self.views else { return };
        let mut registry = self.registry;
        let mut next = registry.peek().clone();
        if next.active == name || next.activate(name).is_err() {
            return;
        }
        self.persist_active_layout();
        save_registry(&spec.registry_key(), &next);
        self.load_view(&next.active);
        registry.set(next);
    }

    /// Restore the active view to its preset and forget its stored layout.
    pub fn reset_view(&self) {
        let Some(spec) = self.views else { return };
        let active = self.registry.peek().active.clone();
        forget_layout(&spec.layout_key(&active));
        let fresh = self.snapshot_for_view(&active);
        let mut snapshot = self.snapshot;
        snapshot.set(fresh);
    }

    /// Create a custom view from the current layout and activate it.
    pub fn save_view_as(&self, name: &str) -> Result<(), String> {
        let Some(spec) = self.views else {
            return Err("views disabled".into());
        };
        let mut registry = self.registry;
        let mut next = registry.peek().clone();
        next.add(name).map_err(|error| error.to_string())?;
        let name = next.views.last().cloned().unwrap_or_default();
        self.persist_active_layout();
        let key = spec.layout_key(&name);
        let snapshot = self.snapshot.peek();
        if let Err(error) = persist_snapshot(
            &LocalStorageLayoutStore::new(key.as_str()),
            &snapshot,
            &self.catalog,
        ) {
            log_layout_error("save view layout", &key, &error);
        }
        next.activate(&name).map_err(|error| error.to_string())?;
        save_registry(&spec.registry_key(), &next);
        let mut store_key = self.store_key;
        store_key.set(key);
        registry.set(next);
        Ok(())
    }

    /// Delete a custom view; presets are permanent.
    pub fn delete_view(&self, name: &str) {
        let Some(spec) = self.views else { return };
        if spec.is_preset(name) {
            return;
        }
        let mut registry = self.registry;
        let mut next = registry.peek().clone();
        let was_active = next.active == name;
        if next.remove(name).is_err() {
            return;
        }
        forget_layout(&spec.layout_key(name));
        save_registry(&spec.registry_key(), &next);
        if was_active {
            self.load_view(&next.active);
        }
        registry.set(next);
    }

    fn persist_active_layout(&self) {
        let key = self.layout_key();
        let snapshot = self.snapshot.peek();
        if let Err(error) = persist_snapshot(&self.store(), &snapshot, &self.catalog) {
            log_layout_error("save layout", &key, &error);
        }
    }
}

/// Create (or restore) one workspace for the calling component.
///
/// `defaults` produces the shipped layout; `catalog` builds the panel metadata
/// once. Both are function pointers so hook ordering stays stable.
pub fn use_panel_workspace<K: PanelKind>(
    storage_key: &'static str,
    defaults: fn() -> Vec<PanelWin<K>>,
    catalog: fn() -> Rc<PanelCatalog<K>>,
    options: WorkspaceOptions,
) -> PanelWorkspace<K> {
    use_workspace(storage_key, defaults, catalog, options, None)
}

/// Create one workspace that offers named views over `defaults`.
///
/// Each view persists under the views key scheme derived from `spec.base`, so
/// `storage_key` is ignored; it is kept in the signature so callers can switch
/// between [`use_panel_workspace`] and this constructor without reshaping
/// their call.
pub fn use_panel_views<K: PanelKind>(
    storage_key: &'static str,
    defaults: fn() -> Vec<PanelWin<K>>,
    catalog: fn() -> Rc<PanelCatalog<K>>,
    views: &'static ViewsSpec<K>,
    options: WorkspaceOptions,
) -> PanelWorkspace<K> {
    use_workspace(storage_key, defaults, catalog, options, Some(views))
}

fn use_workspace<K: PanelKind>(
    storage_key: &'static str,
    defaults: fn() -> Vec<PanelWin<K>>,
    catalog: fn() -> Rc<PanelCatalog<K>>,
    options: WorkspaceOptions,
    views: Option<&'static ViewsSpec<K>>,
) -> PanelWorkspace<K> {
    let shipped: Rc<Vec<PanelWin<K>>> = use_hook(move || Rc::new(defaults()));
    let catalog = use_hook(catalog);
    let registry = use_signal(move || match views {
        Some(spec) => load_registry(&spec.registry_key())
            .map(|stored| spec.sanitize(stored))
            .unwrap_or_else(|| spec.fresh_registry()),
        None => SavedViews::new(&["Main"]),
    });
    let store_key = use_signal(move || match views {
        Some(spec) => spec.layout_key(registry.peek().active.as_str()),
        None => storage_key.to_string(),
    });
    let floors = use_hook({
        let shipped = shipped.clone();
        move || {
            if views.is_some() {
                return Rc::new(Vec::new());
            }
            Rc::new(
                shipped
                    .iter()
                    .map(|panel| (panel.kind, panel.tile_h))
                    .collect::<Vec<_>>(),
            )
        }
    });
    let snapshot = use_signal({
        let catalog = catalog.clone();
        let shipped = shipped.clone();
        let floors = floors.clone();
        move || {
            let key = store_key.peek().clone();
            let (panels, mode) = match views {
                Some(spec) => spec.panels_for(registry.peek().active.as_str(), &shipped),
                None => (shipped.as_ref().clone(), options.default_mode),
            };
            let defaults = Snapshot::from_defaults(panels, mode, current_viewport());
            let mut snapshot = restore_or_default(&key, defaults, &catalog);
            apply_tile_floors(&mut snapshot, floors.as_slice());
            snapshot
        }
    });
    let scratch = use_hook({
        let panel_count = catalog.len();
        move || Rc::new(RefCell::new(ProjectionBuffer::with_panel_capacity(panel_count)))
    });

    PanelWorkspace {
        snapshot,
        catalog,
        scratch,
        registry,
        views,
        store_key,
        save_policy: SavePolicy::OnSettle,
        fill_order: options.fill_order,
        snap: options.snap,
        defaults,
        default_mode: options.default_mode,
        floors,
    }
}

/// Follow browser viewport changes, scaling floating panels with the window.
pub fn mount_viewport_observer<K: PanelKind>(workspace: &PanelWorkspace<K>) {
    let emit = workspace_event_handler(workspace);
    let _status = observe_viewport(EventHandler::new(move |size: Viewport| {
        emit.call(WorkspaceEvent::ViewportChanged {
            size,
            policy: ResizePolicy::ScaleFloating,
        });
    }));
}

/// Project the current snapshot into caller-owned scratch for one render pass.
pub fn project_workspace<'frame, K: PanelKind>(
    workspace: &PanelWorkspace<K>,
    snapshot: &Snapshot<K>,
    scratch: &'frame mut ProjectionBuffer<K>,
) -> ProjectedFrame<'frame, K> {
    let surface = surface_profile(snapshot.viewport.width);
    let chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
    let tile = TileLayoutMetrics::from_tile_metrics(TileMetrics::WEB, surface)
        .with_fill_order(workspace.fill_order);

    project_into(
        ProjectionInput {
            snapshot,
            surface,
            chrome: &chrome,
            clamp: &Clamp::WEB,
            tile: &tile,
        },
        scratch,
    )
}

/// CSS class for the workspace area, reflecting mode and maximized panels.
pub fn workspace_area_class<K: PanelKind>(frame: &ProjectedFrame<'_, K>) -> &'static str {
    if frame
        .panels
        .iter()
        .any(|panel| matches!(panel.placement, Placement::Maximized))
    {
        "ws maxed"
    } else if frame.mode == Mode::Tiling {
        "ws tiling"
    } else {
        "ws floating"
    }
}

/// Paint one projected panel: shell, chrome with traffic lights, body, grip.
///
/// Hosts keep their own loop over `frame.panels` (they differ in keep-alive
/// slots, per-view chrome, and wheel routing) but not this composition.
pub fn panel_element<K: PanelKind>(
    panel: panel_kit_core::frame::PanelProjection<K>,
    meta: &panel_kit_core::PanelMeta<K>,
    emit: EventHandler<WorkspaceEvent<K>>,
    body: Element,
    header_actions: Option<Element>,
) -> Element {
    let class = format!("panel-{}", meta.slug);
    let chrome = crate::widgets::panel::panel_chrome_with_events(
        panel,
        meta,
        emit,
        Some(crate::widgets::panel::traffic_lights(panel, emit)),
        header_actions,
    );

    crate::widgets::panel::panel_shell_with_events(
        panel,
        Some(&class),
        rsx! {
            {chrome}
            {crate::widgets::panel::panel_body(body)}
            {crate::widgets::panel::resize_grip(panel, emit)}
        },
        emit,
    )
}

/// Mutate the snapshot through a persistence-aware write.
pub fn mutate_snapshot<K: PanelKind>(
    workspace: &PanelWorkspace<K>,
    mutate: impl FnOnce(&mut Snapshot<K>),
) {
    let mut snapshot_signal = workspace.snapshot;
    let mut snapshot = snapshot_signal.write();
    mutate(&mut snapshot);
    if let Err(error) = persist_snapshot(&workspace.store(), &snapshot, &workspace.catalog) {
        log_layout_error("save layout", &workspace.layout_key(), &error);
    }
}

/// Restore the shipped layout, discarding the saved one.
///
/// On a views workspace this resets the active view to its preset.
pub fn reset_workspace<K: PanelKind>(workspace: &PanelWorkspace<K>) {
    if workspace.views.is_some() {
        workspace.reset_view();
        return;
    }
    let fresh = workspace.snapshot_for_view(workspace.active_view().as_str());
    let mut snapshot_signal = workspace.snapshot;
    snapshot_signal.set(fresh);
    let mut snapshot = snapshot_signal.write();
    if let Err(error) = persist_snapshot(&workspace.store(), &snapshot, &workspace.catalog) {
        log_layout_error("save layout", &workspace.layout_key(), &error);
    }
}

/// Event handler that reduces into one workspace and applies the save policy.
pub fn workspace_event_handler<K: PanelKind>(
    workspace: &PanelWorkspace<K>,
) -> EventHandler<WorkspaceEvent<K>> {
    let workspace = workspace.clone();
    EventHandler::new(move |event| {
        reduce_workspace_event(&workspace, event);
    })
}

/// Reduce root keyboard events after app handlers decline first refusal.
pub fn handle_key<K: PanelKind>(workspace: &PanelWorkspace<K>, event: &KeyboardEvent) {
    let focus = if crate::input::is_editing() {
        FocusContext::TextInput
    } else if let Some(key) = workspace.snapshot.read().focused {
        FocusContext::Panel(key)
    } else {
        FocusContext::Workspace
    };

    let Some(workspace_event) = keyboard_event(event, focus) else {
        return;
    };
    if reduce_workspace_event(workspace, workspace_event) {
        event.prevent_default();
    }
}

/// Reduce in-flight workspace pointer motion.
pub fn handle_pointer_move<K: PanelKind>(
    workspace: &PanelWorkspace<K>,
    event: &DioxusPointerEvent,
) {
    let kind = if workspace.snapshot.read().drag.is_some() {
        PointerEventKind::Drag(PointerButton::Primary)
    } else {
        PointerEventKind::Moved
    };
    reduce_workspace_event(workspace, pointer_event(HitTarget::Workspace, event, kind));
}

/// Settle root pointer gestures and persist the settled layout.
pub fn handle_pointer_up<K: PanelKind>(workspace: &PanelWorkspace<K>, event: &DioxusPointerEvent) {
    release_pointer(event);
    let changed = reduce_workspace_event(
        workspace,
        pointer_event(
            HitTarget::Workspace,
            event,
            PointerEventKind::Up(PointerButton::Primary),
        ),
    );
    if changed {
        clear_selection();
    }
}

/// Reduce wheel input after native panel-body scroll gets first refusal.
pub fn handle_wheel<K: PanelKind>(workspace: &PanelWorkspace<K>, event: &WheelEvent) {
    if reduce_workspace_event(workspace, wheel_event(event)) {
        event.prevent_default();
    }
}

/// Apply one reducer event and the workspace's persistence policy.
pub fn reduce_workspace_event<K: PanelKind>(
    workspace: &PanelWorkspace<K>,
    event: WorkspaceEvent<K>,
) -> bool {
    let changed = reduce_locked(workspace, event);
    if changed {
        // Dispatch after the snapshot guard is released: listeners such as the
        // viewport observer reduce into the same signal.
        if let Some(window) = web_sys::window() {
            let _ = window.dispatch_event(&web_sys::Event::new("resize").unwrap());
        }
    }
    changed
}

fn reduce_locked<K: PanelKind>(workspace: &PanelWorkspace<K>, event: WorkspaceEvent<K>) -> bool {
    let mut snapshot_signal = workspace.snapshot;
    let mut snapshot = snapshot_signal.write();
    let context = ReduceContext {
        surface: surface_profile(snapshot.viewport.width),
        clamp: &Clamp::WEB,
        command_step: CommandStep::WEB,
        tile: &TileMetrics::WEB,
        snap: workspace.snap,
    };
    let reduction = reduce(&mut snapshot, event, context);
    let changed = reduction.changed;
    let decision = workspace.save_policy.decide(&reduction);

    if let Err(error) =
        apply_save_decision(decision, &workspace.store(), &snapshot, &workspace.catalog)
    {
        log_layout_error("save layout", &workspace.layout_key(), &error);
    }

    changed
}

/// Keep shipped tile-height floors after restoring an older saved layout.
fn apply_tile_floors<K: PanelKind>(snapshot: &mut Snapshot<K>, shipped: &[(K, u8)]) {
    for panel in &mut snapshot.panels {
        if let Some((_, minimum)) = shipped.iter().find(|(kind, _)| *kind == panel.kind) {
            panel.tile_h = panel.tile_h.max(*minimum);
        }
    }
}

fn current_viewport() -> Viewport {
    let (width, height) = viewport_size();
    Viewport {
        width,
        height,
        units: Units::CssPx,
    }
}

fn restore_or_default<K: PanelKind>(
    key: &str,
    defaults: Snapshot<K>,
    catalog: &PanelCatalog<K>,
) -> Snapshot<K> {
    let context = RestoreContext {
        units: Units::CssPx,
        viewport: (defaults.viewport.width, defaults.viewport.height),
    };

    match restore_snapshot(&LocalStorageLayoutStore::new(key), defaults.clone(), catalog, context) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            log_layout_error("restore layout", key, &error);
            defaults
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn load_registry(key: &str) -> Option<SavedViews> {
    let storage = web_sys::window()?.local_storage().ok()??;
    let raw = storage.get_item(key).ok()??;
    serde_json::from_str(&raw).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn load_registry(_key: &str) -> Option<SavedViews> {
    None
}

#[cfg(target_arch = "wasm32")]
fn save_registry(key: &str, registry: &SavedViews) {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    if let Ok(raw) = serde_json::to_string(registry) {
        let _ = storage.set_item(key, &raw);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_registry(_key: &str, _registry: &SavedViews) {}

#[cfg(target_arch = "wasm32")]
fn forget_layout(key: &str) {
    if let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.remove_item(key);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn forget_layout(_key: &str) {}

fn log_layout_error(action: &str, storage_key: &str, error: &LayoutError) {
    let message = format!("panel-kit {action} failed for storage key `{storage_key}`: {error}");

    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&message));

    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{message}");
}

#[cfg(test)]
mod views_tests {
    use super::*;
    use crate::LayoutBuilder;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
    enum P {
        A,
        B,
        C,
    }

    impl PanelKind for P {
        fn title(self) -> &'static str {
            "panel"
        }
    }

    static SPEC: ViewsSpec<P> = ViewsSpec {
        base: "views-test",
        presets: &[
            ViewPreset {
                name: "One",
                mode: Mode::Tiling,
                panels: &[(P::B, 3, 4), (P::A, 1, 2)],
            },
            ViewPreset {
                name: "Two",
                mode: Mode::Floating,
                panels: &[(P::C, 4, 4)],
            },
        ],
    };

    fn defaults() -> Vec<PanelWin<P>> {
        let mut builder = LayoutBuilder::new();
        vec![
            builder.at(P::A, 0.0, 0.0, 10.0, 10.0),
            builder.at(P::B, 0.0, 0.0, 10.0, 10.0),
            builder.at(P::C, 0.0, 0.0, 10.0, 10.0),
        ]
    }

    #[test]
    fn preset_orders_visible_panels_and_minimizes_the_rest() {
        let (panels, mode) = SPEC.panels_for("One", &defaults());
        assert_eq!(mode, Mode::Tiling);
        let order: Vec<(P, WinState, u8, u8)> = panels
            .iter()
            .map(|panel| (panel.kind, panel.state, panel.tile_w, panel.tile_h))
            .collect();
        assert_eq!(order[0], (P::B, WinState::Floating, 3, 4));
        assert_eq!(order[1], (P::A, WinState::Floating, 1, 2));
        assert_eq!((order[2].0, order[2].1), (P::C, WinState::Minimized));
    }

    #[test]
    fn custom_view_falls_back_to_the_first_preset() {
        let (panels, mode) = SPEC.panels_for("mine", &defaults());
        assert_eq!(panels[0].kind, P::B);
        assert_eq!(mode, Mode::Tiling);
    }

    #[test]
    fn sanitize_readds_presets_and_keeps_custom_views() {
        let stored = SavedViews {
            views: vec!["mine".into()],
            active: "mine".into(),
        };
        let registry = SPEC.sanitize(stored);
        assert!(registry.views.iter().any(|view| view == "One"));
        assert!(registry.views.iter().any(|view| view == "Two"));
        assert!(registry.views.iter().any(|view| view == "mine"));
        assert_eq!(registry.active, "mine");
        assert!(!SPEC.is_preset("mine"));
    }

    #[test]
    fn layout_keys_are_namespaced_per_view() {
        assert_eq!(SPEC.registry_key(), views_registry_key("views-test"));
        assert_eq!(SPEC.layout_key("One"), view_layout_key("views-test", "One"));
        assert_ne!(SPEC.layout_key("One"), SPEC.layout_key("Two"));
    }
}
