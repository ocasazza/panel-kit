# Agentic-trace regime over the session site: objects are planner/coder/reviewer
# sessions plus memory stores; morphisms are handoffs. Identity sorting (every
# site kind maps to its own regime sort). Sheaves: Stalks text, sessions Status
# table, handoff Flamegraph over `continues`, Roles badges (per sort), Assembly
# table + stage badges, Physics table, Morphism + Invariants host overlays. `surface`/`glyphs`
# select units/chrome/glyphs; the stalks, sheaves, and binding ids do not vary.

{ lib ? (import <nixpkgs> { }).lib, surface, glyphs ? "unicode" }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  L = import ../lib/mkToposLayout.nix { inherit lib; };
  inherit (topos) mkTopos defaultTheme;
  inherit (L) cell shelf textC tableC badgesC flameC textCell flex fixed;

  shelves = [
    (shelf 3 [
      (cell "stalks" "Stalks" "stalks" 1 (textC "agentic.stalks"))
      (cell "status" "Sessions" "status" 3 (tableC "agentic.status"))
    ])
    (shelf 3 [
      (cell "flame" "Handoffs" "flame" 3 (flameC "agentic.flame"))
      (cell "roles" "Roles" "roles" 1 (badgesC "agentic.roles"))
    ])
    (shelf 3 [
      (cell "assembly" "Assembly" "assembly" 3 (tableC "agentic.assembly"))
      (cell "stages" "Stages" "stages" 1 (badgesC "agentic.stages"))
    ])
    (shelf 3 [
      (cell "physics" "Physics" "physics" 2 (tableC "agentic.physics"))
      (cell "morphism" "Morphism" "morphism" 1 (textC "agentic.morphism"))
      (cell "invariants" "Invariants" "invariants" 1 (tableC "agentic.invariants"))
    ])
  ];
in
mkTopos {
  spec_version = 1;
  id = "topos-agentic";
  regime = "Agentic traces";

  object_sorting = { planner = "planner"; coder = "coder"; reviewer = "reviewer"; memory = "memory"; };
  morphism_sorting = { continues = "continues"; shares = "shares"; consolidates = "consolidates"; };

  stalks = {
    objects = {
      planner = { label = "Planner"; badge = "status"; };
      coder = { label = "Coder"; badge = "entity"; };
      reviewer = { label = "Reviewer"; badge = "tag"; };
      memory = { label = "Memory"; badge = "folder"; };
    };
    morphisms = {
      continues = { label = "continues"; };
      shares = { label = "shares"; };
      consolidates = { label = "consolidates"; };
    };
    stages = {
      chain = "handoff chain";
      branched = "handoff fan";
      ring = "handoff loop";
      sheet = "team sheet";
      tube = "pipeline tube";
      membrane = "closed org";
    };
  };

  sheaves = {
    "agentic.stalks" = {
      kind = "text";
      heading = "Agentic-trace regime";
      preface = [
        "Objects are planner/coder/reviewer sessions and memory stores; morphisms are handoffs."
        "Components self-assemble over continues+shares: chain, ring, sheet, tube, membrane."
        "Stalks: object sort -> label [badge]; morphism sort -> label."
      ];
      include_stalks = true;
    };
    "agentic.status" = {
      kind = "table";
      scope = "objects";
      columns = [
        { key = "session"; title = "Session"; width = flex 3; align = "left"; cell = textCell "title"; }
        { key = "sort"; title = "Sort"; width = flex 2; align = "left"; cell = textCell "kind"; }
        {
          key = "latency";
          title = "Latency";
          width = flex 3;
          align = "left";
          cell = { cell = "meter"; value = "field:latency_ms"; scale = 60.0; text = "field:latency_ms"; };
        }
        {
          key = "status";
          title = "Status";
          width = fixed 10;
          align = "center";
          cell = { cell = "status"; from = "field:status"; };
        }
      ];
    };
    "agentic.flame" = {
      kind = "flamegraph";
      morphism_kind = "continues";
      value = "field:latency_ms";
    };
    "agentic.roles" = { kind = "badges"; group = "kinds"; };
    "agentic.assembly" = { kind = "assembly"; morphism_sorts = [ "continues" "shares" ]; };
    "agentic.stages" = { kind = "assembly_badges"; morphism_sorts = [ "continues" "shares" ]; };
    "agentic.physics" = { kind = "physics"; scope = "objects"; };
    "agentic.morphism" = { kind = "morphism"; };
    "agentic.invariants" = { kind = "invariants"; };
  };

  workspace = L.mkWorkspace {
    inherit surface glyphs shelves;
    id = "topos-agentic-ws";
    persistKey = "panel_kit_topos_agentic_${surface}";
    theme = defaultTheme;
  };
}
