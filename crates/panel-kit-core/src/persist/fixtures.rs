pub(super) fn v1_second_json() -> &'static str {
    r#"{"panels":[{"kind":"Second","x":40.0,"y":60.0,"w":80.0,"h":90.0,"state":"Maximized","z":9,"tile_w":3,"tile_h":4}],"tiling":true}"#
}

pub(super) fn v2_first_cells_json() -> &'static str {
    r#"{"version":2,"units":"Cells","viewport":[200.0,150.0],"mode":"Floating","panels":[{"kind":"First","x":5.0,"y":6.0,"w":7.0,"h":8.0,"state":"Floating","z":4,"tile_w":1,"tile_h":2}]}"#
}

pub(super) fn v2_unknown_json() -> &'static str {
    r#"{"version":2,"units":"CssPx","viewport":[400.0,300.0],"mode":"Floating","panels":[{"kind":"Unknown","x":1.0,"y":2.0,"w":3.0,"h":4.0,"state":"Floating","z":99,"tile_w":1,"tile_h":2}]}"#
}

pub(super) fn future_v3_json() -> &'static str {
    r#"{"version":3,"units":"CssPx","viewport":[400.0,300.0],"mode":"Floating","panels":[]}"#
}

pub(super) fn invalid_viewport_json() -> &'static str {
    r#"{"version":2,"units":"CssPx","viewport":[0.0,300.0],"mode":"Floating","panels":[]}"#
}
