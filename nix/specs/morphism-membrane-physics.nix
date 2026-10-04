# Geometric morphism p_membrane: topos-membrane -> topos-physics (into the
# base). The membrane regime's physics tuning is the inverse image p_membrane*
# of the engine stalks. Equals p_agentic via p_membrane . f = p_agentic.
# Sort maps live in nix/lib/physicsData.nix.

{ lib ? (import <nixpkgs> { }).lib }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  data = import ../lib/physicsData.nix;
in
topos.mkMorphism ({
  id = "membrane-physics";
  domain = "topos-membrane";
  codomain = "topos-physics";
} // data.membraneToPhysics)
