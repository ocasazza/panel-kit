# Geometric morphism psi: topos-okf-catalog -> topos-membrane. Non-injective on
# objects (table, dataset, policy |-> head) and on morphisms (references,
# contains |-> solvation). Commutes over the base: p_membrane . psi = p_catalog.
# Sort maps live in nix/lib/physicsData.nix.

{ lib ? (import <nixpkgs> { }).lib }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  data = import ../lib/physicsData.nix;
in
topos.mkMorphism ({
  id = "okf-catalog-membrane";
  domain = "topos-okf-catalog";
  codomain = "topos-membrane";
} // data.okfCatalogToMembrane)