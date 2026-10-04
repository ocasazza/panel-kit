# Lipid self-assembly regime over the same session site (replaces the
# force-field regime): sessions are lipids under Brownian motion whose bonds
# self-assemble into a bilayer. Species: planner/reviewer -> head, coder ->
# tail, memory -> solvent; continues -> tail_bond, shares -> lateral,
# consolidates -> solvation. Regime stalks carry only UI/UX data; physics tuning
# comes from the base (p_membrane*). Sheaves: Stalks text, Species badges,
# Assembly table + stage badges, energy Boxplot, per-species potential Gauges,
# total potential Meter, Physics table, Morphism + Invariants host overlays.

{ lib ? (import <nixpkgs> { }).lib, surface, glyphs ? "unicode" }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  L = import ../lib/mkToposLayout.nix { inherit lib; };
  inherit (topos) mkTopos defaultTheme;
  inherit (L) cell shelf textC tableC badgesC boxC gaugesC meterC;

  shelves = [
    (shelf 3 [
      (cell "stalks" "Stalks" "stalks" 1 (textC "membrane.stalks"))
      (cell "species" "Species" "species" 1 (badgesC "membrane.species"))
      (cell "spread" "Energy spread" "spread" 2 (boxC "membrane.spread"))
    ])
    (shelf 3 [
      (cell "assembly" "Assembly" "assembly" 3 (tableC "membrane.assembly"))
      (cell "stages" "Stages" "stages" 1 (badgesC "membrane.stages"))
    ])
    (shelf 3 [
      (cell "potential" "Species potential" "potential" 2 (gaugesC "membrane.gauges"))
      (cell "total" "Total potential" "total" 2 (meterC "membrane.meter"))
    ])
    (shelf 3 [
      (cell "physics" "Physics" "physics" 2 (tableC "membrane.physics"))
      (cell "morphism" "Morphism" "morphism" 1 (textC "membrane.morphism"))
      (cell "invariants" "Invariants" "invariants" 1 (tableC "membrane.invariants"))
    ])
  ];
in
mkTopos {
  spec_version = 1;
  id = "topos-membrane";
  regime = "Lipid self-assembly";

  object_sorting = { planner = "head"; reviewer = "head"; coder = "tail"; memory = "solvent"; };
  morphism_sorting = { continues = "tail_bond"; shares = "lateral"; consolidates = "solvation"; };

  stalks = {
    objects = {
      head = { label = "Head"; badge = "status"; };
      tail = { label = "Tail"; badge = "entity"; };
      solvent = { label = "Solvent"; badge = "folder"; };
    };
    morphisms = {
      tail_bond = { label = "tail bond"; };
      lateral = { label = "lateral"; };
      solvation = { label = "solvation"; };
    };
    stages = {
      chain = "chain";
      branched = "branched chain";
      ring = "ring";
      sheet = "bilayer sheet";
      tube = "flat tube";
      membrane = "membrane";
    };
  };

  sheaves = {
    "membrane.stalks" = {
      kind = "text";
      heading = "Lipid self-assembly regime";
      preface = [
        "Sessions are lipids under Brownian motion; bonds self-assemble into a bilayer."
        "Species: planner/reviewer -> head, coder -> tail, memory -> solvent."
        "Assembly over tail_bond+lateral: chain, ring, bilayer sheet, flat tube, closed membrane."
        "Stalks: object sort -> label [badge]; morphism sort -> label."
      ];
      include_stalks = true;
    };
    "membrane.species" = { kind = "badges"; group = "kinds"; };
    "membrane.spread" = { kind = "boxplot"; sample = "field:energy"; };
    "membrane.assembly" = { kind = "assembly"; morphism_sorts = [ "tail_bond" "lateral" ]; };
    "membrane.stages" = { kind = "assembly_badges"; morphism_sorts = [ "tail_bond" "lateral" ]; };
    "membrane.gauges" = { kind = "gauges"; scope = "kinds"; value = "weighted:energy"; scale = 45000.0; };
    "membrane.meter" = {
      kind = "meter";
      label = "Total potential";
      value = "weighted:energy";
      agg = "sum";
      scale = 60000.0;
      unit = "pJ";
    };
    "membrane.physics" = { kind = "physics"; scope = "objects"; };
    "membrane.morphism" = { kind = "morphism"; };
    "membrane.invariants" = { kind = "invariants"; };
  };

  workspace = L.mkWorkspace {
    inherit surface glyphs shelves;
    id = "topos-membrane-ws";
    persistKey = "panel_kit_topos_membrane_${surface}";
    theme = defaultTheme;
  };
}
