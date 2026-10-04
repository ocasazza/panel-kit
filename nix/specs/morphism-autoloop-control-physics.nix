# Geometric morphism p_control: topos-autoloop-control -> topos-physics. Sort
# maps live in nix/lib/physicsData.nix.

{ lib ? (import <nixpkgs> { }).lib }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  data = import ../lib/physicsData.nix;
in
topos.mkMorphism ({
  id = "autoloop-control-physics";
  domain = "topos-autoloop-control";
  codomain = "topos-physics";
} // data.autoloopControlToPhysics)
