use super::fixtures::{chord, default_snapshot, web_context, TestPanel};
use crate::reducer::{
    reduce, ChangePhase, HitTarget, PanelPart, ReduceContext, Reduction, Snapshot, WorkspaceEvent,
};
use crate::tile_drag::{TileDragPhase, LAND_MS, LIFT_MS, LIFT_THRESHOLD_PX};
use crate::{
    FocusContext, Key, Mode, PanelCommand, PointerButton, PointerEvent, PointerEventKind,
    SurfaceCapabilities, SurfaceProfile, WinState, WEB_COMPACT_MAX, WEB_TABLET_MAX,
};

const PRESS: (f64, f64) = (10.0, 10.0);

fn tiling() -> Snapshot<TestPanel> {
    let mut snapshot = default_snapshot();
    snapshot.preferred_mode = Mode::Tiling;
    snapshot
}

fn order(snapshot: &Snapshot<TestPanel>) -> Vec<TestPanel> {
    snapshot.panels.iter().map(|panel| panel.kind).collect()
}

fn pointer(
    snapshot: &mut Snapshot<TestPanel>,
    target: HitTarget<TestPanel>,
    kind: PointerEventKind,
    (x, y): (f64, f64),
) -> Reduction<TestPanel> {
    reduce(
        snapshot,
        WorkspaceEvent::Pointer {
            target,
            event: PointerEvent { kind, x, y },
        },
        web_context(),
    )
}

fn surface(key: TestPanel) -> HitTarget<TestPanel> {
    HitTarget::Panel {
        key,
        part: PanelPart::Surface,
    }
}

fn press_header(snapshot: &mut Snapshot<TestPanel>, key: TestPanel) -> Reduction<TestPanel> {
    pointer(
        snapshot,
        HitTarget::Panel {
            key,
            part: PanelPart::Header,
        },
        PointerEventKind::Down(PointerButton::Primary),
        PRESS,
    )
}

fn drag_over(
    snapshot: &mut Snapshot<TestPanel>,
    target: HitTarget<TestPanel>,
    at: (f64, f64),
) -> Reduction<TestPanel> {
    pointer(
        snapshot,
        target,
        PointerEventKind::Drag(PointerButton::Primary),
        at,
    )
}

fn release(snapshot: &mut Snapshot<TestPanel>, at: (f64, f64)) -> Reduction<TestPanel> {
    pointer(
        snapshot,
        HitTarget::Workspace,
        PointerEventKind::Up(PointerButton::Primary),
        at,
    )
}

fn tick(snapshot: &mut Snapshot<TestPanel>, dt_ms: f64) -> Reduction<TestPanel> {
    reduce(snapshot, WorkspaceEvent::Tick { dt_ms }, web_context())
}

/// Press First's header and drag it over Third far enough to lift.
fn lifted_over_third() -> Snapshot<TestPanel> {
    let mut snapshot = tiling();
    press_header(&mut snapshot, TestPanel::First);
    drag_over(&mut snapshot, surface(TestPanel::Third), (400.0, 300.0));
    snapshot
}

fn phase(snapshot: &Snapshot<TestPanel>) -> Option<TileDragPhase> {
    snapshot.tile_drag.map(|tile| tile.phase)
}

#[test]
fn motion_under_the_lift_threshold_neither_lifts_nor_targets() {
    let mut snapshot = tiling();
    press_header(&mut snapshot, TestPanel::First);
    let short = (
        PRESS.0 + LIFT_THRESHOLD_PX - 1.0,
        PRESS.1 + LIFT_THRESHOLD_PX - 1.0,
    );
    drag_over(&mut snapshot, surface(TestPanel::Third), short);

    assert_eq!(phase(&snapshot), Some(TileDragPhase::Pressed));
    assert_eq!(snapshot.tile_drag.and_then(|tile| tile.target), None);

    let click = release(&mut snapshot, short);
    assert_eq!(click.phase, Some(ChangePhase::Settled));
    assert_eq!(snapshot.tile_drag, None);
    assert_eq!(
        order(&snapshot),
        [TestPanel::First, TestPanel::Second, TestPanel::Third]
    );
}

#[test]
fn crossing_the_threshold_lifts_and_targets_the_hit_slot() {
    let mut snapshot = tiling();
    press_header(&mut snapshot, TestPanel::First);
    let lifted = drag_over(
        &mut snapshot,
        surface(TestPanel::Third),
        (PRESS.0 + LIFT_THRESHOLD_PX, PRESS.1),
    );

    assert_eq!(lifted.phase, Some(ChangePhase::Transient));
    assert_eq!(phase(&snapshot), Some(TileDragPhase::Lifted));
    assert_eq!(
        snapshot.tile_drag.and_then(|tile| tile.target),
        Some(TestPanel::Third)
    );
}

#[test]
fn drop_swaps_the_dragged_and_target_positions_only() {
    let mut snapshot = lifted_over_third();
    assert_eq!(
        order(&snapshot),
        [TestPanel::First, TestPanel::Second, TestPanel::Third],
        "a lift never reorders the committed layout"
    );

    let drop = release(&mut snapshot, (400.0, 300.0));

    assert_eq!(drop.phase, Some(ChangePhase::Settled));
    assert_eq!(
        order(&snapshot),
        [TestPanel::Third, TestPanel::Second, TestPanel::First]
    );
    assert_eq!(phase(&snapshot), Some(TileDragPhase::Settling));
}

#[test]
fn hovering_the_dragged_tiles_own_slot_drops_it_home() {
    let mut snapshot = lifted_over_third();
    drag_over(&mut snapshot, surface(TestPanel::First), (12.0, 30.0));
    assert_eq!(snapshot.tile_drag.and_then(|tile| tile.target), None);
    assert_eq!(
        snapshot.tile_drag.and_then(|tile| tile.previous_target),
        Some(TestPanel::Third)
    );

    let drop = release(&mut snapshot, (12.0, 30.0));

    assert_eq!(
        drop.phase,
        Some(ChangePhase::Transient),
        "a home drop persists nothing"
    );
    assert_eq!(
        order(&snapshot),
        [TestPanel::First, TestPanel::Second, TestPanel::Third]
    );
    assert_eq!(phase(&snapshot), Some(TileDragPhase::Settling));
}

#[test]
fn unresolved_hits_keep_the_current_target() {
    let mut snapshot = lifted_over_third();
    drag_over(&mut snapshot, HitTarget::Workspace, (420.0, 310.0));
    assert_eq!(
        snapshot.tile_drag.and_then(|tile| tile.target),
        Some(TestPanel::Third)
    );

    snapshot.panels[1].state = WinState::Minimized;
    drag_over(&mut snapshot, surface(TestPanel::Second), (430.0, 320.0));
    assert_eq!(
        snapshot.tile_drag.and_then(|tile| tile.target),
        Some(TestPanel::Third),
        "a panel without a slot is never a swap target"
    );
}

#[test]
fn escape_cancels_a_lifted_drag_and_is_not_a_restore() {
    let mut snapshot = lifted_over_third();
    snapshot.panels[1].state = WinState::Minimized;
    snapshot.focused = Some(TestPanel::Second);

    let escape = reduce(
        &mut snapshot,
        WorkspaceEvent::Key {
            chord: chord(Key::Escape, false, false),
            focus: FocusContext::Panel(TestPanel::Second),
        },
        web_context(),
    );

    assert_eq!(escape.phase, Some(ChangePhase::Transient));
    assert_eq!(phase(&snapshot), Some(TileDragPhase::Cancelling));
    assert_eq!(snapshot.tile_drag.and_then(|tile| tile.target), None);
    assert_eq!(
        snapshot.panels[1].state,
        WinState::Minimized,
        "Escape went to the drag, not to the focused panel's restore"
    );

    release(&mut snapshot, (400.0, 300.0));
    assert_eq!(
        order(&snapshot),
        [TestPanel::First, TestPanel::Second, TestPanel::Third]
    );
}

#[test]
fn pointer_cancel_flies_a_lifted_tile_home_and_drops_a_held_one() {
    let mut lifted = lifted_over_third();
    pointer(
        &mut lifted,
        HitTarget::Workspace,
        PointerEventKind::Cancel,
        (0.0, 0.0),
    );
    assert_eq!(phase(&lifted), Some(TileDragPhase::Cancelling));
    release(&mut lifted, (400.0, 300.0));
    assert_eq!(
        order(&lifted),
        [TestPanel::First, TestPanel::Second, TestPanel::Third]
    );

    let mut held = tiling();
    press_header(&mut held, TestPanel::First);
    let cancel = pointer(
        &mut held,
        HitTarget::Workspace,
        PointerEventKind::Cancel,
        PRESS,
    );
    assert_eq!(cancel.phase, Some(ChangePhase::Transient));
    assert_eq!(held.tile_drag, None);
}

#[test]
fn ticks_advance_host_time_and_a_finished_landing_ends_the_drag() {
    let mut snapshot = lifted_over_third();
    assert_eq!(tick(&mut snapshot, f64::NAN), Reduction::unchanged());
    assert_eq!(tick(&mut snapshot, -5.0), Reduction::unchanged());

    tick(&mut snapshot, LIFT_MS / 2.0);
    let lift = snapshot.tile_drag.map(|tile| tile.lift).unwrap();
    assert!((lift - 0.5).abs() < 1e-9, "lift progress {lift}");

    release(&mut snapshot, (400.0, 300.0));
    tick(&mut snapshot, LAND_MS / 2.0);
    assert_eq!(phase(&snapshot), Some(TileDragPhase::Settling));
    let done = tick(&mut snapshot, LAND_MS / 2.0);
    assert_eq!(done.phase, Some(ChangePhase::Transient));
    assert_eq!(snapshot.tile_drag, None);
    assert_eq!(
        order(&snapshot),
        [TestPanel::Third, TestPanel::Second, TestPanel::First]
    );
}

#[test]
fn an_infinite_tick_completes_any_animation_at_once() {
    let mut snapshot = lifted_over_third();
    tick(&mut snapshot, f64::INFINITY);
    let tile = snapshot.tile_drag.unwrap();
    assert_eq!((tile.lift, tile.retarget), (1.0, 1.0));
    assert!(!tile.animating());

    release(&mut snapshot, (400.0, 300.0));
    tick(&mut snapshot, f64::INFINITY);
    assert_eq!(snapshot.tile_drag, None);
}

#[test]
fn grip_and_body_presses_never_start_a_tile_drag() {
    let mut grip = tiling();
    pointer(
        &mut grip,
        HitTarget::Panel {
            key: TestPanel::First,
            part: PanelPart::ResizeGrip,
        },
        PointerEventKind::Down(PointerButton::Primary),
        PRESS,
    );
    assert!(grip.drag.is_some(), "the grip owns a tile resize");
    assert_eq!(grip.tile_drag, None);

    let mut body = tiling();
    let z_before = body.panels.iter().map(|panel| panel.z).collect::<Vec<_>>();
    pointer(
        &mut body,
        surface(TestPanel::First),
        PointerEventKind::Down(PointerButton::Primary),
        PRESS,
    );
    assert_eq!(body.focused, Some(TestPanel::First));
    assert_eq!(body.tile_drag, None);
    assert_eq!(
        body.panels.iter().map(|panel| panel.z).collect::<Vec<_>>(),
        z_before
    );
}

#[test]
fn maximized_panels_and_compact_surfaces_do_not_lift() {
    let mut maximized = tiling();
    maximized.panels[0].state = WinState::Maximized;
    press_header(&mut maximized, TestPanel::First);
    assert_eq!(maximized.focused, Some(TestPanel::First));
    assert_eq!(maximized.tile_drag, None);

    let compact = ReduceContext {
        surface: SurfaceProfile::from_logical_width(
            400.0,
            WEB_COMPACT_MAX,
            WEB_TABLET_MAX,
            SurfaceCapabilities {
                coarse_pointer: true,
                hover: false,
                keyboard: false,
            },
        ),
        ..web_context()
    };
    let mut phone = tiling();
    reduce(
        &mut phone,
        WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: TestPanel::First,
                part: PanelPart::Header,
            },
            event: PointerEvent {
                kind: PointerEventKind::Down(PointerButton::Primary),
                x: PRESS.0,
                y: PRESS.1,
            },
        },
        compact,
    );
    assert_eq!(phone.tile_drag, None);
}

#[test]
fn a_second_press_while_lifted_is_ignored() {
    let mut snapshot = lifted_over_third();
    let before = snapshot.clone();
    let second = press_header(&mut snapshot, TestPanel::Second);
    assert_eq!(second, Reduction::unchanged());
    assert!(snapshot == before);
}

#[test]
fn a_press_during_landing_finishes_it_and_starts_a_new_drag() {
    let mut snapshot = lifted_over_third();
    release(&mut snapshot, (400.0, 300.0));
    assert_eq!(phase(&snapshot), Some(TileDragPhase::Settling));

    press_header(&mut snapshot, TestPanel::Second);

    let tile = snapshot.tile_drag.unwrap();
    assert_eq!(
        (tile.key, tile.phase),
        (TestPanel::Second, TileDragPhase::Pressed)
    );
    assert_eq!(
        order(&snapshot),
        [TestPanel::Third, TestPanel::Second, TestPanel::First]
    );
}

#[test]
fn a_command_ends_the_drag_without_a_swap() {
    let mut snapshot = lifted_over_third();
    let minimize = reduce(
        &mut snapshot,
        WorkspaceEvent::Command {
            target: Some(TestPanel::First),
            command: PanelCommand::Minimize,
        },
        web_context(),
    );
    assert_eq!(minimize.phase, Some(ChangePhase::Settled));
    assert_eq!(snapshot.tile_drag, None);
    assert_eq!(
        order(&snapshot),
        [TestPanel::First, TestPanel::Second, TestPanel::Third]
    );
    assert_eq!(snapshot.panels[0].state, WinState::Minimized);
}
