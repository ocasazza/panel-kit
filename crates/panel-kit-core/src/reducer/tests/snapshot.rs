use super::fixtures::{panels, web_viewport};
use crate::reducer::Snapshot;
use crate::Mode;

#[test]
fn snapshot_from_defaults_matches_controller_seeding() {
    // Both controllers seed fresh state with the given panels, mode, and
    // viewport — and nothing in flight: no focus, no drags, scroll pinned to
    // the top.
    let snapshot = Snapshot::from_defaults(panels(), Mode::Tiling, web_viewport());

    assert_eq!(snapshot.panels.len(), 3);
    assert_eq!(snapshot.preferred_mode, Mode::Tiling);
    assert_eq!(snapshot.viewport, web_viewport());
    assert_eq!(snapshot.focused, None);
    assert_eq!(snapshot.drag, None);
    assert_eq!(snapshot.tile_drag, None);
    assert_eq!(snapshot.workspace_scroll, 0.0);
}
