# Declarative panel-kit layout DSL.
#
# `mkLayout` takes a high-level attrset describing a panel workspace and
# produces a Nix value whose JSON serialization matches `panel-kit-core`'s
# `SavedLayout<K>` serde schema *exactly* (see
# `crates/panel-kit-core/src/lib.rs`):
#
#   SavedLayout<K> { panels: Vec<PanelWin<K>>, tiling: bool }
#   PanelWin<K>    { kind: K, x, y, w, h: f64, state: WinState,
#                    z: i32, tile_w: u8, tile_h: u8 }
#   WinState       = "Floating" | "Minimized" | "Maximized"
#
# `kind` is generic over the app's `PanelKind` enum; for the usual fieldless
# enum, serde serializes a variant as its bare name string (e.g. "Workspace"),
# which is what we emit. The Rust side validates on deserialize — this layer
# only guarantees the shape and the documented field bounds.
#
# Usage:
#   layout = mkLayout {
#     tiling = false;
#     panels = [
#       { kind = "Workspace"; x = 1.0; y = 0.0; w = 62.0; h = 11.0; }
#       { kind = "Inspector"; x = 65.0; y = 0.0; w = 63.0; h = 11.0;
#         state = "Floating"; tile_w = 2; tile_h = 3; }
#     ];
#   };
#   # layout.value   -> the SavedLayout attrset
#   # layout.json    -> the JSON string
#   # layout.kinds   -> the list of distinct panel kinds (for codegen)

{ lib }:

let
  # WinState variants, mirroring the Rust enum (no serde rename).
  winStates = [ "Floating" "Minimized" "Maximized" ];

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

  mkLayout = { tiling ? false, panels }:
    assert lib.assertMsg (builtins.isBool tiling)
      "mkLayout: `tiling` must be a bool (got ${builtins.typeOf tiling})";
    assert lib.assertMsg (builtins.isList panels)
      "mkLayout: `panels` must be a list (got ${builtins.typeOf panels})";
    let
      normalized = lib.imap1 normalizePanel panels;
      value = {
        panels = normalized;
        tiling = tiling;
      };
    in
    {
      # The SavedLayout attrset (serializes to the exact serde schema).
      inherit value;
      # Pretty-printable JSON string.
      json = builtins.toJSON value;
      # Distinct panel kinds in declaration order, for Rust enum codegen.
      kinds = lib.unique (builtins.map (p: p.kind) normalized);
    };
in
{
  inherit mkLayout winStates tileWMax tileHMax;
}
