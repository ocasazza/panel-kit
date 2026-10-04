# Shared layout for the regime topoi: per-surface units/chrome/input (identical
# across topoi) plus a 4-column shelf grid that mirrors the tiling. Callers
# supply `shelves` (packing order; tile_w per cell, uniform tile_h per shelf)
# and the stalks/sheaves; geometry and units differ per surface, panels do not.

{ lib }:

rec {
  bindingSource = id: { source = "binding"; inherit id; };
  flex = weight: { flex = { inherit weight; }; };
  fixed = value: { fixed = { inherit value; }; };
  textCell = expr: { cell = "text"; inherit expr; };

  # Panel content builders, keyed by sheaf binding id.
  textC = id: { kind = "text"; source = bindingSource id; scroll = "auto"; };
  tableC = id: { kind = "table"; source = bindingSource id; };
  badgesC = id: { kind = "badges"; source = bindingSource id; };
  flameC = id: { kind = "flamegraph"; source = bindingSource id; };
  boxC = id: { kind = "boxplot"; source = bindingSource id; };
  gaugesC = id: { kind = "gauges"; source = bindingSource id; };
  meterC = id: { kind = "meter"; source = bindingSource id; };

  cell = id: title: slug: tile_w: content: { inherit id title slug tile_w content; };
  shelf = tile_h: cells: { inherit tile_h cells; };

  perSurface = surface:
    if surface == "web" then {
      layout = {
        units = "CssPx";
        viewport = [ 1280.0 800.0 ];
        clamp = {
          outer_w = 0.0; outer_h = 0.0; floor_w = 320.0; floor_h = 240.0;
          inner = 24.0; edge = 8.0; min_w = 180.0; min_h = 120.0; max_frac = 0.75;
        };
        tile = {
          resize = { row = 150.0; col_floor = 180.0; outer = 0.0; };
          row_min = 120.0; gap = 8.0; padding = 8.0; fill_viewport = true;
        };
      };
      surface = {
        compact_max = 760.0; tablet_max = 1180.0;
        fallback_capabilities = { coarse_pointer = false; hover = true; keyboard = true; };
        resize_policy = "scale_floating";
      };
      chrome = {
        metrics = { inset = 0.0; dock_h = 30.0; };
        hit_target_min = 24.0; panel_header_h = 22.0; panel_frame = true;
        title_in_border = true; mode_control = true; minimize_control = true;
        maximize_control = true; resize_grip = true; dock = true; dock_label = "dock:";
      };
      input_steps = { coarse = 16.0; fine = 1.0; };
      move_step = 16.0;
    } else {
      layout = {
        units = "Cells";
        viewport = [ 128.0 52.0 ];
        clamp = {
          outer_w = 0.0; outer_h = 0.0; floor_w = 24.0; floor_h = 8.0;
          inner = 2.0; edge = 0.0; min_w = 20.0; min_h = 5.0; max_frac = 0.75;
        };
        tile = {
          resize = { row = 4.0; col_floor = 12.0; outer = 0.0; };
          row_min = 4.0; gap = 0.0; padding = 0.0; fill_viewport = true;
        };
      };
      surface = {
        compact_max = 60.0; tablet_max = 110.0;
        fallback_capabilities = { coarse_pointer = false; hover = true; keyboard = true; };
        resize_policy = "preserve_intent";
      };
      chrome = {
        metrics = { inset = 1.0; dock_h = 3.0; };
        hit_target_min = 1.0; panel_header_h = 1.0; panel_frame = true;
        title_in_border = true; mode_control = true; minimize_control = true;
        maximize_control = true; resize_grip = true; dock = true; dock_label = "dock:";
      };
      input_steps = { coarse = 2.0; fine = 1.0; };
      move_step = 4.0;
    };

  # 4-column floating grid. Each shelf is one row of equal height; cells pack
  # left to right by tile_w. Widths stay under the 0.75 max_frac cap.
  grid = surface:
    if surface == "web" then
      { x0 = 8.0; y0 = 8.0; gap = 8.0; colW = 310.0; rowGap = 8.0; vh = 800.0; margin = 8.0; }
    else
      { x0 = 4.0; y0 = 0.0; gap = 0.0; colW = 30.0; rowGap = 0.0; vh = 52.0; margin = 0.0; };

  mkPanels = surface: shelves:
    let
      m = grid surface;
      nRows = builtins.length shelves;
      rowH = (m.vh - 2.0 * m.margin - (nRows - 1 + 0.0) * m.rowGap) / (nRows + 0.0);
      placeShelf = ri: shlf:
        let
          y = m.y0 + (ri + 0.0) * (rowH + m.rowGap);
          folded = lib.foldl
            (st: c:
              let
                x = m.x0 + (st.col + 0.0) * (m.colW + m.gap);
                w = (c.tile_w + 0.0) * m.colW + (c.tile_w - 1 + 0.0) * m.gap;
              in
              { col = st.col + c.tile_w; acc = st.acc ++ [ (c // { inherit x w y; th = shlf.tile_h; }) ]; })
            { col = 0; acc = [ ]; }
            shlf.cells;
        in
        folded.acc;
      flat = lib.concatLists (lib.imap0 placeShelf shelves);
    in
    lib.imap0
      (i: c: {
        id = c.id; title = c.title; slug = c.slug;
        window = {
          x = c.x; y = c.y; w = c.w; h = rowH;
          state = "Floating"; z = i + 1; tile_w = c.tile_w; tile_h = c.th;
        };
        content = c.content;
      })
      flat;

  mkWorkspace = { surface, glyphs, id, persistKey, theme, shelves }:
    let ps = perSurface surface; in {
      spec_version = 1;
      inherit id;
      layout = ps.layout // { preferred_mode = "Tiling"; };
      surface = ps.surface;
      chrome = ps.chrome;
      input = {
        steps = ps.input_steps;
        bindings = [
          { chord = { key = "left"; shift = false; alt = false; ctrl = false; meta = false; };
            command = { kind = "move"; dx = 0.0 - ps.move_step; dy = 0.0; }; }
          { chord = { key = "right"; shift = true; alt = false; ctrl = false; meta = false; };
            command = { kind = "resize"; dw = ps.move_step; dh = 0.0; }; }
          { chord = { key = { char = "m"; }; shift = false; alt = false; ctrl = false; meta = false; };
            command = { kind = "toggle_mode"; }; }
        ];
      };
      inherit glyphs theme;
      persistence = { enabled = true; key = persistKey; restore = true; save_policy = "on_settle"; };
      panels = mkPanels surface shelves;
    };
}
