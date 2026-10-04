# Base topos: the regime-neutral physics engine (lean-topos §42). Its sorts are
# engine particle/bond types; its stalks are the ONLY authored physics
# parameters. Each UI regime carries a geometric morphism p_R into these sorts,
# and a regime's tuning is the inverse image p_R* of these stalks, never
# authored. Values live in nix/lib/physicsData.nix (shared with the morphisms).

{ lib ? (import <nixpkgs> { }).lib }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  data = import ../lib/physicsData.nix;
in
topos.mkBaseTopos {
  spec_version = 1;
  id = "topos-physics";
  regime = "Physics engine";
  stalks = data.physics;
}
