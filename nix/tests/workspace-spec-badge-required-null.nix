{ lib ? (import <nixpkgs> { }).lib }:

let
  workspaceSpecSchema = builtins.fromJSON (builtins.readFile ../schema/workspace-spec.schema.json);
  inherit (import ../lib/mkWorkspaceSpec.nix { inherit lib workspaceSpecSchema; }) mkWorkspaceSpec;
  good = (import ./workspace-spec.nix { inherit lib; }).value;
  badgePanel = builtins.elemAt good.panels 3;
  badge = builtins.head badgePanel.content.source.value;
  malformedBadge = builtins.removeAttrs badge [ "accent_color" ];
  malformed = badgePanel // {
    content = badgePanel.content // {
      source = badgePanel.content.source // { value = [ malformedBadge ]; };
    };
  };
in
mkWorkspaceSpec (good // { panels = [ malformed ]; })
