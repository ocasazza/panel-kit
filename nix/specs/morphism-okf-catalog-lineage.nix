# Geometric morphism f: topos-okf-catalog -> topos-okf-lineage. Non-injective
# on objects and on morphisms, so the image (exists_f) and universal image
# (forall_f) differ: a lineage `source` cannot say which OKF type produced it.
# Commutes over the base: p_lineage . f = p_catalog. Sort maps live in
# nix/lib/physicsData.nix.

{ lib ? (import <nixpkgs> { }).lib }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  data = import ../lib/physicsData.nix;
in
topos.mkMorphism ({
  id = "okf-catalog-lineage";
  domain = "topos-okf-catalog";
  codomain = "topos-okf-lineage";
} // data.okfCatalogToLineage)
