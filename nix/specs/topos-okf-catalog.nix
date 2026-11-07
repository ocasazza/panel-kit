# OKF catalog regime over the merged trio site: object sorts are the
# normalised OKF types (plus `concept` and `log`) together with the agent
# session kinds pushed along ω (a planning or reviewing session is a `policy`,
# a coding session a `computation`, a memory store a `concept`), and morphism
# sorts are the link kinds plus the session bonds read along ω. Identity
# sorting on the OKF kinds. The graph panel is an external `graph` component
# running the one pulled-back force physics; the rest are the corpus' own
# reads: Concepts table, tag badges, per-kind degree gauges, assembly over
# the link kinds with stage badges, the physics table, the Grammar and
# Morphism editors, and the host's Selected overlay.
# `surface`/`glyphs` select units/chrome/glyphs; stalks, sheaves, and binding
# ids do not vary.

{ lib ? (import <nixpkgs> { }).lib, surface, glyphs ? "unicode" }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  L = import ../lib/mkToposLayout.nix { inherit lib; };
  inherit (topos) mkTopos defaultTheme;
  inherit (L) cell shelf textC tableC badgesC gaugesC editorC externalC flex textCell;

  # The corpus' 160 morphisms, counted at both ends: the full degree of any
  # one object sort, so every gauge reads as a share of the whole graph.
  degreeScale = 320.0;

  # Force, like every regime: the physics is the one base pullback, so the
  # arrangement survives a morphism and only the sorts and colours move.
  graphContent = externalC "graph" "okf.graph" {
    algorithm = "amphiphile";
    group_by = null;
    params = { };
  };

  # The trio grid (see topos-agentic.nix): same shelves and widths in every
  # regime, so `t` swaps bodies without moving panels.
  shelves = [
    (shelf 4 [ (cell "graph" "Graph" "graph" 4 graphContent) ])
    (shelf 3 [
      (cell "grammar" "Grammar" "grammar" 2 (editorC "trio.grammar"))
      (cell "morphism" "Morphism" "morphism" 2 (editorC "okf.morphism"))
    ])
    (shelf 3 [
      (cell "selected" "Selected" "selected" 1 (textC "okf.selected"))
      (cell "concepts" "Concepts" "concepts" 3 (tableC "okf.concepts"))
    ])
    (shelf 3 [
      (cell "kinds" "Kinds" "kinds" 3 (gaugesC "okf.kinds"))
      (cell "tags" "Tags" "tags" 1 (badgesC "okf.tags"))
    ])
    (shelf 3 [
      (cell "assembly" "Assembly" "assembly" 3 (tableC "okf.assembly"))
      (cell "stages" "Stages" "stages" 1 (badgesC "okf.stages"))
    ])
    (shelf 3 [ (cell "physics" "Physics" "physics" 4 (tableC "okf.physics")) ])
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
    planner = "policy"; reviewer = "policy"; coder = "computation"; memory = "concept";
  };
  morphism_sorting = {
    contains = "contains"; references = "references"; derived_from = "derived_from";
    executed_by = "executed_by"; attested_by = "attested_by"; computes = "computes";
    continues = "computes"; shares = "executed_by"; consolidates = "references";
    produces = "executed_by"; verifies = "executed_by";
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
    # One badge per distinct first tag with its count. Bundles carry no tags,
    # so the strip restricts to the site's concept sorts.
    "okf.tags" = {
      kind = "badges";
      group = "values";
      value = "tag:0";
      restrict = [ "table" "dataset" "policy" "metric" "computation" "skill" "reference" "concept" "log" ];
    };
    "okf.kinds" = { kind = "gauges"; scope = "kinds"; value = "degree"; scale = degreeScale; };
    "okf.assembly" = { kind = "assembly"; morphism_sorts = [ "derived_from" "computes" "executed_by" "attested_by" "references" ]; kinds = [ "derived_from" "computes" "executed_by" "attested_by" "references" ]; };
    "okf.stages" = { kind = "assembly_badges"; morphism_sorts = [ "derived_from" "computes" "executed_by" "attested_by" "references" ]; kinds = [ "derived_from" "computes" "executed_by" "attested_by" "references" ]; };
    "okf.physics" = { kind = "physics"; scope = "objects"; };
    "okf.selected" = { kind = "text"; heading = "Selected concept"; };
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
