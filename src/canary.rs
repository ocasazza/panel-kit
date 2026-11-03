//! Spec-driven canary rendering: projected panel bodies over a resolved
//! [`ResolvedWorkspace`], plus the fixed demo content the canaries bind.
//!
//! Promoted from the example support glue so every spec canary paints
//! identical content. [`workspace_contents`] is the composition: one
//! section per projected panel, spec-authored inline values painted by
//! [`render_content`], provider-backed ids routed through the host's
//! [`BindingContent`] seam. [`canary_content`] is the fixed-data
//! implementation the canaries pass.

use dioxus::prelude::*;
use crate::PanelHeaderButton;
use panel_kit_core::badge::{BadgeAction, BadgeClickKind, BadgeKind, BadgeSpec};
use panel_kit_core::frame::{FrameStatus, Placement, ProjectedFrame};
use panel_kit_core::reducer::WorkspaceEvent;
use panel_kit_core::spec::{PanelSpec, ResolvedWorkspace};
use panel_kit_core::widgets::charts::{
    five_num, BoxItemModel, BoxItemView, FlameSpanModel, GaugeModel, SeriesModel, SeriesView,
};
use panel_kit_core::widgets::data_table::ColumnKind;
use panel_kit_core::widgets::meter::MeterModel;
use panel_kit_core::widgets::status::{StatusModel, StatusState};
use panel_kit_core::widgets::table::{
    ColumnWidth, TableCell, TableColumn, TableModel, TableRow, TableView, TextAlign,
};
use panel_kit_core::widgets::{ContentSpec, ContentView, DataSource, ScrollPolicy};
use panel_kit_core::SpecPanelId;

/// Host-supplied content for one provider-backed (`DataSource::Binding`) id.
///
/// `id` is the authored binding id and `content` the panel's authored
/// `ContentSpec`, so a host reads the declared kind, scroll policy, and
/// unit instead of re-deriving them. Every bound panel body is painted by
/// this seam alone; [`canary_content`] is the fixed-data implementation
/// the workspace canary passes.
pub type BindingContent<'a> = &'a dyn Fn(&str, &ContentSpec) -> Element;

/// Render projected panels for the spec-driven web canary.
pub fn workspace_contents(
    frame: &ProjectedFrame<'_, SpecPanelId>,
    resolved: &ResolvedWorkspace,
    emit: EventHandler<WorkspaceEvent<SpecPanelId>>,
    header_clicks: Signal<u32>,
    binding: BindingContent<'_>,
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
                        {
                        let class = crate::widgets::panel_layout::panel_class(panel, Some(&panel_class));
                        let style = crate::widgets::panel_layout::panel_style(panel);
                        let key = panel.key;
                        let emit_down = emit;
                        let emit_up = emit;
                        rsx! {
                            section {
                                class: "{class}",
                                style: "{style}",
                                onpointerdown: move |event: dioxus::events::PointerEvent| {
                                    let coords = event.client_coordinates();
                                    emit_down.call(WorkspaceEvent::Pointer {
                                        target: panel_kit_core::reducer::HitTarget::Panel {
                                            key,
                                            part: panel_kit_core::reducer::PanelPart::Surface,
                                        },
                                        event: panel_kit_core::PointerEvent {
                                            kind: panel_kit_core::PointerEventKind::Down(panel_kit_core::PointerButton::Primary),
                                            x: coords.x,
                                            y: coords.y,
                                        },
                                    });
                                },
                                onpointerenter: move |event: dioxus::events::PointerEvent| {
                                    let coords = event.client_coordinates();
                                    emit.call(WorkspaceEvent::Pointer {
                                        target: panel_kit_core::reducer::HitTarget::Panel {
                                            key,
                                            part: panel_kit_core::reducer::PanelPart::Surface,
                                        },
                                        event: panel_kit_core::PointerEvent {
                                            kind: panel_kit_core::PointerEventKind::Moved,
                                            x: coords.x,
                                            y: coords.y,
                                        },
                                    });
                                },
                                onpointerup: move |event: dioxus::events::PointerEvent| {
                                    let coords = event.client_coordinates();
                                    emit_up.call(WorkspaceEvent::Pointer {
                                        target: panel_kit_core::reducer::HitTarget::Panel {
                                            key,
                                            part: panel_kit_core::reducer::PanelPart::Surface,
                                        },
                                        event: panel_kit_core::PointerEvent {
                                            kind: panel_kit_core::PointerEventKind::Up(panel_kit_core::PointerButton::Primary),
                                            x: coords.x,
                                            y: coords.y,
                                        },
                                    });
                                },
                                if show_header {
                                    {crate::widgets::panel::panel_chrome_with_events(
                                        panel,
                                        meta,
                                        emit,
                                        show_lights.then(|| crate::widgets::panel::traffic_lights(panel, emit)),
                                        header_actions(spec, maximized, header_clicks),
                                    )}
                                }
                                {crate::widgets::panel::panel_body(render_content(
                                    spec,
                                    binding,
                                    maximized,
                                    *header_clicks.read(),
                                ))}
                                if resolved.chrome.resize_grip {
                                    {crate::widgets::panel::resize_grip(panel, emit)}
                                }
                            }
                        }
                    }
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

/// Paint one panel body: spec-authored inline values here, every
/// provider-backed id through the host seam.
fn render_content(
    spec: &PanelSpec,
    binding: BindingContent<'_>,
    maximized: bool,
    header_clicks: u32,
) -> Element {
    let content = &spec.content;
    match content {
        ContentSpec::Custom { binding } => {
            render_custom(binding, spec, maximized, header_clicks)
        }
        ContentSpec::Editor {
            binding,
            multiline,
            placeholder,
        } => paint_editor(binding, *multiline, placeholder),
        ContentSpec::Text { source, scroll } => match source {
            DataSource::Inline { value } => paint_text(&value.text, *scroll),
            DataSource::Binding { id } => binding(id, content),
        },
        ContentSpec::Badges { source } => match source {
            DataSource::Inline { value } => paint_badges(value),
            DataSource::Binding { id } => binding(id, content),
        },
        ContentSpec::Table { source } => match source {
            DataSource::Inline { value } => paint_table(&spec.id, value),
            DataSource::Binding { id } => binding(id, content),
        },
        ContentSpec::TimeSeries { source, unit } => match source {
            DataSource::Inline { value } => paint_series(value, unit),
            DataSource::Binding { id } => binding(id, content),
        },
        ContentSpec::Gauges { source } => match source {
            DataSource::Inline { value } => paint_gauges(value),
            DataSource::Binding { id } => binding(id, content),
        },
        ContentSpec::Flamegraph { source } => match source {
            DataSource::Inline { value } => paint_flamegraph(value),
            DataSource::Binding { id } => binding(id, content),
        },
        ContentSpec::Boxplot { source } => match source {
            DataSource::Inline { value } => paint_boxplot(value),
            DataSource::Binding { id } => binding(id, content),
        },
        ContentSpec::Meter { source } => match source {
            DataSource::Inline { value } => paint_meter(value),
            DataSource::Binding { id } => binding(id, content),
        },
        ContentSpec::Status { source } => match source {
            DataSource::Inline { value } => paint_status(value),
            DataSource::Binding { id } => binding(id, content),
        },
        ContentSpec::Spinner { label } => match label {
            DataSource::Inline { value } => paint_spinner(value),
            DataSource::Binding { id } => binding(id, content),
        },
    }
}

/// Fixed demo content for the workspace canary's own binding ids.
///
/// Ids the canary does not register keep the unregistered-binding marker
/// each kind showed before, so an unmapped panel is never a blank body.
pub fn canary_content(id: &str, content: &ContentSpec) -> Element {
    match content {
        ContentSpec::Text { scroll, .. } => paint_text(bound_text(id), *scroll),
        ContentSpec::Badges { .. } => paint_badges(&bound_badges(id)),
        ContentSpec::Table { .. } => paint_table(id, &bound_table(id)),
        ContentSpec::TimeSeries { unit, .. } => paint_series(&bound_series(id), unit),
        ContentSpec::Gauges { .. } => paint_gauges(&bound_gauges(id)),
        ContentSpec::Flamegraph { .. } => paint_flamegraph(&bound_flamegraph(id)),
        ContentSpec::Boxplot { .. } => paint_boxplot(&bound_boxplot(id)),
        ContentSpec::Meter { .. } => paint_meter(&bound_meter(id)),
        ContentSpec::Status { .. } => paint_status(&bound_status(id)),
        ContentSpec::Spinner { .. } => paint_spinner(bound_spinner_label(id)),
        ContentSpec::Custom { .. } => paint_custom(id),
        ContentSpec::Editor {
            binding,
            multiline,
            placeholder,
        } => paint_editor(binding, *multiline, placeholder),
    }
}

fn render_custom(
    binding: &str,
    spec: &PanelSpec,
    maximized: bool,
    header_clicks: u32,
) -> Element {
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
        _ => paint_custom(binding),
    }
}

fn paint_text(text: &str, scroll: ScrollPolicy) -> Element {
    crate::widgets::content_view(ContentView::Text { text, scroll }, 0, noop_badge_action())
}

fn paint_editor(binding: &str, multiline: bool, placeholder: &str) -> Element {
    rsx! {
        textarea {
            class: "pk-editor",
            aria_label: "editor binding {binding}",
            placeholder: "{placeholder}",
            rows: if multiline { "12" } else { "1" },
        }
    }
}

fn paint_custom(binding: &str) -> Element {
    crate::widgets::content_view(ContentView::Custom { binding }, 0, noop_badge_action())
}

fn paint_badges(specs: &[BadgeSpec]) -> Element {
    crate::widgets::content_view(ContentView::Badges(specs), 0, noop_badge_action())
}

/// A table painted under `owner`: the panel or binding id it belongs to.
fn paint_table(owner: &str, table: &TableModel) -> Element {
    crate::widgets::content_view(
        ContentView::Table {
            binding: owner,
            view: TableView {
                columns: &table.columns,
                rows: &table.rows.iter().collect::<Vec<_>>(),
            },
        },
        0,
        noop_badge_action(),
    )
}

fn paint_series(series: &[SeriesModel], unit: &str) -> Element {
    let views: Vec<SeriesView<'_>> = series
        .iter()
        .map(|model| SeriesView {
            name: &model.name,
            points: &model.points,
        })
        .collect();
    crate::widgets::content_view(
        ContentView::TimeSeries {
            series: &views,
            unit,
        },
        0,
        noop_badge_action(),
    )
}

fn paint_gauges(gauges: &[GaugeModel]) -> Element {
    crate::widgets::content_view(ContentView::Gauges(gauges), 0, noop_badge_action())
}

fn paint_flamegraph(spans: &[FlameSpanModel]) -> Element {
    crate::widgets::content_view(ContentView::Flamegraph(spans), 0, noop_badge_action())
}

fn paint_boxplot(items: &[BoxItemModel]) -> Element {
    let views = box_item_views(items);
    crate::widgets::content_view(ContentView::Boxplot(&views), 0, noop_badge_action())
}

fn paint_meter(meter: &MeterModel) -> Element {
    crate::widgets::content_view(ContentView::Meter(meter), 0, noop_badge_action())
}

fn paint_status(status: &StatusModel) -> Element {
    crate::widgets::content_view(ContentView::Status(status), 0, noop_badge_action())
}

fn paint_spinner(label: &str) -> Element {
    crate::widgets::content_view(ContentView::Spinner { label }, 0, noop_badge_action())
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
            TableColumn {
                key: "node".into(),
                title: "Node".into(),
                width: ColumnWidth::Flex { weight: 2 },
                align: TextAlign::Left,
                kind: ColumnKind::Text,
            },
            TableColumn {
                key: "health".into(),
                title: "Health".into(),
                width: ColumnWidth::Fixed { value: 8 },
                align: TextAlign::Center,
                kind: ColumnKind::Enum,
            },
            TableColumn {
                key: "load".into(),
                title: "Load".into(),
                width: ColumnWidth::Fixed { value: 10 },
                align: TextAlign::Right,
                kind: ColumnKind::Number,
            },
        ],
        rows: vec![
            TableRow {
                id: "api".into(),
                tags: Vec::new(),
                cells: vec![
                    TableCell::Text("api".into()),
                    TableCell::Status {
                        label: "ok".into(),
                        color: (39, 201, 63),
                    },
                    TableCell::Meter {
                        ratio: 0.42,
                        text: "42%".into(),
                        color: Some((94, 243, 140)),
                    },
                ],
            },
            TableRow {
                id: "worker".into(),
                tags: Vec::new(),
                cells: vec![
                    TableCell::Text("worker".into()),
                    TableCell::Status {
                        label: "warn".into(),
                        color: (255, 189, 46),
                    },
                    TableCell::Meter {
                        ratio: 0.68,
                        text: "68%".into(),
                        color: Some((255, 189, 46)),
                    },
                ],
            },
        ],
    }
}

fn bound_series(_id: &str) -> Vec<SeriesModel> {
    vec![
        SeriesModel {
            name: "p50".into(),
            points: vec![(0.0, 12.0), (1.0, 18.0), (2.0, 15.0)],
        },
        SeriesModel {
            name: "p95".into(),
            points: vec![(0.0, 35.0), (1.0, 42.0), (2.0, 39.0)],
        },
    ]
}

fn bound_gauges(_id: &str) -> Vec<GaugeModel> {
    vec![
        GaugeModel {
            label: "CPU".into(),
            ratio: 0.64,
            text: "64%".into(),
        },
        GaugeModel {
            label: "Queue".into(),
            ratio: 0.28,
            text: "7 / 25".into(),
        },
    ]
}

fn bound_flamegraph(_id: &str) -> Vec<FlameSpanModel> {
    vec![
        FlameSpanModel {
            label: "root".into(),
            depth: 0,
            value: 100.0,
            color: Some((94, 243, 140)),
        },
        FlameSpanModel {
            label: "decode".into(),
            depth: 1,
            value: 34.0,
            color: Some((59, 155, 255)),
        },
        FlameSpanModel {
            label: "paint".into(),
            depth: 1,
            value: 21.0,
            color: Some((255, 95, 195)),
        },
    ]
}

fn bound_boxplot(_id: &str) -> Vec<BoxItemModel> {
    vec![
        BoxItemModel {
            label: "latency".into(),
            samples: vec![14.0, 18.0, 21.0, 28.0, 35.0],
            color: Some((131, 183, 204)),
        },
        BoxItemModel {
            label: "queue".into(),
            samples: vec![1.0, 4.0, 8.0, 10.0, 14.0],
            color: None,
        },
    ]
}

fn bound_meter(_id: &str) -> MeterModel {
    MeterModel {
        label: "memory".into(),
        ratio: 0.57,
        text: "57%".into(),
        color: Some((94, 243, 140)),
    }
}

fn bound_status(_id: &str) -> StatusModel {
    StatusModel {
        label: "deploy".into(),
        state: StatusState::Ok,
        color: (39, 201, 63),
    }
}

fn bound_spinner_label(_id: &str) -> &'static str {
    "warming renderer"
}
