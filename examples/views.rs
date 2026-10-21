//! Views demo — host-owned named layouts over a composable workspace.
//!
//! Run with: `dx serve --example views --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`).
//!
//! What it demonstrates:
//! - app-owned [`SavedViews`] registry state over the core key scheme,
//! - one [`LocalStorageLayoutStore`] per view (`{base}:view:{name}`),
//! - explicit [`SavePolicy`] writes after reducer events, and
//! - web composition parts (`root`, `panel`, `dock`) instead of a controller.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::events::{KeyboardEvent, PointerEvent as DioxusPointerEvent};
use dioxus::prelude::*;
use gloo_storage::{LocalStorage, Storage};
use panel_kit::input::{clear_selection, keyboard_event, pointer_event, wheel_event};
use panel_kit::store::LocalStorageLayoutStore;
use panel_kit::surface::{observe_viewport, surface_profile, viewport_size};
use panel_kit::widgets::{dock, panel, root};
use panel_kit::{LayoutBuilder, PanelKind, PanelWin, SavedLayout, StoredLayout, CSS};
use panel_kit_core::frame::{
    project_into, ChromeProjectionInput, ProjectionBuffer, ProjectionInput, TileLayoutMetrics,
};
use panel_kit_core::persist::{
    apply_save_decision, persist_snapshot, restore_snapshot, LayoutError, RestoreContext,
    SavePolicy,
};
use panel_kit_core::reducer::{
    reduce, HitTarget, ResizePolicy, Snapshot, Viewport, WorkspaceEvent,
};
use panel_kit_core::views::{view_layout_key, views_registry_key, SavedViews};
use panel_kit_core::{
    ChromeMetrics, Clamp, CommandStep, FocusContext, Mode, PanelCatalog, PointerButton,
    PointerEventKind, TileMetrics, Units,
};
use serde::{Deserialize, Serialize};

/// Base localStorage key: registry at `{BASE_KEY}:views`, layouts at `{BASE_KEY}:view:<name>`.
const BASE_KEY: &str = "panel_kit_example_views";

/// Demo styles layered after `panel_kit::CSS`; the switcher is app-owned UI.
const DEMO_CSS: &str = "
.topbar button { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.topbar button:hover { border-color: var(--fg); }
.topbar button.active-view { background: var(--inv-bg); color: var(--inv-fg); }
.topbar input { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .4rem; font-size: .72rem; font-family: var(--mono);
  width: 7rem; }
.topbar .err { color: var(--red); font-size: .72rem; }
.status-list { margin: .25rem 0; padding-left: 1.1rem; }
.status-list li { color: var(--dim); }
textarea.notes { width: 100%; height: 70%; background: var(--bg); color: var(--fg);
  border: 1px solid var(--line2); border-radius: 3px; font-family: var(--mono);
  font-size: .78rem; padding: .4rem; resize: none; }
";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Panel {
    Notes,
    Preview,
    Status,
}

impl PanelKind for Panel {
    fn title(self) -> &'static str {
        match self {
            Self::Notes => "Notes",
            Self::Preview => "Preview",
            Self::Status => "Status",
        }
    }
}

/// Default layout for a view's first activation and for panels added later.
fn default_layout() -> Vec<PanelWin<Panel>> {
    let mut builder = LayoutBuilder::new();
    vec![
        builder.at(Panel::Notes, 16.0, 16.0, 420.0, 300.0),
        builder.at(Panel::Preview, 452.0, 16.0, 420.0, 300.0),
        builder.at(Panel::Status, 16.0, 332.0, 560.0, 260.0),
    ]
}

/// Catalog used by persistence and dock/chrome parts.
fn panel_catalog() -> PanelCatalog<Panel> {
    PanelCatalog::from_panel_kind_layout(&default_layout()).expect("view panels have stable ids")
}

/// Current browser viewport expressed in core reducer units.
fn current_viewport() -> Viewport {
    let (width, height) = viewport_size();
    Viewport {
        width,
        height,
        units: Units::CssPx,
    }
}

/// Load the registry through the core shape and sanitize invalid stored values.
fn load_registry() -> SavedViews {
    LocalStorage::get::<SavedViews>(views_registry_key(BASE_KEY))
        .map(SavedViews::sanitize)
        .unwrap_or_else(|_| SavedViews::new(&["Main", "Focus"]))
}

/// Persist the registry under the core-owned `{base}:views` key.
fn save_registry(registry: &SavedViews) -> Result<(), String> {
    let key = views_registry_key(BASE_KEY);
    LocalStorage::set(key.as_str(), registry)
        .map_err(|error| format_storage_error("save view registry", &key, error))
}

/// Preserve the legacy first-run copy from the old bare layout key.
fn migrate_legacy_layout(registry: &SavedViews) -> Result<(), String> {
    let storage = LocalStorage::raw();
    let Some(first) = registry.views.first() else {
        return Ok(());
    };
    let dest = view_layout_key(BASE_KEY, first);
    let dest_exists = storage
        .get_item(&dest)
        .map_err(|error| format_storage_error("read first view layout", &dest, error))?
        .is_some();
    if dest_exists {
        return Ok(());
    }

    let Some(raw) = storage
        .get_item(BASE_KEY)
        .map_err(|error| format_storage_error("read legacy layout", BASE_KEY, error))?
    else {
        return Ok(());
    };
    storage
        .set_item(&dest, &raw)
        .map_err(|error| format_storage_error("copy legacy layout into first view", &dest, error))
}

/// Restore one view's snapshot or fall back to defaults while logging failures.
fn restore_view_snapshot(
    name: &str,
    catalog: &PanelCatalog<Panel>,
    viewport: Viewport,
) -> Snapshot<Panel> {
    let defaults = Snapshot::from_defaults(default_layout(), Mode::Floating, viewport);
    let key = view_layout_key(BASE_KEY, name);
    let store = LocalStorageLayoutStore::new(key.as_str());
    restore_snapshot(&store, defaults.clone(), catalog, restore_context(viewport)).unwrap_or_else(
        |error| {
            console_layout_error("restore view layout", &key, &error);
            defaults
        },
    )
}

/// Persist one snapshot into a view's layout record.
fn persist_view_snapshot(
    name: &str,
    snapshot: &Snapshot<Panel>,
    catalog: &PanelCatalog<Panel>,
) -> Result<(), String> {
    let key = view_layout_key(BASE_KEY, name);
    let store = LocalStorageLayoutStore::new(key.as_str());
    persist_snapshot(&store, snapshot, catalog)
        .map_err(|error| format_layout_error("save view layout", &key, &error))
}

/// Restore context shared by every per-view layout record.
fn restore_context(viewport: Viewport) -> RestoreContext {
    RestoreContext {
        units: Units::CssPx,
        viewport: (viewport.width, viewport.height),
    }
}

/// Human-readable layout failure message shared by UI and console reporting.
fn format_layout_error(action: &str, key: &str, error: &LayoutError) -> String {
    format!("panel-kit {action} failed for storage key `{key}`: {error}")
}

/// Browser console reporting for layout persistence failures.
fn console_layout_error(action: &str, key: &str, error: &LayoutError) {
    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&format_layout_error(
        action, key, error,
    )));
}

/// Human-readable storage failure message for user-visible raw localStorage failures.
fn format_storage_error(action: &str, key: &str, error: impl std::fmt::Debug) -> String {
    format!("panel-kit {action} failed for storage key `{key}`: {error:?}")
}

/// Convert a layout persistence failure into the result shape consumed by [`report`].
fn layout_failure(action: &str, key: &str, error: &LayoutError) -> Result<(), String> {
    Err(format_layout_error(action, key, error))
}

/// Convert a raw localStorage failure into the result shape consumed by [`report`].
fn storage_failure(action: &str, key: &str, error: impl std::fmt::Debug) -> Result<(), String> {
    Err(format_storage_error(action, key, error))
}

/// Human-readable schema label for a view's localStorage record.
fn stored_schema(key: &str) -> &'static str {
    match LocalStorage::get::<StoredLayout<Panel>>(key) {
        Ok(StoredLayout::V2(_)) => "V2",
        Ok(StoredLayout::V1(_)) => "V1",
        Err(_) => "not stored yet",
    }
}

/// Apply a reducer event, then run the explicit OnSettle persistence policy.
fn reduce_and_persist(
    event: WorkspaceEvent<Panel>,
    mut snapshot: Signal<Snapshot<Panel>>,
    registry: Signal<SavedViews>,
    catalog: Rc<PanelCatalog<Panel>>,
    err: Signal<String>,
) -> bool {
    let mut next = snapshot.read().clone();
    let context = reduce_context(&next);
    let reduction = reduce(&mut next, event, context);
    if !reduction.changed {
        return false;
    }

    let active = registry.read().active.clone();
    let key = view_layout_key(BASE_KEY, &active);
    let store = LocalStorageLayoutStore::new(key.as_str());
    let decision = SavePolicy::OnSettle.decide(&reduction);
    if let Err(error) = apply_save_decision(decision, &store, &next, &catalog) {
        report(err, layout_failure("save view layout", &key, &error));
    }
    snapshot.set(next);
    true
}

/// Reducer context for web CSS-pixel workspaces.
fn reduce_context(snapshot: &Snapshot<Panel>) -> panel_kit_core::reducer::ReduceContext<'static> {
    panel_kit_core::reducer::ReduceContext {
        surface: surface_profile(snapshot.viewport.width),
        clamp: &Clamp::WEB,
        command_step: CommandStep::WEB,
        tile: &TileMetrics::WEB,
        snap: panel_kit_core::SnapPolicy::default(),
    }
}

/// Switch active views by flushing outgoing state and restoring incoming state.
fn switch_view(
    name: &str,
    mut registry: Signal<SavedViews>,
    mut snapshot: Signal<Snapshot<Panel>>,
    catalog: Rc<PanelCatalog<Panel>>,
) -> Result<(), String> {
    let mut next_registry = registry.read().clone();
    if next_registry.active == name {
        return Ok(());
    }
    let outgoing = next_registry.active.clone();
    next_registry
        .activate(name)
        .map_err(|error| error.to_string())?;
    persist_view_snapshot(&outgoing, &snapshot.read(), &catalog)?;
    save_registry(&next_registry)?;
    let viewport = snapshot.read().viewport;
    snapshot.set(restore_view_snapshot(
        &next_registry.active,
        &catalog,
        viewport,
    ));
    registry.set(next_registry);
    Ok(())
}

/// Workspace class for the projected panel region.
fn workspace_class(frame: &panel_kit_core::frame::ProjectedFrame<'_, Panel>) -> &'static str {
    if frame
        .panels
        .iter()
        .any(|panel| panel.state == panel_kit_core::WinState::Maximized)
    {
        "ws maxed"
    } else if frame.mode == Mode::Tiling {
        "ws tiling"
    } else {
        "ws floating"
    }
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let catalog = use_hook(|| Rc::new(panel_catalog()));
    let initial_state = use_hook(|| {
        let registry = load_registry();
        let migration_error = migrate_legacy_layout(&registry).err();
        (registry, migration_error)
    });
    let registry = use_signal(|| initial_state.0.clone());
    let snapshot = use_signal({
        let catalog = catalog.clone();
        let initial_active = initial_state.0.active.clone();
        move || restore_view_snapshot(&initial_active, &catalog, current_viewport())
    });
    let scratch = use_hook(|| {
        Rc::new(RefCell::new(
            ProjectionBuffer::<Panel>::with_panel_capacity(3),
        ))
    });
    let mut draft = use_signal(String::new);
    let err = use_signal(|| initial_state.1.clone().unwrap_or_default());
    let migration_observation = use_signal(String::new);

    let emit = EventHandler::new({
        let catalog = catalog.clone();
        move |event| {
            reduce_and_persist(event, snapshot, registry, catalog.clone(), err);
        }
    });
    observe_viewport(EventHandler::new({
        let catalog = catalog.clone();
        move |viewport| {
            reduce_and_persist(
                WorkspaceEvent::ViewportChanged {
                    size: viewport,
                    policy: ResizePolicy::ScaleFloating,
                },
                snapshot,
                registry,
                catalog.clone(),
                err,
            );
        }
    }));

    let registry_now = registry.read().clone();
    let active_key = view_layout_key(BASE_KEY, &registry_now.active);
    let active_schema = stored_schema(&active_key);
    let snapshot_now = snapshot.read();
    let surface = surface_profile(snapshot_now.viewport.width);
    let chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
    let tile = TileLayoutMetrics::from_tile_metrics(TileMetrics::WEB, surface);
    let mut scratch_ref = scratch.borrow_mut();
    let frame = project_into(
        ProjectionInput {
            snapshot: &snapshot_now,
            surface,
            chrome: &chrome,
            clamp: &Clamp::WEB,
            tile: &tile,
        },
        &mut scratch_ref,
    );
    let root_class = root::root_class(&frame);
    let ws_class = workspace_class(&frame);
    let surface_label = surface_label(frame.surface.class);
    let ws_style = frame
        .tile_grid
        .map(root::tile_grid_style)
        .unwrap_or_default();
    let key_catalog = catalog.clone();
    let create_catalog = catalog.clone();
    let delete_catalog = catalog.clone();
    let reset_catalog = catalog.clone();
    let migrate_catalog = catalog.clone();

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        div {
            class: "{root_class}",
            tabindex: "0",
            onpointermove: move |event: DioxusPointerEvent| {
                emit.call(pointer_event(HitTarget::Workspace, &event, PointerEventKind::Drag(PointerButton::Primary)));
            },
            onpointerup: move |event: DioxusPointerEvent| {
                clear_selection();
                emit.call(pointer_event(HitTarget::Workspace, &event, PointerEventKind::Up(PointerButton::Primary)));
            },
            onpointercancel: move |event: DioxusPointerEvent| {
                clear_selection();
                emit.call(pointer_event(HitTarget::Workspace, &event, PointerEventKind::Up(PointerButton::Primary)));
            },
            onkeydown: move |event: KeyboardEvent| {
                let focus = keyboard_focus(&snapshot.read());
                if let Some(workspace_event) = keyboard_event(&event, focus) {
                    if reduce_and_persist(workspace_event, snapshot, registry, key_catalog.clone(), err) {
                        event.prevent_default();
                    }
                }
            },
            header { class: "topbar",
                h1 { "panel-kit views demo" }
                span { class: "hint", "surface: {surface_label} · active key: {active_key} [{active_schema}]" }
                for name in registry_now.views.iter().cloned() {
                    {
                        let switch_catalog = catalog.clone();
                        rsx! {
                            button {
                                key: "{name}",
                                class: if name == registry_now.active { "active-view" } else { "" },
                                onclick: move |_| report(err, switch_view(&name, registry, snapshot, switch_catalog.clone())),
                                "{name}"
                            }
                        }
                    }
                }
                input { placeholder: "view name", value: "{draft}", oninput: move |event| draft.set(event.value()) }
                button { onclick: move |_| create_view(draft, err, registry, snapshot, create_catalog.clone()), "+ view" }
                button { onclick: move |_| rename_active(draft, err, registry), "rename active" }
                button { onclick: move |_| delete_active(err, registry, snapshot, delete_catalog.clone()), "delete active" }
                button { onclick: move |_| reset_active_view(err, registry, snapshot, reset_catalog.clone()), "reset view" }
                button { onclick: move |_| seed_v1_view(registry, snapshot, migrate_catalog.clone(), err, migration_observation), "migrate a V1 record" }
                if !err.read().is_empty() { span { class: "err", "{err}" } }
            }
            div { class: "{ws_class}", style: "{ws_style}", onwheel: move |event| emit.call(wheel_event(&event)),
                for projected in frame.panels.iter().copied() {
                    {render_panel(projected, &catalog, emit, &registry_now, &migration_observation)}
                }
            }
            {dock::dock(frame.dock, &catalog, emit, None)}
        }
    }
}

/// Focus context for a web keyboard event.
fn keyboard_focus(snapshot: &Snapshot<Panel>) -> FocusContext<Panel> {
    if panel_kit::input::is_editing() {
        FocusContext::TextInput
    } else if let Some(panel) = snapshot.focused {
        FocusContext::Panel(panel)
    } else {
        FocusContext::Workspace
    }
}

/// Human label for the current responsive surface tier.
fn surface_label(class: panel_kit_core::SurfaceClass) -> &'static str {
    match class {
        panel_kit_core::SurfaceClass::Compact => "compact",
        panel_kit_core::SurfaceClass::Tablet => "tablet",
        panel_kit_core::SurfaceClass::Regular => "regular",
    }
}

/// Render one projected panel from independent web composition parts.
fn render_panel(
    projected: panel_kit_core::frame::PanelProjection<Panel>,
    catalog: &PanelCatalog<Panel>,
    emit: EventHandler<WorkspaceEvent<Panel>>,
    registry: &SavedViews,
    migration_observation: &Signal<String>,
) -> Element {
    let Some(meta) = catalog.get(projected.key) else {
        return rsx! {};
    };
    let class = format!("panel-{}", meta.slug);
    let controls = panel::traffic_lights(projected, emit);
    let chrome = panel::panel_chrome_with_controls(projected, meta, Some(controls), None);
    let body = panel::panel_body(panel_body(projected.key, registry, migration_observation));
    let resize = panel::resize_grip(projected, emit);
    panel::panel_shell(projected, Some(&class), rsx! { {chrome} {body} {resize} })
}

/// Render app-owned panel bodies; the library owns only the panel surfaces.
fn panel_body(
    panel: Panel,
    registry: &SavedViews,
    migration_observation: &Signal<String>,
) -> Element {
    match panel {
        Panel::Notes => rsx! {
            p { "Arrange this view, then switch away and back — every view keeps its own V2 layout." }
            textarea { class: "notes", placeholder: "per-view notes…" }
        },
        Panel::Preview => rsx! {
            p { "The app owns view state and calls reducer/projection/parts directly; no view controller is mounted." }
            p { "Each switch persists the outgoing snapshot before restoring the incoming view's store." }
        },
        Panel::Status => status_body(registry, migration_observation),
    }
}

/// Render status rows exposing the storage-key contract.
fn status_body(registry: &SavedViews, migration_observation: &Signal<String>) -> Element {
    let rows: Vec<String> = registry
        .views
        .iter()
        .map(|name| {
            let marker = if *name == registry.active {
                " (active)"
            } else {
                ""
            };
            let key = view_layout_key(BASE_KEY, name);
            format!("{name} → {key} [{}]{marker}", stored_schema(&key))
        })
        .collect();
    rsx! {
        p { "registry: " code { "{BASE_KEY}:views" } }
        p { "active storage key: " code { "{view_layout_key(BASE_KEY, &registry.active)}" } }
        ul { class: "status-list", for row in rows { li { "{row}" } } }
        p { "A pre-views layout at the bare " code { "{BASE_KEY}" } " key is copied into the first view and never deleted." }
        if !migration_observation.read().is_empty() { p { "{migration_observation}" } }
    }
}

/// Report a view-registry action into the demo error signal.
fn report<E: std::fmt::Display>(mut err: Signal<String>, result: Result<(), E>) {
    match result {
        Ok(()) => err.set(String::new()),
        Err(error) => err.set(error.to_string()),
    }
}

/// Create then activate a view, materializing layout from defaults on first load.
fn create_view(
    mut draft: Signal<String>,
    err: Signal<String>,
    mut registry: Signal<SavedViews>,
    snapshot: Signal<Snapshot<Panel>>,
    catalog: Rc<PanelCatalog<Panel>>,
) {
    let name = draft.read().clone();
    let mut next = registry.read().clone();
    match next.add(&name) {
        Ok(()) => match save_registry(&next) {
            Ok(()) => {
                registry.set(next);
                report(err, switch_view(&name, registry, snapshot, catalog));
                draft.set(String::new());
            }
            Err(error) => report(err, Err(error)),
        },
        Err(error) => report(err, Err(error)),
    }
}

/// Rename the active view and move its raw layout record without clobbering.
fn rename_active(mut draft: Signal<String>, err: Signal<String>, mut registry: Signal<SavedViews>) {
    let old = registry.read().active.clone();
    let new = draft.read().trim().to_string();
    let mut next = registry.read().clone();
    match next.rename(&old, &new) {
        Ok(()) => match move_layout_record(&old, &new).and_then(|()| save_registry(&next)) {
            Ok(()) => {
                registry.set(next);
                draft.set(String::new());
                report(err, Ok::<(), String>(()));
            }
            Err(error) => report(err, Err(error)),
        },
        Err(error) => report(err, Err(error)),
    }
}

/// Copy then remove a renamed view layout record, matching the old no-clobber behavior.
fn move_layout_record(old: &str, new: &str) -> Result<(), String> {
    let storage = LocalStorage::raw();
    let old_key = view_layout_key(BASE_KEY, old);
    let new_key = view_layout_key(BASE_KEY, new);
    if old_key == new_key {
        return Ok(());
    }

    let Some(raw) = storage
        .get_item(&old_key)
        .map_err(|error| format_storage_error("read renamed view layout", &old_key, error))?
    else {
        return Ok(());
    };
    let new_exists = storage
        .get_item(&new_key)
        .map_err(|error| format_storage_error("check renamed view layout", &new_key, error))?
        .is_some();
    if !new_exists {
        storage
            .set_item(&new_key, &raw)
            .map_err(|error| format_storage_error("write renamed view layout", &new_key, error))?;
    }
    storage
        .remove_item(&old_key)
        .map_err(|error| format_storage_error("remove old view layout", &old_key, error))
}

/// Delete the active view and restore the neighbor selected by the registry.
fn delete_active(
    err: Signal<String>,
    mut registry: Signal<SavedViews>,
    mut snapshot: Signal<Snapshot<Panel>>,
    catalog: Rc<PanelCatalog<Panel>>,
) {
    let removed = registry.read().active.clone();
    let mut next = registry.read().clone();
    match next.remove(&removed) {
        Ok(()) => match remove_layout_record(&removed).and_then(|()| save_registry(&next)) {
            Ok(()) => {
                let viewport = snapshot.read().viewport;
                snapshot.set(restore_view_snapshot(&next.active, &catalog, viewport));
                registry.set(next);
                report(err, Ok::<(), String>(()));
            }
            Err(error) => report(err, Err(error)),
        },
        Err(error) => report(err, Err(error)),
    }
}

/// Remove a deleted view's layout record from localStorage.
fn remove_layout_record(view: &str) -> Result<(), String> {
    let storage = LocalStorage::raw();
    let key = view_layout_key(BASE_KEY, view);
    storage
        .remove_item(&key)
        .map_err(|error| format_storage_error("remove deleted view layout", &key, error))
}

/// Clear the active view's store and reset the host snapshot without an immediate save.
fn reset_active_view(
    err: Signal<String>,
    registry: Signal<SavedViews>,
    mut snapshot: Signal<Snapshot<Panel>>,
    catalog: Rc<PanelCatalog<Panel>>,
) {
    let viewport = snapshot.read().viewport;
    let next = Snapshot::from_defaults(default_layout(), Mode::Floating, viewport);
    let active = registry.read().active.clone();
    let key = view_layout_key(BASE_KEY, &active);
    let store = LocalStorageLayoutStore::new(key.as_str());
    let decision = SavePolicy::OnSettle.reset_decision();
    if let Err(error) = apply_save_decision(decision, &store, &next, &catalog) {
        report(err, layout_failure("reset view layout", &key, &error));
    }
    snapshot.set(next);
}

/// Seed a V1 record, then switch through the production reader to observe V2.
fn seed_v1_view(
    registry: Signal<SavedViews>,
    snapshot: Signal<Snapshot<Panel>>,
    catalog: Rc<PanelCatalog<Panel>>,
    err: Signal<String>,
    mut observation: Signal<String>,
) {
    let name = next_migration_name(&registry.read().views);
    create_raw_view(&name, registry, err, snapshot, catalog.clone());
    let key = view_layout_key(BASE_KEY, &name);
    let legacy = SavedLayout {
        panels: default_layout(),
        tiling: true,
    };
    if let Err(error) = LocalStorage::set(&key, legacy) {
        report(err, storage_failure("seed V1 view layout", &key, error));
        return;
    }
    observation.set(format!(
        "Seeded V1 at {key}; switching through the production reader…"
    ));
    report(err, switch_view(&name, registry, snapshot, catalog));
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(75).await;
        observation.set(format!(
            "After read and settle: {key} is observed as {}.",
            stored_schema(&key)
        ));
    });
}

/// Add one raw registry entry for the migration demo.
fn create_raw_view(
    name: &str,
    mut registry: Signal<SavedViews>,
    mut err: Signal<String>,
    _snapshot: Signal<Snapshot<Panel>>,
    _catalog: Rc<PanelCatalog<Panel>>,
) {
    let mut next = registry.read().clone();
    match next.add(name) {
        Ok(()) => match save_registry(&next) {
            Ok(()) => {
                registry.set(next);
                err.set(String::new());
            }
            Err(error) => err.set(error),
        },
        Err(error) => err.set(error.to_string()),
    }
}

/// Pick a unique migration-demo view name.
fn next_migration_name(existing: &[String]) -> String {
    let mut suffix = 1_u32;
    loop {
        let candidate = format!("V1 migration {suffix}");
        if !existing.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_keys_use_the_core_scheme() {
        let registry = SavedViews::new(&["Main", "Focus"]);

        assert_eq!(
            views_registry_key(BASE_KEY),
            "panel_kit_example_views:views"
        );
        assert_eq!(
            view_layout_key(BASE_KEY, &registry.active),
            "panel_kit_example_views:view:Main"
        );
    }

    #[test]
    fn reset_snapshot_keeps_the_existing_viewport() {
        let viewport = Viewport {
            width: 900.0,
            height: 700.0,
            units: Units::CssPx,
        };
        let snapshot = Snapshot::from_defaults(default_layout(), Mode::Floating, viewport);

        assert_eq!(snapshot.viewport, viewport);
        assert_eq!(snapshot.panels.len(), 3);
    }

    #[test]
    fn storage_errors_name_the_action_and_key() {
        let message = format_storage_error("save view registry", "demo:views", "quota exceeded");

        assert!(message.contains("save view registry"));
        assert!(message.contains("demo:views"));
        assert!(message.contains("quota exceeded"));
    }

    #[test]
    fn layout_errors_name_the_action_key_and_cause() {
        let error = LayoutError::Store("quota exceeded".to_string());
        let message = format_layout_error("save view layout", "demo:view:Main", &error);

        assert!(message.contains("save view layout"));
        assert!(message.contains("demo:view:Main"));
        assert!(message.contains("layout store failed: quota exceeded"));
    }

    #[test]
    fn layout_failures_are_ready_for_the_visible_report_path() {
        let error = LayoutError::Store("quota exceeded".to_string());
        let message = layout_failure("reset view layout", "demo:view:Main", &error)
            .expect_err("layout failures must report through the error signal");

        assert!(message.contains("reset view layout"));
        assert!(message.contains("demo:view:Main"));
        assert!(message.contains("layout store failed: quota exceeded"));
    }

    #[test]
    fn storage_failures_are_ready_for_the_visible_report_path() {
        let message = storage_failure(
            "seed V1 view layout",
            "demo:view:Migration",
            "quota exceeded",
        )
        .expect_err("storage failures must report through the error signal");

        assert!(message.contains("seed V1 view layout"));
        assert!(message.contains("demo:view:Migration"));
        assert!(message.contains("quota exceeded"));
    }
}
