# Geometric morphism chi: topos-okf-catalog -> topos-agentic. Non-injective on
# objects (table, dataset, policy |-> planner) and on morphisms (references,
# contains |-> consolidates). Commutes over the base: p_agentic . chi =
# p_catalog; with f: agentic -> membrane the triangle commutes, f . chi = psi.
# Sort maps live in nix/lib/physicsData.nix.

{ lib ? (import <nixpkgs> { }).lib }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  data = import ../lib/physicsData.nix;
in
topos.mkMorphism ({
  id = "okf-catalog-agentic";
  domain = "topos-okf-catalog";
  codomain = "topos-agentic";
} // data.okfCatalogToAgentic)