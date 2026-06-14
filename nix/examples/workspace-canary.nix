# Canary layout: a declarative mirror of the TUI canary
# `crates/panel-kit-tui/examples/workspace_canary.rs` (`defaults()`), proving
# the Nix DSL emits the same `SavedLayout` shape the Rust shells persist.
#
# Geometry, tile spans, and panel kinds match the Rust `Panel` enum
# (Workspace, Badges, Activity, Capacity, Nodes, Notes, Theme) one-for-one.
#
# Evaluate standalone with:
#   nix eval --json -f nix/examples/workspace-canary.nix value
# or build the JSON file via the flake's `packages.layout-canary`.

{ lib ? (import <nixpkgs> { }).lib }:

let
  inherit (import ../lib/mkLayout.nix { inherit lib; }) mkLayout;
in
mkLayout {
  tiling = false;
  panels = [
    { kind = "Workspace"; x = 1.0; y = 0.0; w = 62.0; h = 11.0; tile_w = 1; tile_h = 2; }
    { kind = "Activity"; x = 1.0; y = 12.0; w = 62.0; h = 14.0; tile_w = 1; tile_h = 3; }
    { kind = "Notes"; x = 1.0; y = 27.0; w = 62.0; h = 13.0; tile_w = 1; tile_h = 3; }
    { kind = "Badges"; x = 65.0; y = 0.0; w = 63.0; h = 11.0; tile_w = 2; tile_h = 3; }
    { kind = "Nodes"; x = 65.0; y = 12.0; w = 63.0; h = 9.0; tile_w = 2; tile_h = 2; }
    { kind = "Capacity"; x = 65.0; y = 22.0; w = 63.0; h = 8.0; tile_w = 2; tile_h = 2; }
    { kind = "Theme"; x = 65.0; y = 31.0; w = 63.0; h = 9.0; tile_w = 2; tile_h = 2; }
  ];
}
