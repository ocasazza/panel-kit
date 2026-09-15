{ lib ? (import <nixpkgs> { }).lib }:

let
  workspaceSpecSchema = builtins.fromJSON (builtins.readFile ../schema/workspace-spec.schema.json);
  inherit (import ../lib/mkWorkspaceSpec.nix { inherit lib workspaceSpecSchema; }) mkWorkspaceSpec;
  good = (import ./workspace-spec.nix { inherit lib; }).value;
in
mkWorkspaceSpec (good // { chrome = good.chrome // { new_field = true; }; })
