# OKF lineage regime over the knowledge-catalog site: the same site read by
# the role each concept plays, not by its OKF type. source (table, dataset,
# policy), derived (metric, computation), operator (skill), context (bundle,
# reference, concept, log); morphisms derivation (derived_from, computes),
# execution (executed_by, attested_by), citation (references), containment
# (contains). The graph panel is an external `graph` component laid out dagre
# along derivation; the rest are the derivation table, role gauges, trust
# badges, and the host overlays (selected, morphism, layout). `surface`/
# `glyphs` select units/chrome/glyphs; stalks, sheaves, and binding ids do not
# vary.

{ lib ? (import <nixpkgs> { }).lib, surface, glyphs ? "unicode" }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  L = import ../lib/mkToposLayout.nix { inherit lib; };
  inherit (topos) mkTopos defaultTheme;
  inherit (L) cell shelf textC tableC badgesC gaugesC externalC flex textCell;

  # The corpus' 160 morphisms, counted at both ends: the full degree of any
  # one role, so every gauge reads as a share of the whole graph.
  degreeScale = 320.0;

  graphContent = externalC "graph" "okf.graph" {
    algorithm = "dagre";
    group_by = "sort";
    params = {
      # Left-to-right ranks along derivation, dense separation.
      rank_direction = 2.0;
      ranker = 1.0;
      rank_separation = 60.0;
      node_separation = 40.0;
      acyclic = 1.0;
    };
  };

  shelves = [
    (shelf 3 [ (cell "graph" "Graph" "graph" 4 graphContent) ])
    (shelf 3 [
      (cell "derivation" "Derivation" "derivation" 3 (tableC "okf.derivation"))
      (cell "trust" "Trust" "trust" 1 (badgesC "okf.trust"))
    ])
    (shelf 3 [
      (cell "roles" "Roles" "roles" 2 (gaugesC "okf.roles"))
      (cell "selected" "Selected" "selected" 1 (textC "okf.selected"))
      (cell "layout" "Layout" "layout" 1 (textC "okf.layout"))
    ])
    (shelf 3 [
      (cell "morphism" "Morphism" "morphism" 2 (textC "okf.morphism"))
      (cell "repository" "Repository" "repository" 2 (textC "okf.repository"))
    ])
  ];
in
mkTopos {
  spec_version = 1;
  id = "topos-okf-lineage";
  regime = "OKF lineage";

  object_sorting = {
    table = "source"; dataset = "source"; policy = "source";
    metric = "derived"; computation = "derived"; skill = "operator";
    bundle = "context"; reference = "context"; concept = "context"; log = "context";
  };
  morphism_sorting = {
    derived_from = "derivation"; computes = "derivation";
    executed_by = "execution"; attested_by = "execution";
    references = "citation"; contains = "containment";
  };

  stalks = {
    objects = {
      source = { label = "Source"; badge = "entity"; };
      derived = { label = "Derived"; badge = "tag"; };
      operator = { label = "Operator"; badge = "entity"; };
      context = { label = "Context"; badge = "folder"; };
    };
    morphisms = {
      derivation = { label = "derivation"; };
      execution = { label = "execution"; };
      citation = { label = "citation"; };
      containment = { label = "containment"; };
    };
    stages = {
      chain = "source chain";
      branched = "source fan";
      ring = "citation ring";
      sheet = "derivation sheet";
      tube = "execution tube";
      membrane = "closed lineage";
    };
  };

  sheaves = {
    # The provenance and execution morphisms, read by role rather than kind.
    "okf.derivation" = {
      kind = "table";
      scope = "morphisms";
      restrict = [ "derivation" "execution" ];
      columns = [
        { key = "concept"; title = "Concept"; width = flex 4; align = "left"; cell = textCell "source"; }
        { key = "target"; title = "Target"; width = flex 4; align = "left"; cell = textCell "target"; }
        { key = "kind"; title = "Relation"; width = flex 2; align = "left"; cell = textCell "edge_kind"; }
        {
          key = "weight";
          title = "Weight";
          width = flex 2;
          align = "left";
          cell = { cell = "meter"; value = "weight"; scale = 5.0; text = "weight"; };
        }
        { key = "rest"; title = "Rest"; width = flex 2; align = "right"; cell = textCell "rest_length"; }
      ];
    };
    # Trust is the OKF tier of each computed artifact; sources carry none.
    "okf.trust" = {
      kind = "badges";
      group = "objects";
      value = "field?:trust";
      restrict = [ "derived" ];
    };
    "okf.roles" = { kind = "gauges"; scope = "kinds"; value = "degree"; scale = degreeScale; };
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
    id = "topos-okf-lineage-ws";
    persistKey = "panel_kit_topos_okf_lineage_${surface}";
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
