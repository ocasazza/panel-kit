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
use panel_kit_core::{
    ChromeMetrics, Clamp, CommandStep, FocusContext, Mode, PanelCatalog, PanelKind, PanelWin,
    PointerButton, PointerEventKind, SnapPolicy, TileMetrics, Units,
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

/// One workspace's snapshot, catalog, projection scratch, and persistence.
///
/// Created by [`use_panel_workspace`]; cheap to, and expected to be, cloned
/// into event handlers and child components.
pub struct PanelWorkspace<K: PanelKind> {
    /// localStorage key this workspace persists under.
    pub storage_key: &'static str,
    /// The current layout snapshot; read to render, write through the reducer.
    pub snapshot: Signal<Snapshot<K>>,
    /// Immutable panel identity and presentation metadata.
    pub catalog: Rc<PanelCatalog<K>>,
    /// Reused projection buffer, borrowed mutably for one render pass.
    pub scratch: Rc<RefCell<ProjectionBuffer<K>>>,
    store: Rc<LocalStorageLayoutStore>,
    save_policy: SavePolicy,
    fill_order: TileFillOrder,
    snap: SnapPolicy,
    defaults: fn() -> Vec<PanelWin<K>>,
    default_mode: Mode,
}

impl<K: PanelKind> Clone for PanelWorkspace<K> {
    fn clone(&self) -> Self {
        Self {
            storage_key: self.storage_key,
            snapshot: self.snapshot,
            catalog: self.catalog.clone(),
            scratch: self.scratch.clone(),
            store: self.store.clone(),
            save_policy: self.save_policy.clone(),
            fill_order: self.fill_order,
            snap: self.snap,
            defaults: self.defaults,
            default_mode: self.default_mode,
        }
    }
}

impl<K: PanelKind> PartialEq for PanelWorkspace<K> {
    fn eq(&self, other: &Self) -> bool {
        self.storage_key == other.storage_key
            && self.snapshot == other.snapshot
            && Rc::ptr_eq(&self.catalog, &other.catalog)
            && Rc::ptr_eq(&self.scratch, &other.scratch)
            && Rc::ptr_eq(&self.store, &other.store)
            && self.fill_order == other.fill_order
            && self.snap == other.snap
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
    let initial_viewport = current_viewport();
    let default_snapshot = Snapshot::from_defaults(defaults(), options.default_mode, initial_viewport);
    let catalog = use_hook(catalog);
    let store = use_hook(move || Rc::new(LocalStorageLayoutStore::new(storage_key)));
    let snapshot = use_signal({
        let catalog = catalog.clone();
        let store = store.clone();
        let defaults = default_snapshot.clone();
        move || restore_or_default(storage_key, &store, defaults, &catalog)
    });
    let scratch = use_hook({
        let panel_count = catalog.len();
        move || Rc::new(RefCell::new(ProjectionBuffer::with_panel_capacity(panel_count)))
    });

    PanelWorkspace {
        storage_key,
        snapshot,
        catalog,
        scratch,
        store,
        save_policy: SavePolicy::OnSettle,
        fill_order: options.fill_order,
        snap: options.snap,
        defaults,
        default_mode: options.default_mode,
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
    if let Err(error) = persist_snapshot(&*workspace.store, &snapshot, &workspace.catalog) {
        log_layout_error("save layout", workspace.storage_key, &error);
    }
}

/// Restore the shipped default layout, discarding the saved one.
pub fn reset_workspace<K: PanelKind>(workspace: &PanelWorkspace<K>) {
    let viewport = workspace.snapshot.peek().viewport;
    let fresh = Snapshot::from_defaults((workspace.defaults)(), workspace.default_mode, viewport);
    let mut snapshot_signal = workspace.snapshot;
    snapshot_signal.set(fresh);
    let mut snapshot = snapshot_signal.write();
    if let Err(error) = persist_snapshot(&*workspace.store, &snapshot, &workspace.catalog) {
        log_layout_error("save layout", workspace.storage_key, &error);
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
        apply_save_decision(decision, &*workspace.store, &snapshot, &workspace.catalog)
    {
        log_layout_error("save layout", workspace.storage_key, &error);
    }
    if changed {
        if let Some(window) = web_sys::window() {
            let _ = window.dispatch_event(&web_sys::Event::new("resize").unwrap());
        }
    }

    changed
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
    storage_key: &str,
    store: &LocalStorageLayoutStore,
    defaults: Snapshot<K>,
    catalog: &PanelCatalog<K>,
) -> Snapshot<K> {
    let context = RestoreContext {
        units: Units::CssPx,
        viewport: (defaults.viewport.width, defaults.viewport.height),
    };

    match restore_snapshot(store, defaults.clone(), catalog, context) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            log_layout_error("restore layout", storage_key, &error);
            defaults
        }
    }
}

fn log_layout_error(action: &str, storage_key: &str, error: &LayoutError) {
    let message = format!("panel-kit {action} failed for storage key `{storage_key}`: {error}");

    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&message));

    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{message}");
}
