#[path = "support/web_native_fidelity.rs"]
mod web_native_fidelity;
#[path = "support/web_part_fixtures.rs"]
mod web_part_fixtures;

use dioxus::prelude::*;
use panel_kit_core::frame::{DockProjection, Placement, TileGridProjection};
use panel_kit_core::panel::PanelCatalog;
use panel_kit_core::reducer::{HitTarget, PanelPart, WheelDisposition, WorkspaceEvent};
use panel_kit_core::widgets::table::{
    ColumnWidth, TableCell, TableColumn, TableRow, TableView, TextAlign,
};
use panel_kit_core::{
    FocusContext, Key, KeyChord, PointerButton, PointerEvent, PointerEventKind, Region,
    SurfaceCapabilities, SurfaceClass,
};
use web_part_fixtures::{nodes_meta, noop_workspace_event_handler, projected_panel, ProbePanel};

#[component]
fn StandaloneSurfaceProbe() -> Element {
    let panel = projected_panel(Placement::Floating, false);

    panel_kit::widgets::panel::panel_surface(
        panel,
        Some("panel-nodes"),
        rsx! { article { class: "body-probe", "body only" } },
    )
}

#[test]
fn standalone_surface_has_no_implicit_chrome() {
    let html = dioxus_ssr::render_element(rsx! { StandaloneSurfaceProbe {} });

    assert!(html.contains("class=\"panel panel-nodes\""), "{html}");
    assert!(html.contains("class=\"panel-body\""), "{html}");
    assert!(html.contains("overflow:auto"), "{html}");
    assert!(html.contains("body only"), "{html}");
    assert!(
        !html.contains("panel-head"),
        "surface mounted implicit chrome: {html}"
    );
    assert!(
        !html.contains("lights"),
        "surface mounted implicit traffic lights: {html}"
    );
    assert!(
        !html.contains("dock"),
        "surface mounted implicit dock: {html}"
    );
}

#[component]
fn ChromeProbe() -> Element {
    let panel = projected_panel(Placement::Floating, true);
    let meta = nodes_meta();

    rsx! {
        div { class: "chrome-probe",
            {panel_kit::widgets::panel::panel_shell(panel, Some("panel-nodes"), rsx! {
                {panel_kit::widgets::panel::panel_chrome_with_events(
                    panel,
                    &meta,
                    noop_workspace_event_handler(),
                    Some(panel_kit::widgets::panel::traffic_lights(panel, noop_workspace_event_handler())),
                    Some(rsx! { button { r#type: "button", class: "app-action", "refresh" } }),
                )}
                {panel_kit::widgets::panel::panel_body(rsx! { div { "scrollable" } })}
                {panel_kit::widgets::panel::resize_grip(panel, noop_workspace_event_handler())}
            })}
        }
    }
}

#[test]
fn web_panel_preserves_focus_aria_and_overflow() {
    let html = dioxus_ssr::render_element(rsx! { ChromeProbe {} });

    web_native_fidelity::assert_controller_era_panel_semantics(&html);
}

#[component]
fn SiblingChromeProbe() -> Element {
    let panel = projected_panel(Placement::Floating, true);
    let meta = nodes_meta();

    rsx! {
        div { class: "chrome-probe",
            {panel_kit::widgets::panel::panel_surface(panel, Some("panel-nodes"), rsx! { div { "scrollable" } })}
            {panel_kit::widgets::panel::panel_chrome(
                panel,
                &meta,
                Some(rsx! { button { r#type: "button", class: "app-action", "refresh" } }),
            )}
            {panel_kit::widgets::panel::traffic_lights(panel, noop_workspace_event_handler())}
            {panel_kit::widgets::panel::resize_grip(panel, noop_workspace_event_handler())}
        }
    }
}

#[test]
fn sibling_panel_parts_do_not_match_controller_era_hierarchy() {
    let html = dioxus_ssr::render_element(rsx! { SiblingChromeProbe {} });
    let fidelity_result = std::panic::catch_unwind(|| {
        web_native_fidelity::assert_controller_era_panel_semantics(&html);
    });

    assert!(
        fidelity_result.is_err(),
        "sibling-only composition must not satisfy panel hierarchy fidelity: {html}"
    );
}

#[test]
fn web_and_tui_use_same_tile_grid_projection() {
    let grid = TileGridProjection {
        columns: 3,
        rows: 2,
        track_w: 120.0,
        track_h: 90.0,
        gap: 8.0,
        padding: 12.0,
    };
    let style = panel_kit::widgets::root::tile_grid_style(grid);

    assert_eq!(
        style,
        "grid-template-columns:repeat(3, 120px);grid-template-rows:repeat(2, 90px);gap:8px;padding:12px;"
    );

    let tiled = projected_panel(
        Placement::Tiled {
            column: 1,
            row: 2,
            column_span: 2,
            row_span: 3,
        },
        false,
    );
    assert_eq!(
        panel_kit::widgets::panel::panel_style(tiled),
        "grid-column:2 / span 2;grid-row:3 / span 3;"
    );
}

#[component]
fn DockOnlyProbe() -> Element {
    let catalog = PanelCatalog::try_new(vec![nodes_meta()]).expect("valid catalog");
    let items = [DockProjection {
        source_index: 0,
        key: ProbePanel::Nodes,
        region: Region {
            x: 0.0,
            y: 0.0,
            w: 120.0,
            h: 30.0,
        },
    }];

    panel_kit::widgets::dock::dock(
        &items,
        &catalog,
        noop_workspace_event_handler(),
        Some(rsx! { button { class: "snap-extra", "snap policy" } }),
    )
}

#[test]
fn dock_renders_as_replace_only_part() {
    let html = dioxus_ssr::render_element(rsx! { DockOnlyProbe {} });

    assert!(html.contains("class=\"dock\""), "{html}");
    assert!(html.contains("aria-label=\"Restore Nodes\""), "{html}");
    assert!(html.contains("Nodes"), "{html}");
    assert!(html.contains("class=\"dock-extras\""), "{html}");
    assert!(html.contains("class=\"snap-extra\""), "{html}");
    assert!(
        !html.contains("panel-body"),
        "dock mounted a panel surface: {html}"
    );
    assert!(
        !html.contains("panel-head"),
        "dock mounted panel chrome: {html}"
    );
}

#[component]
fn InteractiveTableProbe() -> Element {
    let columns = [TableColumn {
        key: "policy".into(),
        title: "Policy".into(),
        width: ColumnWidth::Flex { weight: 1 },
        align: TextAlign::Left,
    }];
    let rows = [TableRow {
        cells: vec![TableCell::Text(
            "a long policy value preserved in the tooltip".into(),
        )],
    }];

    rsx! {
        {panel_kit::widgets::table::table(
            TableView { columns: &columns, rows: &rows },
            Some(0),
            Some(EventHandler::new(|_: usize| {})),
            true,
        )}
        {panel_kit::widgets::table::table(
            TableView { columns: &columns, rows: &[] },
            None,
            None,
            false,
        )}
    }
}

#[test]
fn table_exposes_host_selection_density_tooltips_and_empty_state() {
    let html = dioxus_ssr::render_element(rsx! { InteractiveTableProbe {} });

    assert!(
        html.contains("class=\"pk-widget pk-table pk-table-dense\""),
        "{html}"
    );
    assert!(
        html.contains("class=\"pk-table-row selected clickable\""),
        "{html}"
    );
    assert!(
        html.contains("title=\"a long policy value preserved in the tooltip\""),
        "{html}"
    );
    assert!(html.contains("class=\"pk-table-empty\""), "{html}");
    assert!(html.contains("no rows"), "{html}");
}

#[test]
fn input_adapter_translates_events_and_preserves_wheel_precedence() {
    let key = panel_kit::input::key_event_from_parts(
        KeyChord {
            key: Key::Char('m'),
            shift: false,
            alt: false,
            ctrl: false,
            meta: true,
        },
        FocusContext::Panel(ProbePanel::Nodes),
    );
    assert_eq!(
        key,
        WorkspaceEvent::Key {
            chord: KeyChord {
                key: Key::Char('m'),
                shift: false,
                alt: false,
                ctrl: false,
                meta: true
            },
            focus: FocusContext::Panel(ProbePanel::Nodes)
        }
    );

    let pointer = panel_kit::input::pointer_event_from_parts(
        HitTarget::Panel {
            key: ProbePanel::Nodes,
            part: PanelPart::Header,
        },
        PointerEvent {
            kind: PointerEventKind::Down(PointerButton::Primary),
            x: 7.0,
            y: 9.0,
        },
    );
    assert_eq!(
        pointer,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: ProbePanel::Nodes,
                part: PanelPart::Header
            },
            event: PointerEvent {
                kind: PointerEventKind::Down(PointerButton::Primary),
                x: 7.0,
                y: 9.0
            }
        }
    );

    assert_eq!(
        panel_kit::input::wheel_event_from_parts::<ProbePanel>(12.0, true),
        WorkspaceEvent::Wheel {
            delta_y: 12.0,
            disposition: WheelDisposition::ContentConsumed
        }
    );
    assert_eq!(
        panel_kit::input::wheel_event_from_parts::<ProbePanel>(12.0, false),
        WorkspaceEvent::Wheel {
            delta_y: 12.0,
            disposition: WheelDisposition::BubbleToWorkspace
        }
    );
}

#[test]
fn surface_adapter_classifies_browser_viewports() {
    let profile = panel_kit::surface::surface_profile(900.0);
    assert_eq!(profile.class, SurfaceClass::Tablet);
    assert_eq!(
        profile.caps,
        SurfaceCapabilities {
            coarse_pointer: false,
            hover: true,
            keyboard: true
        }
    );

    assert_eq!(
        panel_kit::surface::viewport_from_size(640.0, 480.0)
            .unwrap()
            .width,
        640.0
    );
    assert!(panel_kit::surface::viewport_from_size(f64::NAN, 480.0).is_none());
}

#[test]
fn surface_registration_status_reports_failures() {
    let status = panel_kit::surface::ViewportObserverStatus {
        resize_observer: panel_kit::surface::ViewportObserverState::Registered,
        window_resize_listener: panel_kit::surface::ViewportObserverState::Failed("denied".into()),
    };

    assert!(!status.is_fully_registered());
}

#[test]
fn tiling_drag_to_swap_reorders_panels_via_surface_pointer_motion() {
    use panel_kit_core::reducer::{reduce, ChangePhase, ReduceContext, Snapshot, Viewport};
    use panel_kit_core::{Clamp, CommandStep, LayoutBuilder, Mode, SnapPolicy, TileMetrics, Units};
    let mut layout = LayoutBuilder::new();
    let panels = vec![
        layout.at(ProbePanel::Nodes, 16.0, 16.0, 360.0, 360.0).with_tile(2, 1),
        layout.at(ProbePanel::Logs, 16.0, 392.0, 360.0, 260.0).with_tile(2, 1),
    ];
    let mut snapshot = Snapshot::from_defaults(
        panels,
        Mode::Tiling,
        Viewport { width: 1400.0, height: 900.0, units: Units::CssPx },
    );

    let context = ReduceContext {
        surface: panel_kit::surface::surface_profile(1400.0),
        clamp: &Clamp::WEB,
        command_step: CommandStep::WEB,
        tile: &TileMetrics::WEB,
        snap: SnapPolicy::default(),
    };

    // 1. Pointer down on Nodes header starts tile drag
    let down = reduce(
        &mut snapshot,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: ProbePanel::Nodes,
                part: PanelPart::Header,
            },
            event: PointerEvent {
                kind: PointerEventKind::Down(PointerButton::Primary),
                x: 100.0,
                y: 20.0,
            },
        },
        context,
    );
    assert_eq!(down.phase, Some(ChangePhase::Continuous));
    assert_eq!(snapshot.tile_drag, Some(ProbePanel::Nodes));

    // 2. Dragging pointer over Logs surface (via onpointerenter/motion) triggers tile swap
    let motion = reduce(
        &mut snapshot,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: ProbePanel::Logs,
                part: PanelPart::Surface,
            },
            event: PointerEvent {
                kind: PointerEventKind::Moved,
                x: 100.0,
                y: 450.0,
            },
        },
        context,
    );
    assert_eq!(motion.phase, Some(ChangePhase::Continuous));
    assert_eq!(
        snapshot.panels.iter().map(|p| p.kind).collect::<Vec<_>>(),
        vec![ProbePanel::Logs, ProbePanel::Nodes]
    );

    // 3. Pointer up settles the gesture
    let up = reduce(
        &mut snapshot,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: ProbePanel::Nodes,
                part: PanelPart::Header,
            },
            event: PointerEvent {
                kind: PointerEventKind::Up(PointerButton::Primary),
                x: 100.0,
                y: 450.0,
            },
        },
        context,
    );
    assert_eq!(up.phase, Some(ChangePhase::Settled));
    assert_eq!(snapshot.tile_drag, None);
}
