use super::fixtures::{default_snapshot, web_context, TestPanel};
use crate::reducer::{
    reduce, ChangePhase, Reduction, ResizePolicy, WheelDisposition, WorkspaceEvent,
};

#[test]
fn reducer_preserves_wheel_precedence() {
    let mut consumed = scrollable_snapshot();
    let before = consumed.clone();
    let consumed_reduction = reduce(
        &mut consumed,
        WorkspaceEvent::Wheel {
            delta_y: 120.0,
            disposition: WheelDisposition::ContentConsumed,
        },
        web_context(),
    );
    assert_eq!(consumed_reduction, Reduction::unchanged());
    assert!(consumed == before);

    let mut bubbled = scrollable_snapshot();
    let bubbled_reduction = reduce(
        &mut bubbled,
        WorkspaceEvent::Wheel {
            delta_y: 120.0,
            disposition: WheelDisposition::BubbleToWorkspace,
        },
        web_context(),
    );
    assert_eq!(bubbled_reduction.phase, Some(ChangePhase::Settled));
    assert_eq!(bubbled.workspace_scroll, 120.0);

    let mut pinned = default_snapshot();
    let pinned_reduction = reduce(
        &mut pinned,
        WorkspaceEvent::Wheel {
            delta_y: 120.0,
            disposition: WheelDisposition::BubbleToWorkspace,
        },
        web_context(),
    );
    assert_eq!(pinned_reduction, Reduction::unchanged());
}

#[test]
fn viewport_resize_policy_is_explicit() {
    let mut scaled = default_snapshot();
    let scaled_reduction = reduce(
        &mut scaled,
        WorkspaceEvent::ViewportChanged {
            size: viewport(500.0, 400.0),
            policy: ResizePolicy::ScaleFloating,
        },
        web_context(),
    );
    assert_eq!(scaled_reduction.phase, Some(ChangePhase::Settled));
    assert_eq!(scaled.viewport, viewport(500.0, 400.0));
    assert_eq!((scaled.panels[0].x, scaled.panels[0].y), (10.0, 10.0));
    assert_eq!((scaled.panels[0].w, scaled.panels[0].h), (150.0, 100.0));

    let mut preserved = default_snapshot();
    let panels_before = preserved.panels.clone();
    let preserved_reduction = reduce(
        &mut preserved,
        WorkspaceEvent::ViewportChanged {
            size: viewport(500.0, 400.0),
            policy: ResizePolicy::PreserveIntent,
        },
        web_context(),
    );
    assert_eq!(preserved_reduction.phase, Some(ChangePhase::Settled));
    assert_eq!(preserved.viewport, viewport(500.0, 400.0));
    assert!(preserved.panels == panels_before);

    for rejected in [
        viewport(0.0, 400.0),
        viewport(-1.0, 400.0),
        viewport(f64::NAN, 400.0),
        viewport(500.0, f64::INFINITY),
    ] {
        let mut snapshot = default_snapshot();
        let before = snapshot.clone();
        let reduction = reduce(
            &mut snapshot,
            WorkspaceEvent::ViewportChanged {
                size: rejected,
                policy: ResizePolicy::ScaleFloating,
            },
            web_context(),
        );
        assert_eq!(reduction, Reduction::unchanged(), "{rejected:?}");
        assert!(snapshot == before, "{rejected:?}");
    }
}

fn scrollable_snapshot() -> crate::reducer::Snapshot<TestPanel> {
    let mut snapshot = default_snapshot();
    snapshot.panels[2].y = 1_200.0;
    snapshot.panels[2].h = 300.0;
    snapshot
}

fn viewport(width: f64, height: f64) -> crate::reducer::Viewport {
    crate::reducer::Viewport {
        width,
        height,
        units: crate::Units::CssPx,
    }
}
