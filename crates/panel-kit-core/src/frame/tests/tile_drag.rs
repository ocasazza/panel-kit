use super::fixtures::{center, project_snapshot, scratch_for, TestPanel};
use crate::frame::{hit_test, PanelProjection, ProjectedFrame};
use crate::reducer::{HitTarget, PanelPart, Snapshot, Viewport};
use crate::tile_drag::{TileDrag, TileDragPhase, TileRole};
use crate::{LayoutBuilder, Mode, Region, Units};

/// Four 1x1 tiles on the two-column fixture surface: Alpha | Beta over
/// Gamma | Docked.
fn grid() -> Snapshot<TestPanel> {
    let mut layout = LayoutBuilder::new();
    let panels = [
        TestPanel::Alpha,
        TestPanel::Beta,
        TestPanel::Gamma,
        TestPanel::Docked,
    ]
    .map(|key| layout.at(key, 0.0, 0.0, 200.0, 150.0).with_tile(1, 1))
    .to_vec();
    Snapshot::from_defaults(
        panels,
        Mode::Tiling,
        Viewport {
            width: 800.0,
            height: 600.0,
            units: Units::CssPx,
        },
    )
}

fn lifted(key: TestPanel, offset: (f64, f64), target: Option<TestPanel>) -> TileDrag<TestPanel> {
    let mut drag = TileDrag::pressed(key, (100.0, 100.0));
    drag.phase = TileDragPhase::Lifted;
    drag.pointer = (100.0 + offset.0, 100.0 + offset.1);
    drag.target = target;
    drag.lift = 1.0;
    drag
}

fn panel(frame: &ProjectedFrame<'_, TestPanel>, key: TestPanel) -> PanelProjection<TestPanel> {
    *frame.panels.iter().find(|panel| panel.key == key).unwrap()
}

fn resting_slots() -> Vec<(TestPanel, Region)> {
    let snapshot = grid();
    let mut scratch = scratch_for(&snapshot);
    let frame = project_snapshot(&snapshot, &mut scratch);
    frame
        .panels
        .iter()
        .map(|panel| (panel.key, panel.region))
        .collect()
}

fn slot_of(key: TestPanel) -> Region {
    resting_slots()
        .into_iter()
        .find(|(candidate, _)| *candidate == key)
        .unwrap()
        .1
}

#[test]
fn a_lifted_tile_follows_the_pointer_from_its_slot_and_draws_on_top() {
    let mut snapshot = grid();
    snapshot.tile_drag = Some(lifted(TestPanel::Alpha, (37.0, 21.0), None));
    let mut scratch = scratch_for(&snapshot);
    let frame = project_snapshot(&snapshot, &mut scratch);

    let alpha = panel(&frame, TestPanel::Alpha);
    let home = slot_of(TestPanel::Alpha);
    assert_eq!(alpha.slot, home);
    assert_eq!(
        alpha.region,
        Region::new(home.x + 37.0, home.y + 21.0, home.w, home.h)
    );
    assert_eq!(alpha.tile_role, TileRole::Lifted);
    assert_eq!(alpha.lift, 1.0);
    assert_eq!(
        frame.panels.last().map(|panel| panel.key),
        Some(TestPanel::Alpha)
    );
    assert!(frame
        .panels
        .iter()
        .all(|other| other.key == TestPanel::Alpha || other.z < alpha.z));
}

#[test]
fn the_target_previews_into_the_dragged_slot_while_others_stay_put() {
    let mut snapshot = grid();
    snapshot.tile_drag = Some(lifted(
        TestPanel::Alpha,
        (300.0, 200.0),
        Some(TestPanel::Docked),
    ));
    let mut scratch = scratch_for(&snapshot);
    let frame = project_snapshot(&snapshot, &mut scratch);

    let docked = panel(&frame, TestPanel::Docked);
    assert_eq!(docked.tile_role, TileRole::Target);
    assert_eq!(docked.slot, slot_of(TestPanel::Docked));
    assert_eq!(docked.region, slot_of(TestPanel::Alpha), "swap preview");
    for key in [TestPanel::Beta, TestPanel::Gamma] {
        assert_eq!(
            panel(&frame, key).region,
            slot_of(key),
            "{key:?} keeps its slot"
        );
    }
}

#[test]
fn displaced_tiles_animate_between_previews_on_host_time() {
    let mut snapshot = grid();
    let mut drag = lifted(TestPanel::Alpha, (300.0, 200.0), Some(TestPanel::Docked));
    drag.retarget = 0.5;
    snapshot.tile_drag = Some(drag);
    let mut scratch = scratch_for(&snapshot);
    let frame = project_snapshot(&snapshot, &mut scratch);

    let docked = panel(&frame, TestPanel::Docked).region;
    let (from, to) = (slot_of(TestPanel::Docked), slot_of(TestPanel::Alpha));
    assert!(
        docked.x < from.x && docked.x > to.x && docked.y < from.y && docked.y > to.y,
        "half way: {docked:?} between {from:?} and {to:?}"
    );
}

#[test]
fn hits_while_lifted_resolve_against_resting_slots() {
    let mut snapshot = grid();
    snapshot.tile_drag = Some(lifted(
        TestPanel::Alpha,
        (300.0, 200.0),
        Some(TestPanel::Docked),
    ));
    let mut scratch = scratch_for(&snapshot);
    let frame = project_snapshot(&snapshot, &mut scratch);
    let surface = |key| {
        Some(HitTarget::Panel {
            key,
            part: PanelPart::Surface,
        })
    };

    assert_eq!(
        hit_test(&frame, center(slot_of(TestPanel::Docked))),
        surface(TestPanel::Docked),
        "the target's slot keeps naming the target after its preview moved away"
    );
    assert_eq!(
        hit_test(&frame, center(slot_of(TestPanel::Alpha))),
        surface(TestPanel::Alpha),
        "the dragged tile's slot is home even with the target drawn there"
    );
}

#[test]
fn points_in_gaps_and_past_the_grid_edges_name_the_nearest_slot() {
    let mut snapshot = grid();
    snapshot.tile_drag = Some(lifted(TestPanel::Alpha, (5.0, 5.0), None));
    let mut scratch = scratch_for(&snapshot);
    let frame = project_snapshot(&snapshot, &mut scratch);
    let surface = |key| {
        Some(HitTarget::Panel {
            key,
            part: PanelPart::Surface,
        })
    };
    let docked = slot_of(TestPanel::Docked);
    let beta = slot_of(TestPanel::Beta);

    assert_eq!(
        hit_test(
            &frame,
            (docked.x + docked.w + 500.0, docked.y + docked.h + 500.0)
        ),
        surface(TestPanel::Docked)
    );
    assert_eq!(
        hit_test(&frame, (beta.x + beta.w / 2.0, -400.0)),
        surface(TestPanel::Beta)
    );
    assert_eq!(
        hit_test(&frame, (-400.0, beta.y + 1.0)),
        surface(TestPanel::Alpha)
    );
}

#[test]
fn a_settling_tile_flies_from_its_release_point_into_the_committed_slot() {
    let mut snapshot = grid();
    // The drop committed Alpha <-> Docked: Docked, Beta, Gamma, Alpha.
    snapshot.panels.swap(0, 3);
    let mut drag = lifted(TestPanel::Alpha, (300.0, 200.0), Some(TestPanel::Docked));
    drag.phase = TileDragPhase::Settling;
    let home = slot_of(TestPanel::Alpha);
    let released = Region::new(home.x + 300.0, home.y + 200.0, home.w, home.h);

    for (land, expected) in [(0.0, released), (1.0, slot_of(TestPanel::Docked))] {
        drag.land = land;
        snapshot.tile_drag = Some(drag);
        let mut scratch = scratch_for(&snapshot);
        let frame = project_snapshot(&snapshot, &mut scratch);
        let alpha = panel(&frame, TestPanel::Alpha);
        assert_eq!(alpha.tile_role, TileRole::Landing);
        assert_eq!(alpha.region, expected, "land = {land}");
        assert_eq!(alpha.slot, slot_of(TestPanel::Docked));
        assert_eq!(panel(&frame, TestPanel::Docked).region, home);
    }
}
