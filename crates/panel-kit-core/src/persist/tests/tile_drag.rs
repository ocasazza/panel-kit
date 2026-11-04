use super::common::{catalog, defaults, restore_context, MemoryStore, TestPanel};
use crate::persist::{apply_save_decision, restore_snapshot, SavePolicy};
use crate::reducer::{reduce, HitTarget, PanelPart, ReduceContext, Snapshot, WorkspaceEvent};
use crate::{
    Clamp, CommandStep, Mode, PointerButton, PointerEvent, PointerEventKind, SnapPolicy,
    SurfaceCapabilities, SurfaceProfile, TileMetrics, WEB_COMPACT_MAX, WEB_TABLET_MAX,
};

fn context() -> ReduceContext<'static> {
    ReduceContext {
        surface: SurfaceProfile::from_logical_width(
            1000.0,
            WEB_COMPACT_MAX,
            WEB_TABLET_MAX,
            SurfaceCapabilities {
                coarse_pointer: false,
                hover: true,
                keyboard: true,
            },
        ),
        clamp: &Clamp::WEB,
        command_step: CommandStep::WEB,
        tile: &TileMetrics::WEB,
        snap: SnapPolicy::default(),
    }
}

fn pointer(
    target: HitTarget<TestPanel>,
    kind: PointerEventKind,
    x: f64,
) -> WorkspaceEvent<TestPanel> {
    WorkspaceEvent::Pointer {
        target,
        event: PointerEvent { kind, x, y: 10.0 },
    }
}

/// Press First's header, drag over Third, animate, drop, and animate the landing.
fn swap_gesture() -> Vec<WorkspaceEvent<TestPanel>> {
    let header = HitTarget::Panel {
        key: TestPanel::First,
        part: PanelPart::Header,
    };
    let third = HitTarget::Panel {
        key: TestPanel::Third,
        part: PanelPart::Surface,
    };
    let drag = PointerEventKind::Drag(PointerButton::Primary);
    vec![
        pointer(header, PointerEventKind::Down(PointerButton::Primary), 10.0),
        pointer(third, drag, 200.0),
        WorkspaceEvent::Tick { dt_ms: 16.0 },
        pointer(third, drag, 300.0),
        WorkspaceEvent::Tick { dt_ms: 16.0 },
        pointer(
            HitTarget::Workspace,
            PointerEventKind::Up(PointerButton::Primary),
            300.0,
        ),
        WorkspaceEvent::Tick { dt_ms: 100.0 },
        WorkspaceEvent::Tick { dt_ms: 100.0 },
    ]
}

/// Replay the gesture under `policy`, returning every saved record's panel
/// order alongside the index of the event that caused it.
fn saved_orders(policy: SavePolicy) -> (Vec<(usize, Vec<TestPanel>)>, Snapshot<TestPanel>) {
    let store = MemoryStore::empty();
    let mut snapshot = defaults();
    snapshot.preferred_mode = Mode::Tiling;
    let mut saves = Vec::new();
    for (index, event) in swap_gesture().into_iter().enumerate() {
        let reduction = reduce(&mut snapshot, event, context());
        let before = store.save_calls().len();
        apply_save_decision(policy.decide(&reduction), &store, &snapshot, &catalog()).unwrap();
        if store.save_calls().len() > before {
            let restored =
                restore_snapshot(&store, defaults(), &catalog(), restore_context()).unwrap();
            assert_eq!(
                restored.tile_drag, None,
                "a saved record never carries a drag"
            );
            saves.push((
                index,
                restored.panels.iter().map(|panel| panel.kind).collect(),
            ));
        }
    }
    assert_eq!(snapshot.tile_drag, None, "the landing finished");
    (saves, snapshot)
}

#[test]
fn only_the_drop_persists_and_it_persists_the_swap() {
    const DROP: usize = 5;
    let swapped = vec![TestPanel::Third, TestPanel::Second, TestPanel::First];

    let (on_settle, _) = saved_orders(SavePolicy::OnSettle);
    assert_eq!(on_settle, vec![(DROP, swapped.clone())]);

    let (on_change, snapshot) = saved_orders(SavePolicy::OnChange);
    let original = vec![TestPanel::First, TestPanel::Second, TestPanel::Third];
    assert_eq!(
        on_change,
        vec![(0, original), (DROP, swapped.clone())],
        "lift, hover, and animation ticks are transient under every policy"
    );
    assert_eq!(
        snapshot
            .panels
            .iter()
            .map(|panel| panel.kind)
            .collect::<Vec<_>>(),
        swapped
    );
}
