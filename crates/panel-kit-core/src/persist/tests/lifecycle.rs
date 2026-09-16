use serde_json::{json, Value};

use super::common::{catalog, defaults, restore_context, MemoryStore, TestPanel};
use super::fixtures::{
    future_v3_json, invalid_viewport_json, v1_second_json, v2_first_cells_json, v2_unknown_json,
};
use crate::persist::{persist_snapshot, restore_snapshot, LayoutError, LayoutStore};
use crate::{Mode, Units, WinState, LAYOUT_SCHEMA_VERSION};

#[test]
fn core_store_restores_v1_reconciles_merges_saves_v2() {
    let store = MemoryStore::seeded(v1_second_json());
    let restored = restore_snapshot(&store, defaults(), &catalog(), restore_context()).unwrap();

    assert_restored_v1(&restored);
    assert_reconciles_v2_cells_to_css_px();

    persist_snapshot(&store, &restored, &catalog()).unwrap();
    assert_saved_layout_json(
        &store.saved_json().expect("persist writes a V2 record"),
        expected_restored_v1_saved_layout(),
    );
}

#[test]
fn store_clear_is_observable() {
    let store = MemoryStore::seeded(v2_unknown_json());
    let restored = restore_snapshot(&store, defaults(), &catalog(), restore_context()).unwrap();
    let restored_order = restored
        .panels
        .iter()
        .map(|panel| panel.kind)
        .collect::<Vec<_>>();

    assert_eq!(
        restored_order,
        vec![TestPanel::First, TestPanel::Second, TestPanel::Third]
    );

    store.clear().unwrap();
    assert_eq!(store.load().unwrap(), None);
}

#[test]
fn invalid_json_future_version_and_invalid_saved_viewport_error() {
    let cases = [
        ("invalid json", "{", ErrorKind::Decode),
        ("future version", future_v3_json(), ErrorKind::FutureVersion),
        (
            "invalid saved viewport",
            invalid_viewport_json(),
            ErrorKind::InvalidViewport,
        ),
    ];

    for (name, json, kind) in cases {
        let store = MemoryStore::seeded(json);
        let result = restore_snapshot(&store, defaults(), &catalog(), restore_context());
        let Err(error) = result else {
            panic!("{name} should fail")
        };

        assert_error_kind(error, kind);
        assert_eq!(store.saved_json().as_deref(), Some(json), "{name}");
    }
}

#[test]
fn serde_spellings_match_existing_saved_layouts() {
    let store = MemoryStore::empty();

    persist_snapshot(&store, &defaults(), &catalog()).unwrap();
    assert_saved_layout_json(
        &store.saved_json().unwrap(),
        expected_default_saved_layout(),
    );
}

fn assert_restored_v1(restored: &crate::reducer::Snapshot<TestPanel>) {
    assert_eq!(restored.preferred_mode, Mode::Tiling);
    assert_eq!(restored.viewport.units, Units::CssPx);
    assert_eq!(restored.viewport.width, 400.0);
    assert_eq!(restored.panels.len(), 3);
    assert_eq!(restored.panels[0].kind, TestPanel::Second);
    assert_eq!(restored.panels[0].state, WinState::Maximized);
    assert_eq!(restored.panels[1].kind, TestPanel::First);
    assert_eq!(restored.panels[2].kind, TestPanel::Third);
}

fn assert_reconciles_v2_cells_to_css_px() {
    let store = MemoryStore::seeded(v2_first_cells_json());
    let restored = restore_snapshot(&store, defaults(), &catalog(), restore_context()).unwrap();
    let panel = &restored.panels[0];

    assert_eq!(panel.kind, TestPanel::First);
    assert_eq!(
        (panel.x, panel.y, panel.w, panel.h),
        (10.0, 12.0, 14.0, 16.0)
    );
}

fn assert_saved_layout_json(saved: &str, expected: Value) {
    let actual = serde_json::from_str::<Value>(saved).expect("saved layout is valid JSON");

    assert_eq!(actual, expected);
}

fn expected_default_saved_layout() -> Value {
    json!({
        "version": LAYOUT_SCHEMA_VERSION,
        "units": "CssPx",
        "viewport": [400.0, 300.0],
        "mode": "Floating",
        "panels": [
            panel_json("First", [10.0, 20.0, 100.0, 80.0], "Floating", 1, [1, 2]),
            panel_json("Second", [160.0, 20.0, 120.0, 90.0], "Floating", 2, [1, 2]),
            panel_json("Third", [320.0, 20.0, 140.0, 100.0], "Floating", 3, [1, 2]),
        ],
    })
}

fn expected_restored_v1_saved_layout() -> Value {
    json!({
        "version": LAYOUT_SCHEMA_VERSION,
        "units": "CssPx",
        "viewport": [400.0, 300.0],
        "mode": "Tiling",
        "panels": [
            panel_json("Second", [40.0, 60.0, 80.0, 90.0], "Maximized", 9, [3, 4]),
            panel_json("First", [10.0, 20.0, 100.0, 80.0], "Floating", 1, [1, 2]),
            panel_json("Third", [320.0, 20.0, 140.0, 100.0], "Floating", 3, [1, 2]),
        ],
    })
}

fn panel_json(
    kind: &'static str,
    [x, y, w, h]: [f64; 4],
    state: &'static str,
    z: i32,
    [tile_w, tile_h]: [u8; 2],
) -> Value {
    json!({
        "kind": kind,
        "x": x,
        "y": y,
        "w": w,
        "h": h,
        "state": state,
        "z": z,
        "tile_w": tile_w,
        "tile_h": tile_h,
    })
}

#[derive(Clone, Copy)]
enum ErrorKind {
    Decode,
    FutureVersion,
    InvalidViewport,
}

fn assert_error_kind(error: LayoutError, kind: ErrorKind) {
    match (error, kind) {
        (LayoutError::Decode(_), ErrorKind::Decode) => {}
        (LayoutError::UnsupportedVersion(3), ErrorKind::FutureVersion) => {}
        (LayoutError::InvalidViewport { .. }, ErrorKind::InvalidViewport) => {}
        (error, _) => panic!("unexpected layout error: {error:?}"),
    }
}
