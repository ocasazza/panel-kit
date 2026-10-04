# Geometric morphism p_lineage: topos-okf-lineage -> topos-physics. Composed
# with f: topos-okf-catalog -> topos-okf-lineage it is p_catalog on every sort,
# so both OKF regimes induce one physics section. Sort maps live in
# nix/lib/physicsData.nix.

{ lib ? (import <nixpkgs> { }).lib }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  data = import ../lib/physicsData.nix;
in
topos.mkMorphism ({
  id = "okf-lineage-physics";
  domain = "topos-okf-lineage";
  codomain = "topos-physics";
} // data.okfLineageToPhysics)
