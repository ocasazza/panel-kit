{ lib ? (import <nixpkgs> { }).lib }:

let
  workspaceSpecSchema = builtins.fromJSON (builtins.readFile ../schema/workspace-spec.schema.json);
  workspace = import ../lib/mkWorkspaceSpec.nix { inherit lib workspaceSpecSchema; };
  inherit (workspace) mkWorkspaceSpec defaultTheme;

  window = x: y: w: h: z: tile_w: tile_h: {
    inherit x y w h z tile_w tile_h;
    state = "Floating";
  };

  panel = id: title: slug: windowSpec: content: {
    inherit id title slug content;
    window = windowSpec;
  };

  bindingSource = id: { source = "binding"; inherit id; };
in
mkWorkspaceSpec {
  spec_version = 1;
  id = "workspace-canary";

  layout = {
    units = "Cells";
    viewport = [ 128.0 52.0 ];
    preferred_mode = "Floating";
    clamp = {
      outer_w = 0.0;
      outer_h = 0.0;
      floor_w = 24.0;
      floor_h = 8.0;
      inner = 2.0;
      edge = 0.0;
      min_w = 20.0;
      min_h = 5.0;
    };
    tile = {
      resize = { row = 4.0; col_floor = 12.0; outer = 0.0; };
      row_min = 4.0;
      gap = 0.0;
      padding = 0.0;
      fill_viewport = true;
    };
  };

  surface = {
    compact_max = 60.0;
    tablet_max = 110.0;
    fallback_capabilities = {
      coarse_pointer = false;
      hover = true;
      keyboard = true;
    };
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
    bindings = [
      {
        chord = { key = "left"; shift = false; alt = false; ctrl = false; meta = false; };
        command = { kind = "move"; dx = -16.0; dy = 0.0; };
      }
      {
        chord = { key = "right"; shift = true; alt = false; ctrl = false; meta = false; };
        command = { kind = "resize"; dw = 16.0; dh = 0.0; };
      }
      {
        chord = { key = { char = "m"; }; shift = false; alt = false; ctrl = false; meta = false; };
        command = { kind = "toggle_mode"; };
      }
    ];
  };

  glyphs = "unicode";
  theme = defaultTheme;

  persistence = {
    enabled = true;
    key = "panel_kit_canary";
    restore = true;
    save_policy = "on_settle";
  };

  panels = [
    (panel "Workspace" "Workspace" "workspace"
      (window 1.0 0.0 62.0 11.0 1 1 2)
      { kind = "custom"; binding = "canary.workspace"; })

    (panel "Activity" "Activity" "activity"
      (window 1.0 12.0 62.0 14.0 2 1 3)
      { kind = "time_series"; source = bindingSource "canary.activity"; unit = "ms"; })

    (panel "Flame" "Flame" "flame"
      (window 1.0 27.0 62.0 11.0 3 1 3)
      { kind = "flamegraph"; source = bindingSource "canary.flame"; })

    (panel "Notes" "Notes" "notes"
      (window 1.0 39.0 62.0 13.0 4 1 3)
      { kind = "text"; source = bindingSource "canary.notes"; scroll = "auto"; })

    (panel "Badges" "Badges" "badges"
      (window 65.0 0.0 63.0 11.0 5 2 3)
      { kind = "badges"; source = bindingSource "canary.badges"; })

    (panel "Nodes" "Nodes" "nodes"
      (window 65.0 12.0 63.0 9.0 6 2 2)
      { kind = "table"; source = bindingSource "canary.nodes"; })

    (panel "Capacity" "Capacity" "capacity"
      (window 65.0 22.0 63.0 8.0 7 2 2)
      { kind = "gauges"; source = bindingSource "canary.capacity"; })

    (panel "Distribution" "Distribution" "distribution"
      (window 65.0 31.0 63.0 11.0 8 2 3)
      { kind = "boxplot"; source = bindingSource "canary.distribution"; })

    (panel "Theme" "Theme" "theme"
      (window 65.0 43.0 63.0 9.0 9 2 2)
      { kind = "custom"; binding = "canary.theme"; })
  ];
}

