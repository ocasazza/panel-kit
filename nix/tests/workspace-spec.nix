{ lib ? (import <nixpkgs> { }).lib }:

let
  workspaceSpecSchema = builtins.fromJSON (builtins.readFile ../schema/workspace-spec.schema.json);
  workspace = import ../lib/mkWorkspaceSpec.nix { inherit lib workspaceSpecSchema; };
  inherit (workspace) mkWorkspaceSpec defaultTheme;

  window = index: {
    x = 1.0 + (index * 2.0); y = 1.0 + index; w = 24.0; h = 8.0;
    state = "Floating"; z = index + 1; tile_w = 1; tile_h = 2;
  };
  panel = index: id: title: content: {
    inherit id title content;
    slug = id;
    window = window index;
  };
in
mkWorkspaceSpec {
  spec_version = 1;
  id = "workspace-spec-reference";
  layout = {
    units = "Cells";
    viewport = [ 128.0 52.0 ];
    preferred_mode = "Floating";
    clamp = { outer_w = 0.0; outer_h = 0.0; floor_w = 24.0; floor_h = 8.0; inner = 2.0; edge = 0.0; min_w = 20.0; min_h = 5.0; };
    tile = { resize = { row = 4.0; col_floor = 12.0; outer = 0.0; }; row_min = 4.0; gap = 0.0; padding = 0.0; fill_viewport = true; };
  };
  surface = {
    compact_max = 60.0;
    tablet_max = 110.0;
    fallback_capabilities = { coarse_pointer = false; hover = true; keyboard = true; };
    resize_policy = "preserve_intent";
  };
  chrome = {
    metrics = { inset = 1.0; dock_h = 3.0; };
    hit_target_min = 1.0;
    panel_header_h = 1.0;
    panel_frame = true;
    title_in_border = true;
    mode_control = true;
    minimize_control = true;
    maximize_control = true;
    resize_grip = true;
    dock = true;
    dock_label = "dock:";
  };
  input = {
    steps = { coarse = 2.0; fine = 1.0; };
    bindings = [ { chord = { key = "left"; shift = false; alt = false; ctrl = false; meta = false; }; command = { kind = "move"; dx = -16.0; dy = 0.0; }; } ];
  };
  glyphs = "unicode";
  theme = defaultTheme;
  persistence = { enabled = true; key = "panel_kit_reference"; restore = true; save_policy = "on_settle"; };
  panels = [
    (panel 0 "custom" "Custom" { kind = "custom"; binding = "canary.custom"; })
    (panel 1 "text" "Text" { kind = "text"; source = { source = "inline"; value = { text = "hello"; }; }; scroll = "wrap"; })
    (panel 2 "editor" "Editor" { kind = "editor"; binding = "canary.editor"; multiline = true; placeholder = "Write"; })
    (panel 3 "badges" "Badges" { kind = "badges"; source = { source = "inline"; value = [ { kind = "tag"; field = "tag"; value = "nix"; active = false; with_x = false; with_plus = true; small = false; override_color = [ 80 180 130 ]; accent_color = null; click_kind = "toggle"; emit_hover = false; } ]; }; })
    (panel 4 "table" "Table" { kind = "table"; source = { source = "inline"; value = { columns = [ { key = "name"; title = "Name"; width = { Fixed = { value = 12; }; }; align = "left"; } ]; rows = [ { cells = [ { kind = "text"; value = "alpha"; } ]; } ]; }; }; })
    (panel 5 "time-series" "Time Series" { kind = "time_series"; source = { source = "inline"; value = [ { name = "load"; points = [ [ 0.0 1.0 ] [ 1.0 2.0 ] ]; } ]; }; unit = "req/s"; })
    (panel 6 "gauges" "Gauges" { kind = "gauges"; source = { source = "inline"; value = [ { label = "cpu"; ratio = 0.42; text = "42%"; } ]; }; })
    (panel 7 "flamegraph" "Flamegraph" { kind = "flamegraph"; source = { source = "inline"; value = [ { label = "root"; depth = 0; value = 1.0; color = null; } ]; }; })
    (panel 8 "boxplot" "Boxplot" { kind = "boxplot"; source = { source = "inline"; value = [ { label = "latency"; samples = [ 1.0 2.0 3.0 ]; color = null; } ]; }; })
    (panel 9 "meter" "Meter" { kind = "meter"; source = { source = "inline"; value = { label = "memory"; ratio = 0.5; text = "50%"; color = null; }; }; })
    (panel 10 "status" "Status" { kind = "status"; source = { source = "inline"; value = { label = "build"; state = "ok"; color = [ 39 201 63 ]; }; }; })
    (panel 11 "spinner" "Spinner" { kind = "spinner"; label = { source = "inline"; value = "Loading"; }; })
  ];
}
