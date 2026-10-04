# omp auto-loop trace regime, over the trace site glued to the control site
# along `repo`. Sessions assemble over in_repo + pursues: sessions on one repo
# and goal bond. Control kinds other than `repo` sort to `elsewhere`, which no
# sheaf reads. Read-only: no edits. Design:
# .specs/design/topos-autoloop-2026-10-04.md.

{ lib ? (import <nixpkgs> { }).lib, surface, glyphs ? "unicode" }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  L = import ../lib/mkToposLayout.nix { inherit lib; };
  inherit (topos) mkTopos defaultTheme;
  inherit (L) cell shelf tableC badgesC textCell flex fixed;

  traceSorts = [ "session" "event" "goal" "repo" "gate" ];
  traceMorphisms = [ "in_repo" "pursues" "runs_gate" "spawned" "emitted" "next" ];
  controlKinds = [ "claim" "proposal" "queued_goal" "repo_policy" "term" "param" "prompt" ];
  to = value: names: lib.genAttrs names (_: value);
  identity = names: lib.genAttrs names (name: name);

  col = key: title: weight: expr: { inherit key title; width = flex weight; align = "left"; cell = textCell expr; };
  num = key: title: expr: { inherit key title; width = fixed 8; align = "right"; cell = textCell expr; };

  shelves = [
    (shelf 3 [
      (cell "sessions" "Sessions" "sessions" 3 (tableC "autoloop.sessions"))
      (cell "kinds" "Trace" "kinds" 1 (badgesC "autoloop.trace_kinds"))
    ])
    (shelf 3 [
      (cell "events" "Events" "events" 4 (tableC "autoloop.events"))
    ])
    (shelf 3 [
      (cell "assembly" "Assembly" "assembly" 3 (tableC "autoloop.assembly"))
      (cell "stages" "Stages" "stages" 1 (badgesC "autoloop.stages"))
    ])
    (shelf 3 [
      (cell "physics" "Physics" "physics" 4 (tableC "autoloop.physics"))
    ])
  ];
in
mkTopos {
  spec_version = 1;
  id = "topos-autoloop-trace";
  regime = "omp auto-loop trace";

  object_sorting = identity traceSorts // to "elsewhere" controlKinds;
  morphism_sorting = identity traceMorphisms // to "elsewhere" [ "targets" "governs" "revises" ];

  stalks = {
    objects = {
      session = { label = "Session"; badge = "entity"; };
      event = { label = "Event"; badge = "date"; };
      goal = { label = "Goal"; badge = "doctype"; };
      repo = { label = "Repository"; badge = "folder"; };
      gate = { label = "Gate"; badge = "status"; };
      elsewhere = { label = "Control"; badge = "generic"; };
    };
    morphisms = {
      in_repo = { label = "in repo"; };
      pursues = { label = "pursues"; };
      runs_gate = { label = "runs gate"; };
      spawned = { label = "spawned"; };
      emitted = { label = "emitted"; };
      next = { label = "next"; };
      elsewhere = { label = "control"; };
    };
    stages = {
      chain = "one session";
      branched = "sessions sharing a repo";
      ring = "sessions sharing a repo and goal";
      sheet = "sheet";
      tube = "tube";
      membrane = "membrane";
    };
  };

  sheaves = {
    "autoloop.sessions" = {
      kind = "table";
      scope = "objects";
      restrict = [ "session" ];
      columns = [
        (col "session" "Session" 2 "title")
        (col "goal" "Goal state" 1 "tag:0")
        (col "agent" "Agent" 1 "tag:1")
        (col "cwd" "Directory" 3 "field:cwd")
        # Older producers wrote sessions without these counters.
        (num "continuations" "Cont." "field?:continuations")
        (num "max" "Max" "field?:max_continuations")
        (num "heartbeats" "Beats" "field?:heartbeats")
      ];
    };
    "autoloop.events" = {
      kind = "table";
      scope = "objects";
      restrict = [ "event" ];
      columns = [
        (col "ts" "Time" 2 "field:ts")
        (col "session" "Session" 1 "field:sess")
        (col "class" "Class" 1 "tag:0")
        (col "event" "Event" 4 "title")
      ];
    };
    "autoloop.trace_kinds" = { kind = "badges"; group = "kinds"; restrict = traceSorts; };
    "autoloop.assembly" = { kind = "assembly"; morphism_sorts = [ "in_repo" "pursues" ]; };
    "autoloop.stages" = { kind = "assembly_badges"; morphism_sorts = [ "in_repo" "pursues" ]; };
    "autoloop.physics" = { kind = "physics"; scope = "objects"; };
  };

  workspace = L.mkWorkspace {
    inherit surface glyphs shelves;
    id = "topos-autoloop-trace-ws";
    persistKey = "panel_kit_topos_autoloop_trace_${surface}";
    theme = defaultTheme;
  };
}
