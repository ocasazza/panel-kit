//! Host-owned workspace wiring for the spec-driven web canary example.
//!
//! This is example support, not a public controller facade: the example still
//! owns startup decoding, state, event policy, persistence timing, projection,
//! and render order. The helper only keeps the mechanical reducer/persistence
//! loop readable.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::events::{KeyboardEvent, PointerEvent as DioxusPointerEvent, WheelEvent};
use dioxus::prelude::*;
use panel_kit::input::{clear_selection, keyboard_event, pointer_event, release_pointer, wheel_event};
use panel_kit::store::LocalStorageLayoutStore;
use panel_kit::surface::{browser_capabilities, observe_viewport};
use panel_kit_core::frame::{
    project_into, ChromeProjectionInput, Placement, ProjectedFrame, ProjectionBuffer,
    ProjectionInput, TileLayoutMetrics,
};
use panel_kit_core::persist::{
    apply_save_decision, restore_snapshot, LayoutError, RestoreContext,
};
use panel_kit_core::reducer::{reduce, HitTarget, Snapshot, Viewport, WorkspaceEvent};
use panel_kit_core::spec::{BackendKind, BindingManifest, ResolvedWorkspace, WorkspaceSpec};
use panel_kit_core::{
    FocusContext, Mode, PointerButton, PointerEventKind, SpecPanelId, SurfaceProfile,
};

/// Host-owned state and ports for one spec-resolved web workspace.
#[derive(Clone)]
pub struct SpecWorkspaceState {
    /// Immutable spec data resolved once at startup.
    pub resolved: Rc<ResolvedWorkspace>,
    /// Plain reducer state owned by the Dioxus host.
    pub snapshot: Signal<Snapshot<SpecPanelId>>,
    /// Reusable caller-owned projection scratch.
    pub scratch: Rc<RefCell<ProjectionBuffer<SpecPanelId>>>,
    store: Rc<LocalStorageLayoutStore>,
}

/// Decode, validate, resolve, restore, and prepare a spec-authored workspace.
pub fn use_spec_workspace(spec_json: &'static str) -> SpecWorkspaceState {
    let resolved = use_hook(move || Rc::new(resolve_workspace_spec(spec_json)));
    let store = use_hook({
        let key = resolved.persistence.key.clone();
        move || Rc::new(LocalStorageLayoutStore::new(key.clone()))
    });
    let snapshot = use_signal({
        let resolved = resolved.clone();
        let store = store.clone();
        move || restore_or_default(&resolved, &store)
    });
    let scratch = use_hook({
        let panel_count = snapshot.peek().panels.len();
        move || Rc::new(RefCell::new(ProjectionBuffer::with_panel_capacity(panel_count)))
    });

    SpecWorkspaceState { resolved, snapshot, scratch, store }
}

/// Subscribe the host-owned workspace to browser viewport changes.
pub fn mount_viewport_observer(workspace: &SpecWorkspaceState) {
    let emit = workspace_event_handler(workspace);
    let policy = workspace.resolved.surface.resize_policy;
    let _status = observe_viewport(EventHandler::new(move |size: Viewport| {
        emit.call(WorkspaceEvent::ViewportChanged { size, policy });
    }));
}

/// Project the current snapshot into caller-owned scratch for one render pass.
pub fn project_workspace<'frame>(
    workspace: &SpecWorkspaceState,
    snapshot: &Snapshot<SpecPanelId>,
    scratch: &'frame mut ProjectionBuffer<SpecPanelId>,
) -> ProjectedFrame<'frame, SpecPanelId> {
    let surface = surface_profile_for(workspace, snapshot.viewport.width);
    let chrome = chrome_projection(&workspace.resolved);
    let tile = tile_metrics(&workspace.resolved, surface);

    project_into(
        ProjectionInput { snapshot, surface, chrome: &chrome, clamp: &workspace.resolved.layout.clamp, tile: &tile },
        scratch,
    )
}

/// CSS class for the `.ws` area that contains projected panels.
pub fn workspace_area_class(frame: &ProjectedFrame<'_, SpecPanelId>) -> &'static str {
    if frame.panels.iter().any(|panel| matches!(panel.placement, Placement::Maximized)) {
        "ws maxed"
    } else if frame.mode == Mode::Tiling {
        "ws tiling"
    } else {
        "ws floating"
    }
}

/// Build an event handler that reduces events and applies the spec save policy.
pub fn workspace_event_handler(
    workspace: &SpecWorkspaceState,
) -> EventHandler<WorkspaceEvent<SpecPanelId>> {
    let workspace = workspace.clone();
    EventHandler::new(move |event| {
        reduce_workspace_event(&workspace, event);
    })
}

/// Reduce a root keyboard event after the example has declined first refusal.
pub fn handle_key(workspace: &SpecWorkspaceState, event: &KeyboardEvent) {
    let focus = if panel_kit::input::is_editing() {
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

/// Reduce a root pointer-move event for in-flight workspace gestures.
pub fn handle_pointer_move(workspace: &SpecWorkspaceState, event: &DioxusPointerEvent) {
    let kind = if workspace.snapshot.read().drag.is_some() {
        PointerEventKind::Drag(PointerButton::Primary)
    } else {
        PointerEventKind::Moved
    };
    reduce_workspace_event(workspace, pointer_event(HitTarget::Workspace, event, kind));
}

/// Settle a pointer gesture and let the spec save policy persist settled layout.
pub fn handle_pointer_up(workspace: &SpecWorkspaceState, event: &DioxusPointerEvent) {
    release_pointer(event);
    let changed = reduce_workspace_event(
        workspace,
        pointer_event(HitTarget::Workspace, event, PointerEventKind::Up(PointerButton::Primary)),
    );
    if changed {
        clear_selection();
    }
}

/// Reduce a root wheel event after native panel body scroll gets first refusal.
pub fn handle_wheel(workspace: &SpecWorkspaceState, event: &WheelEvent) {
    if reduce_workspace_event(workspace, wheel_event(event)) {
        event.prevent_default();
    }
}

/// Reset to authored defaults and clear persisted operator state without re-saving.
pub fn reset_workspace(workspace: &SpecWorkspaceState) {
    if let Err(error) = apply_save_decision(
        workspace.resolved.persistence.save_policy.reset_decision(),
        &*workspace.store,
        &workspace.resolved.initial,
        &workspace.resolved.catalog,
    ) {
        log_layout_error("clear layout", &workspace.resolved.persistence.key, &error);
    }

    let mut snapshot = workspace.snapshot;
    snapshot.set(workspace.resolved.initial.clone());
}

fn resolve_workspace_spec(spec_json: &str) -> ResolvedWorkspace {
    let spec = WorkspaceSpec::from_json_str(spec_json)
        .expect("PANEL_KIT_WORKSPACE_SPEC must contain strict WorkspaceSpec JSON");
    let providers = BindingManifest {
        backend: BackendKind::Web,
        panels: spec.panels.iter().map(|panel| panel.provider_declaration()).collect(),
    };

    spec.resolve(&providers)
        .expect("web canary providers must match the authored WorkspaceSpec")
}

fn restore_or_default(
    resolved: &ResolvedWorkspace,
    store: &LocalStorageLayoutStore,
) -> Snapshot<SpecPanelId> {
    if !resolved.persistence.restore {
        return resolved.initial.clone();
    }

    let context = RestoreContext {
        units: resolved.initial.viewport.units,
        viewport: (resolved.initial.viewport.width, resolved.initial.viewport.height),
    };

    match restore_snapshot(store, resolved.initial.clone(), &resolved.catalog, context) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            log_layout_error("restore layout", &resolved.persistence.key, &error);
            resolved.initial.clone()
        }
    }
}

fn reduce_workspace_event(
    workspace: &SpecWorkspaceState,
    event: WorkspaceEvent<SpecPanelId>,
) -> bool {
    let mut snapshot_signal = workspace.snapshot;
    let mut snapshot = snapshot_signal.write();
    let surface = surface_profile_for(workspace, snapshot.viewport.width);
    let context = panel_kit_core::reducer::ReduceContext {
        surface,
        clamp: &workspace.resolved.layout.clamp,
        command_step: workspace.resolved.input.steps,
        tile: &workspace.resolved.layout.tile.resize,
    };
    let reduction = reduce(&mut snapshot, event, context);
    let decision = workspace.resolved.persistence.save_policy.decide(&reduction);
    let changed = reduction.changed;

    if let Err(error) = apply_save_decision(decision, &*workspace.store, &snapshot, &workspace.resolved.catalog) {
        log_layout_error("save layout", &workspace.resolved.persistence.key, &error);
    }

    changed
}

fn surface_profile_for(workspace: &SpecWorkspaceState, width: f64) -> SurfaceProfile {
    SurfaceProfile::from_logical_width(
        width,
        workspace.resolved.surface.compact_max,
        workspace.resolved.surface.tablet_max,
        browser_capabilities(),
    )
}

fn chrome_projection(resolved: &ResolvedWorkspace) -> ChromeProjectionInput {
    let chrome = &resolved.chrome;
    ChromeProjectionInput {
        metrics: chrome.metrics,
        hit_target_min: chrome.hit_target_min,
        panel_header_h: chrome.panel_header_h,
        panel_frame: chrome.panel_frame,
        title_in_border: chrome.title_in_border,
        mode_control: chrome.mode_control,
        minimize_control: chrome.minimize_control,
        maximize_control: chrome.maximize_control,
        resize_grip: chrome.resize_grip,
        dock: chrome.dock,
    }
}

fn tile_metrics(resolved: &ResolvedWorkspace, surface: SurfaceProfile) -> TileLayoutMetrics {
    let tile = &resolved.layout.tile;
    TileLayoutMetrics {
        resize: tile.resize,
        columns: surface.tile_columns(),
        row_min: tile.row_min,
        gap: tile.gap,
        padding: tile.padding,
        fill_viewport: tile.fill_viewport,
    }
}

fn log_layout_error(action: &str, storage_key: &str, error: &LayoutError) {
    let message = format!("panel-kit {action} failed for storage key `{storage_key}`: {error}");

    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&message));

    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{message}");
}
