# Declarative panel-kit layout DSL.
#
# `mkLayout` takes a high-level attrset describing a panel workspace and
# produces a Nix value whose JSON serialization matches `panel-kit-core`'s
# `SavedLayoutV2<K>` serde schema *exactly* (see
# `crates/panel-kit-core/src/lib.rs`):
#
#   SavedLayoutV2<K> { version: u32, units: Units, viewport: (f64, f64),
#                      mode: Mode, panels: Vec<PanelWin<K>> }
#   Units          = "CssPx" | "Cells"
#   Mode           = "Floating" | "Tiling"
#   PanelWin<K>    { kind: K, x, y, w, h: f64, state: WinState,
#                    z: i32, tile_w: u8, tile_h: u8 }
#   WinState       = "Floating" | "Minimized" | "Maximized"
#
# The legacy V1 shape (`{ panels, tiling }`) is read-only as of panel-kit
# 1.0.0: the Rust shells still *load* it through `StoredLayout` and upgrade
# it with `migrate_v1`, but nothing writes it any more, so this DSL emits V2
# directly. `units` and `viewport` are required because a floating rectangle
# is meaningless without them — `x = 65.0` is 65 CSS pixels to the web
# renderer and 65 character cells to the terminal, and V1 had no way to say
# which. Declaring them lets `reconcile_units` rescale a layout that is
# loaded by a renderer other than the one it was authored for.
#
# `kind` is generic over the app's `PanelKind` enum; for the usual fieldless
# enum, serde serializes a variant as its bare name string (e.g. "Workspace"),
# which is what we emit. The Rust side validates on deserialize — this layer
# only guarantees the shape and the documented field bounds.
#
# Usage:
#   layout = mkLayout {
#     units = "Cells";
#     viewport = [ 128.0 40.0 ];
#     mode = "Floating";
#     panels = [
#       { kind = "Workspace"; x = 1.0; y = 0.0; w = 62.0; h = 11.0; }
#       { kind = "Inspector"; x = 65.0; y = 0.0; w = 63.0; h = 11.0;
#         state = "Floating"; tile_w = 2; tile_h = 3; }
#     ];
#   };
#   # layout.value   -> the SavedLayoutV2 attrset
#   # layout.json    -> the JSON string
#   # layout.kinds   -> the list of distinct panel kinds (for codegen)

{ lib }:

let
  # WinState variants, mirroring the Rust enum (no serde rename).
  winStates = [ "Floating" "Minimized" "Maximized" ];

  # Mode variants, mirroring the Rust enum.
  modes = [ "Floating" "Tiling" ];

  # Units variants, mirroring the Rust enum.
  unitKinds = [ "CssPx" "Cells" ];

  # Schema version this DSL emits; must track `LAYOUT_SCHEMA_VERSION`.
  schemaVersion = 2;

  # Field bounds from panel-kit-core: tile_w in 1..=4, tile_h in 1..=6.
  tileWMax = 4;
  tileHMax = 6;

  clamp = lo: hi: n: lib.max lo (lib.min hi n);

  # Coerce numeric geometry to a float so `builtins.toJSON` emits `1.0`,
  # not `1`; serde's `f64` parses both, but matching the on-disk shape the
  # Rust shells write keeps round-trips byte-stable.
  toFloat = n: n + 0.0;

  required = [ "kind" "x" "y" "w" "h" ];

  normalizePanel = idx: panel:
    let
      present = builtins.attrNames panel;
      missing = lib.subtractLists present required;
      state = panel.state or "Floating";
      tileW = panel.tile_w or 1;
      tileH = panel.tile_h or 2;
    in
    assert lib.assertMsg (missing == [ ])
      ("mkLayout: panel ${toString idx} missing required field(s): "
        + lib.concatStringsSep ", " missing);
    assert lib.assertMsg (builtins.isString panel.kind)
      "mkLayout: panel ${toString idx} `kind` must be a string (got ${builtins.typeOf panel.kind})";
    assert lib.assertMsg (builtins.elem state winStates)
      "mkLayout: panel ${toString idx} `state` must be one of ${lib.concatStringsSep ", " winStates} (got ${toString state})";
    assert lib.assertMsg (builtins.isInt tileW && tileW >= 1 && tileW <= tileWMax)
      "mkLayout: panel ${toString idx} `tile_w` must be an int in 1..=${toString tileWMax} (got ${toString tileW})";
    assert lib.assertMsg (builtins.isInt tileH && tileH >= 1 && tileH <= tileHMax)
      "mkLayout: panel ${toString idx} `tile_h` must be an int in 1..=${toString tileHMax} (got ${toString tileH})";
    {
      kind = panel.kind;
      x = toFloat panel.x;
      y = toFloat panel.y;
      w = toFloat panel.w;
      h = toFloat panel.h;
      state = state;
      # Default z follows declaration order (1-based) so omitting `z`
      # yields a deterministic back-to-front stack matching list order.
      z = panel.z or idx;
      tile_w = clamp 1 tileWMax tileW;
      tile_h = clamp 1 tileHMax tileH;
    };

  mkLayout = { units, viewport, mode ? "Floating", panels }:
    assert lib.assertMsg (builtins.isString units && builtins.elem units unitKinds)
      "mkLayout: `units` must be one of ${lib.concatStringsSep ", " unitKinds} (got ${toString units})";
    assert lib.assertMsg (builtins.isString mode && builtins.elem mode modes)
      "mkLayout: `mode` must be one of ${lib.concatStringsSep ", " modes} (got ${toString mode})";
    assert lib.assertMsg
      (builtins.isList viewport && builtins.length viewport == 2
        && builtins.all builtins.isFloat (builtins.map toFloat viewport))
      "mkLayout: `viewport` must be a two-element list [ width height ]";
    assert lib.assertMsg (builtins.isList panels)
      "mkLayout: `panels` must be a list (got ${builtins.typeOf panels})";
    let
      normalized = lib.imap1 normalizePanel panels;
      value = {
        version = schemaVersion;
        units = units;
        # serde encodes a Rust tuple as a JSON array.
        viewport = builtins.map toFloat viewport;
        mode = mode;
        panels = normalized;
      };
    in
    {
      # The SavedLayoutV2 attrset (serializes to the exact serde schema).
      inherit value;
      # Pretty-printable JSON string.
      json = builtins.toJSON value;
      # Distinct panel kinds in declaration order, for Rust enum codegen.
      kinds = lib.unique (builtins.map (p: p.kind) normalized);
    };
in
{
  inherit mkLayout winStates modes unitKinds schemaVersion tileWMax tileHMax;
}
