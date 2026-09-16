use crate::frame::{
    hit_test, project_chrome, project_dock_into, project_panel, ChromeProjectionInput, FrameStatus,
    PanelProjectionInput, Placement,
};
use crate::reducer::{HitTarget, PanelPart, Viewport};
use crate::{ChromeMetrics, Clamp, Mode, Region, WinState};

use super::fixtures::{
    assert_rect_clean, center, keys, project_snapshot, scratch_for, semantic_snapshot, TestPanel,
};

#[test]
fn projected_frame_resolves_all_semantic_regions() {
    let snapshot = semantic_snapshot(Mode::Floating);
    let mut scratch = scratch_for(&snapshot);
    let frame = project_snapshot(&snapshot, &mut scratch);

    assert_eq!(frame.status, FrameStatus::Ready);
    assert_eq!(frame.mode, Mode::Floating);
    assert_eq!(
        keys(frame.panels),
        vec![TestPanel::Beta, TestPanel::Alpha, TestPanel::Gamma]
    );
    assert_eq!(
        frame.dock.iter().map(|entry| entry.key).collect::<Vec<_>>(),
        vec![TestPanel::Docked]
    );

    let gamma = frame
        .panels
        .iter()
        .find(|panel| panel.key == TestPanel::Gamma)
        .unwrap();
    assert_eq!(gamma.placement, Placement::Floating);
    assert_eq!(gamma.z, 30);
    assert!(gamma.state == WinState::Floating);
    assert!(gamma.focused);
    assert!(gamma.pointer_dragging);
    assert!(!gamma.tile_dragging);
    assert!(gamma.chrome.outer.w > 0.0);
    assert!(gamma.chrome.body.h > 0.0);
    assert!(gamma.chrome.header_hit.h > 0.0);
    assert!(gamma.chrome.mode_hit.is_some());
    assert!(gamma.chrome.minimize_hit.is_some());
    assert!(gamma.chrome.maximize_hit.is_some());
    assert!(gamma.chrome.resize_hit.is_some());

    assert_eq!(
        hit_test(&frame, center(gamma.chrome.body)),
        Some(HitTarget::Panel {
            key: TestPanel::Gamma,
            part: PanelPart::Surface
        })
    );
    assert_eq!(
        hit_test(&frame, center(gamma.chrome.mode_hit.unwrap())),
        Some(HitTarget::Panel {
            key: TestPanel::Gamma,
            part: PanelPart::ModeControl
        })
    );
    assert_eq!(
        hit_test(&frame, center(frame.dock[0].region)),
        Some(HitTarget::Dock {
            key: TestPanel::Docked
        })
    );
    assert!(frame.content_extent.h >= 230.0);
}

#[test]
fn too_small_viewport_has_no_negative_or_nan_rectangles() {
    let mut snapshot = semantic_snapshot(Mode::Floating);
    snapshot.viewport.width = Clamp::WEB.min_w;
    snapshot.viewport.height = Clamp::WEB.min_h;
    let mut scratch = scratch_for(&snapshot);
    let frame = project_snapshot(&snapshot, &mut scratch);

    assert_eq!(frame.status, FrameStatus::TooSmall);
    assert_rect_clean(frame.chrome.root);
    assert_rect_clean(frame.chrome.workspace);
    assert_rect_clean(frame.chrome.dock);
    assert!(frame.panels.is_empty());
    assert!(frame.dock.is_empty());

    snapshot.viewport.width = f64::NAN;
    let mut scratch = scratch_for(&snapshot);
    let non_finite = project_snapshot(&snapshot, &mut scratch);
    assert_eq!(non_finite.status, FrameStatus::TooSmall);
    assert!(non_finite.panels.is_empty());
}

#[test]
fn paint_order_and_visibility_follow_each_mode() {
    let floating_snapshot = semantic_snapshot(Mode::Floating);
    let mut floating_scratch = scratch_for(&floating_snapshot);
    let floating = project_snapshot(&floating_snapshot, &mut floating_scratch);
    assert_eq!(
        keys(floating.panels),
        vec![TestPanel::Beta, TestPanel::Alpha, TestPanel::Gamma]
    );
    assert_eq!(
        floating
            .dock
            .iter()
            .map(|entry| entry.key)
            .collect::<Vec<_>>(),
        vec![TestPanel::Docked]
    );

    let tiling_snapshot = semantic_snapshot(Mode::Tiling);
    let mut tiling_scratch = scratch_for(&tiling_snapshot);
    let tiling = project_snapshot(&tiling_snapshot, &mut tiling_scratch);
    assert_eq!(
        keys(tiling.panels),
        vec![TestPanel::Alpha, TestPanel::Beta, TestPanel::Gamma]
    );
    assert!(matches!(
        tiling.panels[1].placement,
        Placement::Tiled {
            column: 0,
            row: 2,
            column_span: 2,
            row_span: 1
        }
    ));

    let mut maxed = semantic_snapshot(Mode::Floating);
    maxed.panels[0].state = WinState::Maximized;
    maxed.panels[2].state = WinState::Maximized;
    let mut maxed_scratch = scratch_for(&maxed);
    let maximized = project_snapshot(&maxed, &mut maxed_scratch);
    assert_eq!(keys(maximized.panels), vec![TestPanel::Gamma]);
    assert_eq!(maximized.panels[0].placement, Placement::Maximized);
}

#[test]
fn hit_test_resolves_body_and_controls_with_fixed_vectors() {
    let snapshot = semantic_snapshot(Mode::Floating);
    let mut scratch = scratch_for(&snapshot);
    let frame = project_snapshot(&snapshot, &mut scratch);
    let gamma = frame
        .panels
        .iter()
        .find(|panel| panel.key == TestPanel::Gamma)
        .unwrap();

    assert_eq!(
        hit_test(&frame, center(gamma.chrome.body)),
        Some(HitTarget::Panel {
            key: TestPanel::Gamma,
            part: PanelPart::Surface
        })
    );
    assert_eq!(
        hit_test(&frame, center(gamma.chrome.header_hit)),
        Some(HitTarget::Panel {
            key: TestPanel::Gamma,
            part: PanelPart::Header
        })
    );
    assert_eq!(
        hit_test(&frame, center(gamma.chrome.minimize_hit.unwrap())),
        Some(HitTarget::Panel {
            key: TestPanel::Gamma,
            part: PanelPart::MinimizeControl
        })
    );
    assert_eq!(
        hit_test(&frame, center(gamma.chrome.maximize_hit.unwrap())),
        Some(HitTarget::Panel {
            key: TestPanel::Gamma,
            part: PanelPart::MaximizeControl
        })
    );
    assert_eq!(
        hit_test(&frame, center(gamma.chrome.resize_hit.unwrap())),
        Some(HitTarget::Panel {
            key: TestPanel::Gamma,
            part: PanelPart::ResizeGrip
        })
    );
    assert_eq!(
        hit_test(
            &frame,
            (
                frame.chrome.workspace.x + 1.0,
                frame.chrome.workspace.y + 1.0
            )
        ),
        Some(HitTarget::Workspace)
    );
}

#[test]
fn project_panel_requires_no_snapshot_catalog_store_or_dock() {
    let snapshot = semantic_snapshot(Mode::Floating);
    let panel = snapshot.panels[2];
    let chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
    let input = PanelProjectionInput {
        viewport: Region::new(0.0, 0.0, 800.0, 570.0),
        preferred_mode: Mode::Floating,
        surface: super::fixtures::regular_surface(),
        focused: true,
        pointer_dragging: true,
        tile_dragging: false,
        workspace_scroll: 25.0,
        clamp: &Clamp::WEB,
        chrome: &chrome,
        tile_grid: None,
        tiled_origin: None,
    };

    let projected = project_panel(&panel, 2, input).expect("floating panel should project");

    assert_eq!(projected.source_index, 2);
    assert_eq!(projected.key, TestPanel::Gamma);
    assert!(projected.focused);
    assert!(projected.pointer_dragging);
    assert!(!projected.tile_dragging);
    assert_eq!(projected.state, WinState::Floating);
    assert_eq!(projected.placement, Placement::Floating);
}

#[test]
fn standalone_surface_has_no_implicit_chrome() {
    let snapshot = semantic_snapshot(Mode::Floating);
    let chrome = ChromeProjectionInput::surface_only(ChromeMetrics::WEB);
    let viewport = Viewport {
        width: 800.0,
        height: 600.0,
        units: crate::Units::CssPx,
    };
    let workspace_chrome = project_chrome(viewport, &chrome);
    let input = PanelProjectionInput {
        viewport: workspace_chrome.workspace,
        preferred_mode: Mode::Floating,
        surface: super::fixtures::regular_surface(),
        focused: false,
        pointer_dragging: false,
        tile_dragging: false,
        workspace_scroll: 0.0,
        clamp: &Clamp::WEB,
        chrome: &chrome,
        tile_grid: None,
        tiled_origin: None,
    };
    let projected = project_panel(&snapshot.panels[0], 0, input).unwrap();
    let mut dock = Vec::new();
    project_dock_into(&snapshot.panels, workspace_chrome.dock, &mut dock);

    assert_eq!(workspace_chrome.workspace, workspace_chrome.root);
    assert_eq!(workspace_chrome.dock.h, 0.0);
    assert_eq!(projected.chrome.body, projected.chrome.outer);
    assert_eq!(projected.chrome.header_hit.h, 0.0);
    assert!(projected.chrome.mode_hit.is_none());
    assert!(projected.chrome.minimize_hit.is_none());
    assert!(projected.chrome.maximize_hit.is_none());
    assert!(projected.chrome.resize_hit.is_none());
    assert!(dock.is_empty());
}

#[test]
fn partial_projectors_match_full_frame_regions() {
    let floating_snapshot = semantic_snapshot(Mode::Floating);
    let mut floating_scratch = scratch_for(&floating_snapshot);
    let floating_frame = project_snapshot(&floating_snapshot, &mut floating_scratch);
    let floating_panel = floating_frame
        .panels
        .iter()
        .find(|panel| panel.key == TestPanel::Gamma)
        .unwrap();
    let floating_chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
    let floating_partial = project_panel(
        &floating_snapshot.panels[floating_panel.source_index],
        floating_panel.source_index,
        PanelProjectionInput {
            viewport: floating_frame.chrome.workspace,
            preferred_mode: floating_snapshot.preferred_mode,
            surface: floating_frame.surface,
            focused: floating_panel.focused,
            pointer_dragging: floating_panel.pointer_dragging,
            tile_dragging: floating_panel.tile_dragging,
            workspace_scroll: floating_frame.workspace_scroll,
            clamp: &Clamp::WEB,
            chrome: &floating_chrome,
            tile_grid: floating_frame.tile_grid.as_ref(),
            tiled_origin: None,
        },
    )
    .unwrap();
    assert_eq!(floating_partial, *floating_panel);

    let tiling_snapshot = semantic_snapshot(Mode::Tiling);
    let mut tiling_scratch = scratch_for(&tiling_snapshot);
    let tiling_frame = project_snapshot(&tiling_snapshot, &mut tiling_scratch);
    let tiled_panel = tiling_frame
        .panels
        .iter()
        .find(|panel| panel.key == TestPanel::Beta)
        .unwrap();
    let Placement::Tiled { column, row, .. } = tiled_panel.placement else {
        panic!("expected tiled placement");
    };
    let tiling_chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
    let tiling_partial = project_panel(
        &tiling_snapshot.panels[tiled_panel.source_index],
        tiled_panel.source_index,
        PanelProjectionInput {
            viewport: tiling_frame.chrome.workspace,
            preferred_mode: tiling_snapshot.preferred_mode,
            surface: tiling_frame.surface,
            focused: tiled_panel.focused,
            pointer_dragging: tiled_panel.pointer_dragging,
            tile_dragging: tiled_panel.tile_dragging,
            workspace_scroll: tiling_frame.workspace_scroll,
            clamp: &Clamp::WEB,
            chrome: &tiling_chrome,
            tile_grid: tiling_frame.tile_grid.as_ref(),
            tiled_origin: Some((column, row)),
        },
    )
    .unwrap();
    assert_eq!(tiling_partial, *tiled_panel);
}
