# Topos-spec producer. Wraps `mkWorkspaceSpec` for the embedded `workspace`
# member and assembles the topos envelope (stalks + sheaves) around it.
# The crate's `Topos::from_json_str`/`validate` enforce the binding ↔
# sheaf ↔ content-kind contract; this layer validates the workspace.
#
#   topos = (import ./mkToposSpec.nix { inherit lib }).mkTopos {
#     id; regime; object_sorting; morphism_sorting;
#     stalks      = { objects; morphisms; stages; };
#     sheaves     = { "<binding>" = { kind = "..."; ... }; };
#     workspace   = { spec_version; id; layout; ... panels; };  # mkWorkspaceSpec input
#   };
#   # topos.value -> the Topos attrset
#   # topos.json  -> the JSON string (pkgs.writeText'd by the flake)

{ lib ? (import <nixpkgs> { }).lib }:

let
  workspaceSpecSchema = builtins.fromJSON (builtins.readFile ../schema/workspace-spec.schema.json);
  wsLib = import ./mkWorkspaceSpec.nix { inherit lib workspaceSpecSchema; };
in
{
  inherit (wsLib) defaultTheme mkWorkspaceSpec;

  # Regime topos: object/morphism sorting functors + stalks keyed by regime
  # sort + sheaves over the embedded workspace.
  mkTopos = { spec_version ? 1, id, regime, object_sorting, morphism_sorting, stalks, sheaves, workspace }:
    let
      ws = wsLib.mkWorkspaceSpec workspace;
      value = {
        inherit spec_version id regime object_sorting morphism_sorting stalks sheaves;
        workspace = ws.value;
      };
    in
    {
      inherit value;
      json = builtins.toJSON value;
      workspace_json = ws.json;
      bindings = ws.bindings;
      panel_ids = ws.panel_ids;
    };

  # Base topos (topos-physics): stalks only, no sorting/sheaves/workspace.
  mkBaseTopos = { spec_version ? 1, id, regime, stalks }:
    let value = { inherit spec_version id regime stalks; };
    in { inherit value; json = builtins.toJSON value; };

  # Geometric morphism document (regime->regime or regime->base share the type).
  mkMorphism = { id, domain, codomain, object_sorts, morphism_sorts }:
    let value = { inherit id domain codomain object_sorts morphism_sorts; };
    in { inherit value; json = builtins.toJSON value; };
}
