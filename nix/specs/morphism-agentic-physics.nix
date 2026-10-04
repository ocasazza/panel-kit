# Geometric morphism p_agentic: topos-agentic -> topos-physics (into the base).
# The agentic regime's physics tuning is the inverse image p_agentic* of the
# engine stalks. Equals p_membrane . f on every site kind.
# Sort maps live in nix/lib/physicsData.nix.

{ lib ? (import <nixpkgs> { }).lib }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  data = import ../lib/physicsData.nix;
in
topos.mkMorphism ({
  id = "agentic-physics";
  domain = "topos-agentic";
  codomain = "topos-physics";
} // data.agenticToPhysics)
