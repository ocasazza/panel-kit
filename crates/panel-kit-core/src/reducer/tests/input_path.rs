use super::fixtures::{default_snapshot, web_context, TestPanel};
use crate::reducer::{reduce, ChangePhase, HitTarget, PanelPart, Reduction, WorkspaceEvent};
use crate::{Mode, PointerButton, PointerEvent, PointerEventKind};

#[test]
fn reducer_replays_move_resize_reorder_and_settle_phases() {
    let mut snapshot = default_snapshot();
    let down = reduce(
        &mut snapshot,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: TestPanel::First,
                part: PanelPart::Header,
            },
            event: pointer_down(30.0, 35.0),
        },
        web_context(),
    );
    assert_eq!(down.phase, Some(ChangePhase::Continuous));
    assert_eq!(snapshot.focused, Some(TestPanel::First));
    assert!(snapshot.drag.is_some());

    let moved = reduce(
        &mut snapshot,
        WorkspaceEvent::Pointer {
            target: HitTarget::Workspace,
            event: pointer_drag(70.0, 80.0),
        },
        web_context(),
    );
    assert_eq!(moved.phase, Some(ChangePhase::Continuous));
    assert_eq!(snapshot.panels[0].x, 60.0);
    assert_eq!(snapshot.panels[0].y, 65.0);

    let up = reduce(
        &mut snapshot,
        WorkspaceEvent::Pointer {
            target: HitTarget::Workspace,
            event: pointer_up(70.0, 80.0),
        },
        web_context(),
    );
    assert_eq!(up.phase, Some(ChangePhase::Settled));
    assert_eq!(snapshot.drag, None);

    let mut tiling = default_snapshot();
    tiling.preferred_mode = Mode::Tiling;
    let start_reorder = reduce(
        &mut tiling,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: TestPanel::First,
                part: PanelPart::Header,
            },
            event: pointer_down(10.0, 10.0),
        },
        web_context(),
    );
    assert_eq!(start_reorder.phase, Some(ChangePhase::Continuous));
    assert_eq!(tiling.tile_drag, Some(TestPanel::First));

    let hover_reorder = reduce(
        &mut tiling,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: TestPanel::Third,
                part: PanelPart::Surface,
            },
            event: pointer_moved(140.0, 160.0),
        },
        web_context(),
    );
    assert_eq!(hover_reorder.phase, Some(ChangePhase::Continuous));
    assert_eq!(
        tiling.panels.iter().map(|panel| panel.kind).collect::<Vec<_>>(),
        vec![TestPanel::Second, TestPanel::Third, TestPanel::First]
    );

    let reorder_up = reduce(
        &mut tiling,
        WorkspaceEvent::Pointer {
            target: HitTarget::Workspace,
            event: pointer_up(140.0, 160.0),
        },
        web_context(),
    );
    assert_eq!(reorder_up.phase, Some(ChangePhase::Settled));
    assert_eq!(tiling.tile_drag, None);
}

#[test]
fn invalid_pointer_targets_are_noops_not_panics() {
    let mut snapshot = default_snapshot();
    let before = snapshot.clone();
    let reduction = reduce(
        &mut snapshot,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: TestPanel::Gone,
                part: PanelPart::Header,
            },
            event: pointer_down(1.0, 1.0),
        },
        web_context(),
    );
    assert_eq!(reduction, Reduction::unchanged());
    assert!(snapshot == before);

    let mut snapshot = default_snapshot();
    snapshot.preferred_mode = Mode::Tiling;
    snapshot.tile_drag = Some(TestPanel::Gone);
    let before = snapshot.clone();
    let reduction = reduce(
        &mut snapshot,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: TestPanel::Second,
                part: PanelPart::Surface,
            },
            event: pointer_moved(80.0, 90.0),
        },
        web_context(),
    );
    assert_eq!(reduction, Reduction::unchanged());
    assert!(snapshot == before);
}

fn pointer_down(x: f64, y: f64) -> PointerEvent {
    pointer(PointerEventKind::Down(PointerButton::Primary), x, y)
}

fn pointer_drag(x: f64, y: f64) -> PointerEvent {
    pointer(PointerEventKind::Drag(PointerButton::Primary), x, y)
}

fn pointer_up(x: f64, y: f64) -> PointerEvent {
    pointer(PointerEventKind::Up(PointerButton::Primary), x, y)
}

fn pointer_moved(x: f64, y: f64) -> PointerEvent {
    pointer(PointerEventKind::Moved, x, y)
}

fn pointer(kind: PointerEventKind, x: f64, y: f64) -> PointerEvent {
    PointerEvent { kind, x, y }
}
