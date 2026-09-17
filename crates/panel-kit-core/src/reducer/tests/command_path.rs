use super::fixtures::{default_snapshot, web_context, TestPanel};
use crate::reducer::{reduce, ChangePhase, HitTarget, PanelPart, Reduction, WorkspaceEvent};
use crate::{
    apply_command, apply_drag, begin_drag, begin_tile_resize, reorder_tile, restore, Clamp,
    CommandStep, DragKind, Mode, PanelCommand, PointerButton, PointerEvent, PointerEventKind,
    SnapPolicy, TileMetrics, WinState,
};

/// CK-26: reduced commands and pointer gestures must equal the controllers'
/// direct-call handoff. The reducer delegates to the same shared free
/// functions instead of owning a second transition implementation.
#[test]
fn reducer_reuses_existing_command_and_pointer_transitions() {
    let cases = [
        (None, PanelCommand::Move { dx: -16.0, dy: 0.0 }),
        (None, PanelCommand::FocusNext),
        (None, PanelCommand::Minimize),
        (Some(TestPanel::Third), PanelCommand::Maximize),
        (Some(TestPanel::First), PanelCommand::ToggleMode),
        (Some(TestPanel::Third), PanelCommand::Restore),
    ];

    for (target, command) in cases {
        let mut reduced = default_snapshot();
        reduced.focused = Some(TestPanel::Second);
        // Third starts maximized so the targeted Restore has work to do.
        reduced.panels[2].state = WinState::Maximized;
        let mut direct = reduced.clone();

        let reduction = reduce(
            &mut reduced,
            WorkspaceEvent::Command { target, command },
            web_context(),
        );

        let mut focused = target.or(direct.focused);
        apply_command(
            &mut direct.panels,
            &mut direct.preferred_mode,
            &mut focused,
            command,
            Clamp::WEB,
            CommandStep::WEB,
            direct.viewport.width,
            direct.viewport.height,
        );
        direct.focused = focused;

        assert!(
            reduced.panels == direct.panels,
            "panels diverged for {command:?} on {target:?}"
        );
        assert_eq!(reduced.preferred_mode, direct.preferred_mode);
        assert_eq!(reduced.focused, direct.focused);
        assert!(reduction.changed, "{command:?} on {target:?}");
        assert_eq!(reduction.phase, Some(crate::reducer::ChangePhase::Settled));
    }

    assert_pointer_transition_delegation();
}

fn assert_pointer_transition_delegation() {
    let mut reduced = default_snapshot();
    let mut direct = reduced.clone();
    let start = reduce(
        &mut reduced,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: TestPanel::First,
                part: PanelPart::Header,
            },
            event: pointer(PointerEventKind::Down(PointerButton::Primary), 30.0, 35.0),
        },
        web_context(),
    );
    // The floating pointer-down transition raises the panel to the front
    // before the drag begins; the direct hand-roll mirrors that step.
    let front = crate::front_z(&direct.panels);
    direct.panels[0].z = front;
    direct.drag = begin_drag(
        &mut direct.panels,
        0,
        DragKind::Move,
        30.0,
        35.0,
        direct.viewport.width,
        direct.viewport.height,
        &Clamp::WEB,
    );
    direct.focused = Some(TestPanel::First);
    assert_eq!(start.phase, Some(ChangePhase::Continuous));
    assert!(reduced == direct);

    let moved = reduce(
        &mut reduced,
        WorkspaceEvent::Pointer {
            target: HitTarget::Workspace,
            event: pointer(PointerEventKind::Drag(PointerButton::Primary), 50.0, 70.0),
        },
        web_context(),
    );
    apply_drag(
        &mut direct.panels,
        &direct.drag.unwrap(),
        50.0,
        70.0,
        false,
        SnapPolicy::default(),
        direct.viewport.width,
        &Clamp::WEB,
        &TileMetrics::WEB,
    );
    assert_eq!(moved.phase, Some(ChangePhase::Continuous));
    assert!(reduced == direct);

    let mut reduced_resize = default_snapshot();
    reduced_resize.preferred_mode = Mode::Tiling;
    let mut direct_resize = reduced_resize.clone();
    let resize = reduce(
        &mut reduced_resize,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: TestPanel::Second,
                part: PanelPart::ResizeGrip,
            },
            event: pointer(PointerEventKind::Down(PointerButton::Primary), 320.0, 320.0),
        },
        web_context(),
    );
    direct_resize.drag = begin_tile_resize(&direct_resize.panels, 1, 320.0, 320.0);
    direct_resize.focused = Some(TestPanel::Second);
    assert_eq!(resize.phase, Some(ChangePhase::Continuous));
    assert!(reduced_resize == direct_resize);

    let mut reduced_reorder = default_snapshot();
    reduced_reorder.preferred_mode = Mode::Tiling;
    reduced_reorder.tile_drag = Some(TestPanel::First);
    let mut direct_reorder = reduced_reorder.clone();
    let reorder = reduce(
        &mut reduced_reorder,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: TestPanel::Third,
                part: PanelPart::Surface,
            },
            event: pointer(PointerEventKind::Moved, 140.0, 160.0),
        },
        web_context(),
    );
    reorder_tile(
        &mut direct_reorder.panels,
        TestPanel::First,
        TestPanel::Third,
    );
    assert_eq!(reorder.phase, Some(ChangePhase::Continuous));
    assert!(reduced_reorder == direct_reorder);

    let mut reduced_restore = default_snapshot();
    reduced_restore.panels[1].state = WinState::Minimized;
    let mut direct_restore = reduced_restore.clone();
    let dock = reduce(
        &mut reduced_restore,
        WorkspaceEvent::Pointer {
            target: HitTarget::Dock {
                key: TestPanel::Second,
            },
            event: pointer(PointerEventKind::Down(PointerButton::Primary), 0.0, 0.0),
        },
        web_context(),
    );
    restore(&mut direct_restore.panels, TestPanel::Second);
    direct_restore.focused = Some(TestPanel::Second);
    assert_eq!(dock.phase, Some(ChangePhase::Settled));
    assert_eq!(dock.focus_request, Some(TestPanel::Second));
    assert!(reduced_restore == direct_restore);
}

fn pointer(kind: PointerEventKind, x: f64, y: f64) -> PointerEvent {
    PointerEvent { kind, x, y }
}

#[test]
fn no_op_command_reports_unchanged() {
    let mut snapshot = default_snapshot();
    let reduction = reduce(
        &mut snapshot,
        WorkspaceEvent::Command {
            target: None,
            command: PanelCommand::Move { dx: -16.0, dy: 0.0 },
        },
        web_context(),
    );
    assert_eq!(reduction, Reduction::unchanged());
    assert!(snapshot == default_snapshot());

    let mut snapshot = default_snapshot();
    snapshot.focused = Some(TestPanel::Second);
    let reduction = reduce(
        &mut snapshot,
        WorkspaceEvent::Command {
            target: None,
            command: PanelCommand::Restore,
        },
        web_context(),
    );
    assert_eq!(reduction, Reduction::unchanged());
}

#[test]
fn invalid_command_targets_are_noops_not_panics() {
    // A target absent from the layout is rejected at the reducer boundary: no
    // panic, no state change, `focused` left untouched.
    let mut snapshot = default_snapshot();
    snapshot.focused = Some(TestPanel::Second);
    let before = snapshot.clone();

    let reduction = reduce(
        &mut snapshot,
        WorkspaceEvent::Command {
            target: Some(TestPanel::Gone),
            command: PanelCommand::Minimize,
        },
        web_context(),
    );

    assert_eq!(reduction, Reduction::unchanged());
    assert!(snapshot == before);
}
