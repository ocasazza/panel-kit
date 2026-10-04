# Geometric morphism f: topos-agentic -> topos-membrane. Non-injective on
# objects (planner, reviewer |-> head), so the image (exists_f) and universal
# image (forall_f) differ. Commutes over the base: p_membrane . f = p_agentic.
# Sort maps live in nix/lib/physicsData.nix.

{ lib ? (import <nixpkgs> { }).lib }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  data = import ../lib/physicsData.nix;
in
topos.mkMorphism ({
  id = "agentic-membrane";
  domain = "topos-agentic";
  codomain = "topos-membrane";
} // data.agenticToMembrane)
