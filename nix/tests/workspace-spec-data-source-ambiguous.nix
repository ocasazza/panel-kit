{ lib ? (import <nixpkgs> { }).lib }:

let
  workspaceSpecSchema = builtins.fromJSON (builtins.readFile ../schema/workspace-spec.schema.json);
  inherit (import ../lib/mkWorkspaceSpec.nix { inherit lib workspaceSpecSchema; }) mkWorkspaceSpec;
  good = (import ./workspace-spec.nix { inherit lib; }).value;
  first = builtins.elemAt good.panels 1;
  malformed = first // {
    content = first.content // {
      source = first.content.source // { id = "ambiguous-inline-source"; };
    };
  };
in
mkWorkspaceSpec (good // { panels = [ malformed ]; })
