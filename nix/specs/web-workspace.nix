{ lib ? (import <nixpkgs> { }).lib }:

let
  workspaceSpecSchema = builtins.fromJSON (builtins.readFile ../schema/workspace-spec.schema.json);
  workspace = import ../lib/mkWorkspaceSpec.nix { inherit lib workspaceSpecSchema; };
  inherit (workspace) mkWorkspaceSpec defaultTheme;

  window = index: {
    x = 24.0 + (index * 32.0);
    y = 24.0 + (index * 28.0);
    w = 360.0;
    h = 220.0;
    state = "Floating";
    z = index + 1;
    tile_w = 1;
    tile_h = 2;
  };

  panel = index: id: title: content: {
    inherit id title content;
    slug = id;
    window = window index;
  };
in
mkWorkspaceSpec {
  spec_version = 1;
  id = "web-workspace";

  layout = {
    units = "CssPx";
    viewport = [ 1280.0 800.0 ];
    preferred_mode = "Floating";
    clamp = {
      outer_w = 0.0;
      outer_h = 0.0;
      floor_w = 320.0;
      floor_h = 240.0;
      inner = 24.0;
      edge = 8.0;
      min_w = 180.0;
      min_h = 120.0;
      max_frac = 0.75;
    };
    tile = {
      resize = { row = 150.0; col_floor = 180.0; outer = 0.0; };
      row_min = 120.0;
      gap = 8.0;
      padding = 8.0;
      fill_viewport = true;
    };
  };

  surface = {
    compact_max = 760.0;
    tablet_max = 1180.0;
    fallback_capabilities = {
      coarse_pointer = false;
      hover = true;
      keyboard = true;
    };
    resize_policy = "scale_floating";
  };

  chrome = {
    metrics = { inset = 0.0; dock_h = 30.0; };
    hit_target_min = 24.0;
    panel_header_h = 22.0;
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
    steps = { coarse = 16.0; fine = 1.0; };
    bindings = [
      {
        chord = { key = "left"; shift = false; alt = false; ctrl = false; meta = false; };
        command = { kind = "move"; dx = -16.0; dy = 0.0; };
      }
      {
        chord = { key = { char = "f"; }; shift = false; alt = false; ctrl = false; meta = false; };
        command = { kind = "maximize"; };
      }
    ];
  };

  glyphs = "unicode";
  theme = defaultTheme;

  persistence = {
    enabled = true;
    key = "panel_kit_web_workspace";
    restore = true;
    save_policy = "on_settle";
  };

  panels = [
    (panel 0 "editor" "Editor" {
      kind = "editor";
      binding = "web.editor";
      multiline = true;
      placeholder = "Write canary notes…";
    })

    (panel 1 "meter" "Meter" {
      kind = "meter";
      source = {
        source = "inline";
        value = { label = "memory"; ratio = 0.57; text = "57%"; color = [ 94 243 140 ]; };
      };
    })

    (panel 2 "status" "Status" {
      kind = "status";
      source = {
        source = "inline";
        value = { label = "deploy"; state = "ok"; color = [ 39 201 63 ]; };
      };
    })

    (panel 3 "spinner" "Spinner" {
      kind = "spinner";
      label = { source = "inline"; value = "warming renderer"; };
    })
  ];
}
