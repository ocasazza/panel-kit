# Geometric morphism p_catalog: topos-okf-catalog -> topos-physics. Sort maps
# live in nix/lib/physicsData.nix.

{ lib ? (import <nixpkgs> { }).lib }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  data = import ../lib/physicsData.nix;
in
topos.mkMorphism ({
  id = "okf-catalog-physics";
  domain = "topos-okf-catalog";
  codomain = "topos-physics";
} // data.okfCatalogToPhysics)
