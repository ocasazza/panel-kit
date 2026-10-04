# OKF catalog regime over the knowledge-catalog site: object sorts are the
# normalised OKF types (plus `concept` and `log`, which the corpus' link rules
# can produce but the pinned snapshot does not exercise) and morphism sorts are
# the link kinds. Identity sorting. The graph panel is an external `graph`
# component laid out concentric by sort; the rest are the corpus' own reads:
# Concepts table, tag badges, per-kind degree gauges, stable-share meter, and
# the host overlays (selected concept, repository/grammar, morphism, layout).
# `surface`/`glyphs` select units/chrome/glyphs; stalks, sheaves, and binding
# ids do not vary.

{ lib ? (import <nixpkgs> { }).lib, surface, glyphs ? "unicode" }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  L = import ../lib/mkToposLayout.nix { inherit lib; };
  inherit (topos) mkTopos defaultTheme;
  inherit (L) cell shelf textC tableC badgesC gaugesC meterC externalC flex textCell;

  # The corpus' 160 morphisms, counted at both ends: the full degree of any
  # one object sort, so every gauge reads as a share of the whole graph.
  degreeScale = 320.0;

  graphContent = externalC "graph" "okf.graph" {
    algorithm = "concentric";
    group_by = "sort";
    params = {
      # Concentric rings by degree, one bucket per OKF type.
      metric = 0.0;
      min_radius = 60.0;
      level_spacing = 90.0;
      bucket_count = 10.0;
      clockwise = 1.0;
    };
  };

  shelves = [
    (shelf 3 [ (cell "graph" "Graph" "graph" 4 graphContent) ])
    (shelf 3 [
      (cell "concepts" "Concepts" "concepts" 3 (tableC "okf.concepts"))
      (cell "tags" "Tags" "tags" 1 (badgesC "okf.tags"))
    ])
    (shelf 3 [
      (cell "kinds" "Kinds" "kinds" 2 (gaugesC "okf.kinds"))
      (cell "stable" "Stable share" "stable" 1 (meterC "okf.stable"))
      (cell "selected" "Selected" "selected" 1 (textC "okf.selected"))
    ])
    (shelf 3 [
      (cell "repository" "Repository" "repository" 1 (textC "okf.repository"))
      (cell "morphism" "Morphism" "morphism" 1 (textC "okf.morphism"))
      (cell "layout" "Layout" "layout" 1 (textC "okf.layout"))
    ])
  ];
in
mkTopos {
  spec_version = 1;
  id = "topos-okf-catalog";
  regime = "OKF catalog";

  object_sorting = {
    table = "table"; dataset = "dataset"; policy = "policy";
    metric = "metric"; computation = "computation"; skill = "skill";
    reference = "reference"; bundle = "bundle"; concept = "concept"; log = "log";
  };
  morphism_sorting = {
    contains = "contains"; references = "references"; derived_from = "derived_from";
    executed_by = "executed_by"; attested_by = "attested_by"; computes = "computes";
  };

  stalks = {
    objects = {
      bundle = { label = "Bundle"; badge = "folder"; };
      table = { label = "Table"; badge = "entity"; };
      dataset = { label = "Dataset"; badge = "entity"; };
      policy = { label = "Policy"; badge = "doctype"; };
      metric = { label = "Metric"; badge = "tag"; };
      computation = { label = "Computation"; badge = "entity"; };
      skill = { label = "Skill"; badge = "entity"; };
      reference = { label = "Reference"; badge = "generic"; };
      concept = { label = "Concept"; badge = "generic"; };
      log = { label = "Log"; badge = "date"; };
    };
    morphisms = {
      contains = { label = "contains"; };
      references = { label = "references"; };
      derived_from = { label = "derived from"; };
      executed_by = { label = "executed by"; };
      attested_by = { label = "attested by"; };
      computes = { label = "computes"; };
    };
    stages = {
      chain = "containment chain";
      branched = "containment fan";
      ring = "citation ring";
      sheet = "bundle sheet";
      tube = "provenance tube";
      membrane = "closed catalog";
    };
  };

  sheaves = {
    "okf.concepts" = {
      kind = "table";
      scope = "objects";
      columns = [
        { key = "concept"; title = "Concept"; width = flex 4; align = "left"; cell = textCell "title"; }
        { key = "kind"; title = "Kind"; width = flex 2; align = "left"; cell = textCell "kind"; }
        { key = "status"; title = "Status"; width = flex 2; align = "left"; cell = textCell "field?:status"; }
        { key = "trust"; title = "Trust"; width = flex 2; align = "left"; cell = textCell "field?:trust"; }
        { key = "tags"; title = "Tags"; width = flex 2; align = "left"; cell = textCell "tag:0"; }
        { key = "stale"; title = "Stale after"; width = flex 3; align = "left"; cell = textCell "field?:stale_after"; }
      ];
    };
    # One badge per concept's first tag. Bundles carry no tags, so the strip
    # restricts to the site's concept sorts.
    "okf.tags" = {
      kind = "badges";
      group = "objects";
      value = "tag:0";
      restrict = [ "table" "dataset" "policy" "metric" "computation" "skill" "reference" "concept" "log" ];
    };
    "okf.kinds" = { kind = "gauges"; scope = "kinds"; value = "degree"; scale = degreeScale; };
    "okf.stable" = {
      kind = "meter";
      label = "Stable concepts";
      value = "field?:status";
      agg = "count";
      filter = { field = "status"; equals = "stable"; };
      scale = 53.0;
    };
    # Host overlays: the selected concept's attributes and neighbours, the
    # repository and grammar tables, f against the active subobject, and the
    # layout transport.
    "okf.selected" = { kind = "text"; heading = "Selected concept"; };
    "okf.repository" = { kind = "text"; heading = "Repository"; };
    "okf.morphism" = { kind = "text"; heading = "Morphism f: topos-okf-catalog -> topos-okf-lineage"; };
    "okf.layout" = { kind = "text"; heading = "Layout transport"; };
  };

  workspace = L.mkWorkspace {
    inherit surface glyphs shelves;
    id = "topos-okf-catalog-ws";
    persistKey = "panel_kit_topos_okf_catalog_${surface}";
    theme = defaultTheme;
    components = [
      {
        id = "graph";
        contract = "graph";
        requires = [ "webgpu" ];
        keep_mounted = true;
        min_size = if surface == "web" then [ 320.0 240.0 ] else [ 24.0 8.0 ];
        events = [ "select" "hover" ];
        fallback = "cells";
      }
    ];
  };
}
