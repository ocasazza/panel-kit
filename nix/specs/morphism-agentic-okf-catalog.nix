# Geometric morphism omega: topos-agentic -> topos-okf-catalog. Non-injective
# on objects (planner, reviewer |-> policy) and on morphisms (shares, produces
# |-> executed_by). Commutes over the base: p_catalog . omega = p_agentic, and
# with psi: okf-catalog -> membrane the diagram closes, psi . omega = f. Sort and
# attribute maps live in nix/lib/physicsData.nix.

{ lib ? (import <nixpkgs> { }).lib }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  data = import ../lib/physicsData.nix;
in
topos.mkMorphism ({
  id = "agentic-okf-catalog";
  domain = "topos-agentic";
  codomain = "topos-okf-catalog";
} // data.agenticToOkfCatalog)