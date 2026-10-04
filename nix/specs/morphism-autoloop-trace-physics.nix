# Geometric morphism p_trace: topos-autoloop-trace -> topos-physics. Sort maps
# live in nix/lib/physicsData.nix.

{ lib ? (import <nixpkgs> { }).lib }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  data = import ../lib/physicsData.nix;
in
topos.mkMorphism ({
  id = "autoloop-trace-physics";
  domain = "topos-autoloop-trace";
  codomain = "topos-physics";
} // data.autoloopTraceToPhysics)
