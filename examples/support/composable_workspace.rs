//! Shared host-owned workspace wiring for web examples.
//!
//! This module is example support, not a public controller facade. Each example
//! still owns its render loop and composes panel parts directly; this file keeps
//! repeated persistence/projection setup identical across the canaries.
#[path = "workspace_events.rs"]
mod workspace_events;

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit::store::LocalStorageLayoutStore;
use panel_kit::surface::{observe_viewport, surface_profile, viewport_size};
use panel_kit_core::frame::{
    project_into, ChromeProjectionInput, Placement, ProjectedFrame, ProjectionBuffer,
    ProjectionInput, TileLayoutMetrics,
};
use panel_kit_core::persist::{restore_snapshot, LayoutError, RestoreContext, SavePolicy};
use panel_kit_core::reducer::{ResizePolicy, Snapshot, Viewport, WorkspaceEvent};
use panel_kit_core::{
    ChromeMetrics, Clamp, Mode, PanelCatalog, PanelKind, PanelWin, TileMetrics, Units,
};

pub use workspace_events::{
    handle_key, handle_pointer_move, handle_pointer_up, handle_wheel, workspace_event_handler,
};

/// Host-owned workspace state and ports shared by one example instance.
#[derive(Clone)]
pub struct DemoPanelState<K: PanelKind> {
    /// Browser localStorage key documented by the example.
    pub storage_key: &'static str,
    /// The explicit persistence policy this example applies after reductions.
    pub save_policy: SavePolicy,
    /// Plain reducer state owned by the Dioxus host.
    pub snapshot: Signal<Snapshot<K>>,
    /// Stable ID catalog used by persistence and dock/chrome titles.
    pub catalog: Rc<PanelCatalog<K>>,
    /// Reusable caller-owned projection scratch.
    pub scratch: Rc<RefCell<ProjectionBuffer<K>>>,
    /// Browser storage transport kept private to the host-owned reducer loop.
    pub(super) store: Rc<LocalStorageLayoutStore>,
}

/// Initialize a host-owned workspace from defaults and one layout store.
pub fn use_demo_workspace<K: PanelKind>(
    storage_key: &'static str,
    defaults: fn() -> Vec<PanelWin<K>>,
    save_policy: SavePolicy,
) -> DemoPanelState<K> {
    let initial_viewport = initial_viewport();
    let default_snapshot = Snapshot::from_defaults(defaults(), Mode::Floating, initial_viewport);
    let catalog_panels = default_snapshot.panels.clone();
    let catalog = use_hook(move || {
        Rc::new(
            PanelCatalog::from_panel_kind_layout(&catalog_panels)
                .expect("example panel enum must serialize as stable string IDs"),
        )
    });
    let store = use_hook(move || Rc::new(LocalStorageLayoutStore::new(storage_key)));
    let snapshot = use_signal({
        let catalog = catalog.clone();
        let store = store.clone();
        let defaults = default_snapshot.clone();
        move || restore_or_default(storage_key, &store, defaults, &catalog)
    });
    let scratch = use_hook({
        let panel_count = snapshot.peek().panels.len();
        move || Rc::new(RefCell::new(ProjectionBuffer::with_panel_capacity(panel_count)))
    });

    DemoPanelState { storage_key, save_policy, snapshot, catalog, scratch, store }
}

/// Subscribe the host-owned workspace to browser viewport changes.
pub fn mount_viewport_observer<K: PanelKind>(workspace: &DemoPanelState<K>) {
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
    snapshot: &Snapshot<K>,
    scratch: &'frame mut ProjectionBuffer<K>,
) -> ProjectedFrame<'frame, K> {
    let surface = surface_profile(snapshot.viewport.width);
    let chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
    let tile = TileLayoutMetrics::from_tile_metrics(TileMetrics::WEB, surface);

    project_into(
        ProjectionInput { snapshot, surface, chrome: &chrome, clamp: &Clamp::WEB, tile: &tile },
        scratch,
    )
}

/// CSS class for the `.ws` area that contains projected panels.
pub fn workspace_area_class<K: PanelKind>(frame: &ProjectedFrame<'_, K>) -> &'static str {
    if frame.panels.iter().any(|panel| matches!(panel.placement, Placement::Maximized)) {
        "ws maxed"
    } else if frame.mode == Mode::Tiling {
        "ws tiling"
    } else {
        "ws floating"
    }
}

fn initial_viewport() -> Viewport {
    let (width, height) = viewport_size();
    Viewport { width, height, units: Units::CssPx }
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

/// Report layout persistence issues without hiding the host-side decision point.
pub(super) fn log_layout_error(action: &str, storage_key: &str, error: &LayoutError) {
    let message = format!("panel-kit {action} failed for storage key `{storage_key}`: {error}");

    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&message));

    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{message}");
}

/// Web-canary-specific rendering and demo bindings kept out of the executable entrypoint.
pub mod web_canary {
    use dioxus::prelude::*;
    use panel_kit::PanelHeaderButton;
    use panel_kit_core::badge::{BadgeAction, BadgeClickKind, BadgeKind, BadgeSpec};
    use panel_kit_core::frame::{FrameStatus, Placement, ProjectedFrame};
    use panel_kit_core::reducer::WorkspaceEvent;
    use panel_kit_core::spec::{PanelSpec, ResolvedWorkspace};
    use panel_kit_core::widgets::charts::{
        five_num, BoxItemModel, BoxItemView, FlameSpanModel, GaugeModel, SeriesModel, SeriesView,
    };
    use panel_kit_core::widgets::meter::MeterModel;
    use panel_kit_core::widgets::status::{StatusModel, StatusState};
    use panel_kit_core::widgets::table::{
        ColumnWidth, TableCell, TableColumn, TableModel, TableRow, TableView, TextAlign,
    };
    use panel_kit_core::widgets::{ContentSpec, ContentView, DataSource, ScrollPolicy, TextModel};
    use panel_kit_core::SpecPanelId;

    /// Render projected panels for the spec-driven web canary.
    pub fn workspace_contents(
        frame: &ProjectedFrame<'_, SpecPanelId>,
        resolved: &ResolvedWorkspace,
        emit: EventHandler<WorkspaceEvent<SpecPanelId>>,
        header_clicks: Signal<u32>,
    ) -> Element {
        if frame.status == FrameStatus::TooSmall {
            return rsx! {
                div { class: "too-small", "WorkspaceSpec viewport is too small for panel projection." }
            };
        }

        let show_header = has_header_chrome(resolved);
        let show_lights = has_traffic_lights(resolved);

        rsx! {
            for panel in frame.panels.iter().copied() {
                if let Some(meta) = resolved.catalog.get(panel.key) {
                    if let Some(spec) = panel_spec(resolved, panel.key) {
                        {
                            let panel_class = format!("panel-{}", meta.slug);
                            let maximized = matches!(panel.placement, Placement::Maximized);
                            panel_kit::widgets::panel::panel_shell(panel, Some(&panel_class), rsx! {
                                if show_header {
                                    {panel_kit::widgets::panel::panel_chrome_with_events(
                                        panel,
                                        meta,
                                        emit,
                                        show_lights.then(|| panel_kit::widgets::panel::traffic_lights(panel, emit)),
                                        header_actions(spec, maximized, header_clicks),
                                    )}
                                }
                                {panel_kit::widgets::panel::panel_body(render_content(
                                    spec,
                                    maximized,
                                    *header_clicks.read(),
                                ))}
                                if resolved.chrome.resize_grip {
                                    {panel_kit::widgets::panel::resize_grip(panel, emit)}
                                }
                            })
                        }
                    }
                }
            }
        }
    }

    /// Human-readable mode label for the canary status bar.
    pub fn mode_label(mode: panel_kit_core::Mode) -> &'static str {
        match mode {
            panel_kit_core::Mode::Floating => "floating",
            panel_kit_core::Mode::Tiling => "tiling",
        }
    }

    /// Human-readable surface label for the canary status bar.
    pub fn surface_label(class: panel_kit_core::SurfaceClass) -> &'static str {
        match class {
            panel_kit_core::SurfaceClass::Compact => "compact",
            panel_kit_core::SurfaceClass::Tablet => "tablet",
            panel_kit_core::SurfaceClass::Regular => "regular",
        }
    }

    /// Comma-separated content-kind summary for the canary top bar.
    pub fn content_kinds(resolved: &ResolvedWorkspace) -> String {
        resolved
            .panels
            .iter()
            .map(|panel| panel.content.kind())
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn panel_spec(resolved: &ResolvedWorkspace, key: SpecPanelId) -> Option<&PanelSpec> {
        resolved.panels.get(key.index() as usize)
    }

    fn has_header_chrome(resolved: &ResolvedWorkspace) -> bool {
        let chrome = &resolved.chrome;
        chrome.panel_frame
            || chrome.title_in_border
            || chrome.mode_control
            || chrome.minimize_control
            || chrome.maximize_control
    }

    fn has_traffic_lights(resolved: &ResolvedWorkspace) -> bool {
        let chrome = &resolved.chrome;
        chrome.mode_control || chrome.minimize_control || chrome.maximize_control
    }

    fn header_actions(
        spec: &PanelSpec,
        maximized: bool,
        mut header_clicks: Signal<u32>,
    ) -> Option<Element> {
        if !matches!(spec.content, ContentSpec::Status { .. }) {
            return None;
        }

        Some(rsx! {
            PanelHeaderButton {
                label: "ping",
                title: "Exercise app-owned header action in spec-composed chrome",
                active: *header_clicks.read() > 0 || maximized,
                on_press: move |_| *header_clicks.write() += 1,
            }
        })
    }

    fn render_content(spec: &PanelSpec, maximized: bool, header_clicks: u32) -> Element {
        match &spec.content {
            ContentSpec::Custom { binding } => render_custom(binding, spec, maximized, header_clicks),
            ContentSpec::Text { source, scroll } => render_text(source, *scroll),
            ContentSpec::Editor { binding, multiline, placeholder } => rsx! {
                textarea {
                    class: "pk-editor",
                    aria_label: "editor binding {binding}",
                    placeholder: "{placeholder}",
                    rows: if *multiline { "12" } else { "1" },
                }
            },
            ContentSpec::Badges { source } => render_badges(source),
            ContentSpec::Table { source } => render_table(source),
            ContentSpec::TimeSeries { source, unit } => render_time_series(source, unit),
            ContentSpec::Gauges { source } => render_gauges(source),
            ContentSpec::Flamegraph { source } => render_flamegraph(source),
            ContentSpec::Boxplot { source } => render_boxplot(source),
            ContentSpec::Meter { source } => render_meter(source),
            ContentSpec::Status { source } => render_status(source),
            ContentSpec::Spinner { label } => render_spinner(label),
        }
    }

    fn render_custom(binding: &str, spec: &PanelSpec, maximized: bool, header_clicks: u32) -> Element {
        match binding {
            "canary.workspace" => rsx! {
                section { class: "pk-custom-card", role: "group", aria_label: "workspace summary",
                    h2 { "Host-owned composition" }
                    p { "Panel “{spec.title}” was declared by Nix, resolved into SpecPanelId, and projected into web parts." }
                    p { "maximized: {maximized} · header actions: {header_clicks}" }
                }
            },
            "canary.theme" => rsx! {
                section { class: "pk-custom-card", role: "group", aria_label: "theme summary",
                    h2 { "Theme + chrome" }
                    p { "CSS variables come from the single core theme source; chrome booleans decide which parts mount." }
                    ul { class: "spec-list",
                        li { "focus ring, palette, typography, and density are spec leaves" }
                        li { "dock/header/lights/grip are opt-in composition parts" }
                    }
                }
            },
            _ => panel_kit::widgets::content_view(ContentView::Custom { binding }, 0, noop_badge_action()),
        }
    }

    fn render_text(source: &DataSource<TextModel>, scroll: ScrollPolicy) -> Element {
        match source {
            DataSource::Inline { value } => {
                panel_kit::widgets::content_view(ContentView::Text { text: &value.text, scroll }, 0, noop_badge_action())
            }
            DataSource::Binding { id } => {
                let text = bound_text(id);
                panel_kit::widgets::content_view(ContentView::Text { text, scroll }, 0, noop_badge_action())
            }
        }
    }

    fn render_badges(source: &DataSource<Vec<BadgeSpec>>) -> Element {
        match source {
            DataSource::Inline { value } => panel_kit::widgets::content_view(ContentView::Badges(value), 0, noop_badge_action()),
            DataSource::Binding { id } => {
                let badges = bound_badges(id);
                panel_kit::widgets::content_view(ContentView::Badges(&badges), 0, noop_badge_action())
            }
        }
    }

    fn render_table(source: &DataSource<TableModel>) -> Element {
        match source {
            DataSource::Inline { value } => render_table_model(value),
            DataSource::Binding { id } => render_table_model(&bound_table(id)),
        }
    }

    fn render_table_model(table: &TableModel) -> Element {
        panel_kit::widgets::content_view(
            ContentView::Table(TableView { columns: &table.columns, rows: &table.rows }),
            0,
            noop_badge_action(),
        )
    }

    fn render_time_series(source: &DataSource<Vec<SeriesModel>>, unit: &str) -> Element {
        match source {
            DataSource::Inline { value } => render_series_models(value, unit),
            DataSource::Binding { id } => render_series_models(&bound_series(id), unit),
        }
    }

    fn render_series_models(series: &[SeriesModel], unit: &str) -> Element {
        let views: Vec<SeriesView<'_>> = series
            .iter()
            .map(|model| SeriesView { name: &model.name, points: &model.points })
            .collect();
        panel_kit::widgets::content_view(ContentView::TimeSeries { series: &views, unit }, 0, noop_badge_action())
    }

    fn render_gauges(source: &DataSource<Vec<GaugeModel>>) -> Element {
        match source {
            DataSource::Inline { value } => panel_kit::widgets::content_view(ContentView::Gauges(value), 0, noop_badge_action()),
            DataSource::Binding { id } => {
                let gauges = bound_gauges(id);
                panel_kit::widgets::content_view(ContentView::Gauges(&gauges), 0, noop_badge_action())
            }
        }
    }

    fn render_flamegraph(source: &DataSource<Vec<FlameSpanModel>>) -> Element {
        match source {
            DataSource::Inline { value } => panel_kit::widgets::content_view(ContentView::Flamegraph(value), 0, noop_badge_action()),
            DataSource::Binding { id } => {
                let spans = bound_flamegraph(id);
                panel_kit::widgets::content_view(ContentView::Flamegraph(&spans), 0, noop_badge_action())
            }
        }
    }

    fn render_boxplot(source: &DataSource<Vec<BoxItemModel>>) -> Element {
        match source {
            DataSource::Inline { value } => render_box_models(value),
            DataSource::Binding { id } => render_box_models(&bound_boxplot(id)),
        }
    }

    fn render_box_models(items: &[BoxItemModel]) -> Element {
        let views = box_item_views(items);
        panel_kit::widgets::content_view(ContentView::Boxplot(&views), 0, noop_badge_action())
    }

    fn render_meter(source: &DataSource<MeterModel>) -> Element {
        match source {
            DataSource::Inline { value } => panel_kit::widgets::content_view(ContentView::Meter(value), 0, noop_badge_action()),
            DataSource::Binding { id } => {
                let meter = bound_meter(id);
                panel_kit::widgets::content_view(ContentView::Meter(&meter), 0, noop_badge_action())
            }
        }
    }

    fn render_status(source: &DataSource<StatusModel>) -> Element {
        match source {
            DataSource::Inline { value } => panel_kit::widgets::content_view(ContentView::Status(value), 0, noop_badge_action()),
            DataSource::Binding { id } => {
                let status = bound_status(id);
                panel_kit::widgets::content_view(ContentView::Status(&status), 0, noop_badge_action())
            }
        }
    }

    fn render_spinner(label: &DataSource<String>) -> Element {
        match label {
            DataSource::Inline { value } => panel_kit::widgets::content_view(
                ContentView::Spinner { label: value.as_str() },
                0,
                noop_badge_action(),
            ),
            DataSource::Binding { id } => {
                let label = bound_spinner_label(id);
                panel_kit::widgets::content_view(ContentView::Spinner { label }, 0, noop_badge_action())
            }
        }
    }

    fn box_item_views(items: &[BoxItemModel]) -> Vec<BoxItemView<'_>> {
        items
            .iter()
            .filter_map(|item| {
                five_num(&item.samples).map(|summary| BoxItemView {
                    label: &item.label,
                    summary,
                    color: item.color,
                })
            })
            .collect()
    }

    fn noop_badge_action() -> EventHandler<BadgeAction> {
        EventHandler::new(|_: BadgeAction| {})
    }

    fn bound_text(id: &str) -> &'static str {
        match id {
            "canary.notes" => "WorkspaceSpec text binding rendered by the web scroll painter.\n\nScroll policy is native overflow:auto, so wheel chaining can bubble at the content edge.",
            _ => "unregistered text binding",
        }
    }

    fn bound_badges(id: &str) -> Vec<BadgeSpec> {
        let mut spec = BadgeSpec::new("stage", id.trim_start_matches("canary."), BadgeKind::Status);
        spec.active = true;
        spec.with_plus = true;
        spec.accent_color = Some((94, 243, 140));
        spec.click_kind = BadgeClickKind::Clicked;
        vec![spec]
    }

    fn bound_table(_id: &str) -> TableModel {
        TableModel {
            columns: vec![
                TableColumn { key: "node".into(), title: "Node".into(), width: ColumnWidth::Flex { weight: 2 }, align: TextAlign::Left },
                TableColumn { key: "health".into(), title: "Health".into(), width: ColumnWidth::Fixed { value: 8 }, align: TextAlign::Center },
                TableColumn { key: "load".into(), title: "Load".into(), width: ColumnWidth::Fixed { value: 10 }, align: TextAlign::Right },
            ],
            rows: vec![
                TableRow { cells: vec![
                    TableCell::Text("api".into()),
                    TableCell::Status { label: "ok".into(), color: (39, 201, 63) },
                    TableCell::Meter { ratio: 0.42, text: "42%".into(), color: Some((94, 243, 140)) },
                ] },
                TableRow { cells: vec![
                    TableCell::Text("worker".into()),
                    TableCell::Status { label: "warn".into(), color: (255, 189, 46) },
                    TableCell::Meter { ratio: 0.68, text: "68%".into(), color: Some((255, 189, 46)) },
                ] },
            ],
        }
    }

    fn bound_series(_id: &str) -> Vec<SeriesModel> {
        vec![
            SeriesModel { name: "p50".into(), points: vec![(0.0, 12.0), (1.0, 18.0), (2.0, 15.0)] },
            SeriesModel { name: "p95".into(), points: vec![(0.0, 35.0), (1.0, 42.0), (2.0, 39.0)] },
        ]
    }

    fn bound_gauges(_id: &str) -> Vec<GaugeModel> {
        vec![
            GaugeModel { label: "CPU".into(), ratio: 0.64, text: "64%".into() },
            GaugeModel { label: "Queue".into(), ratio: 0.28, text: "7 / 25".into() },
        ]
    }

    fn bound_flamegraph(_id: &str) -> Vec<FlameSpanModel> {
        vec![
            FlameSpanModel { label: "root".into(), depth: 0, value: 100.0, color: Some((94, 243, 140)) },
            FlameSpanModel { label: "decode".into(), depth: 1, value: 34.0, color: Some((59, 155, 255)) },
            FlameSpanModel { label: "paint".into(), depth: 1, value: 21.0, color: Some((255, 95, 195)) },
        ]
    }

    fn bound_boxplot(_id: &str) -> Vec<BoxItemModel> {
        vec![
            BoxItemModel { label: "latency".into(), samples: vec![14.0, 18.0, 21.0, 28.0, 35.0], color: Some((131, 183, 204)) },
            BoxItemModel { label: "queue".into(), samples: vec![1.0, 4.0, 8.0, 10.0, 14.0], color: None },
        ]
    }

    fn bound_meter(_id: &str) -> MeterModel {
        MeterModel { label: "memory".into(), ratio: 0.57, text: "57%".into(), color: Some((94, 243, 140)) }
    }

    fn bound_status(_id: &str) -> StatusModel {
        StatusModel { label: "deploy".into(), state: StatusState::Ok, color: (39, 201, 63) }
    }

    fn bound_spinner_label(_id: &str) -> &'static str {
        "warming renderer"
    }
}
