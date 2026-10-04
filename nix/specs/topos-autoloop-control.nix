# omp auto-loop control regime, over the control site glued to the trace site
# along `repo`. Each table is one control sort with the edits its loop accepts;
# every edit writes back through the autoloop-control grammar's emitter. Trace
# kinds sort to `elsewhere`, which no sheaf reads. Design:
# .specs/design/topos-autoloop-2026-10-04.md.

{ lib ? (import <nixpkgs> { }).lib, surface, glyphs ? "unicode" }:

let
  topos = import ../lib/mkToposSpec.nix { inherit lib; };
  L = import ../lib/mkToposLayout.nix { inherit lib; };
  inherit (topos) mkTopos defaultTheme;
  inherit (L) cell shelf tableC badgesC textCell flex fixed;

  grammar = "autoloop-control";
  controlSorts = [ "claim" "proposal" "queued_goal" "repo" "repo_policy" "term" "param" "prompt" ];
  traceKinds = [ "session" "event" "goal" "gate" ];
  traceMorphisms = [ "in_repo" "pursues" "runs_gate" "spawned" "emitted" "next" ];
  to = value: names: lib.genAttrs names (_: value);
  identity = names: lib.genAttrs names (name: name);

  col = key: title: weight: expr: { inherit key title; width = flex weight; align = "left"; cell = textCell expr; };
  num = key: title: expr: { inherit key title; width = fixed 8; align = "right"; cell = textCell expr; };
  table = sort: columns: edits: { kind = "table"; scope = "objects"; restrict = [ sort ]; inherit columns edits; };

  shelves = [
    (shelf 3 [
      (cell "claims" "Claims" "claims" 3 (tableC "autoloop.claims"))
      (cell "kinds" "Control state" "kinds" 1 (badgesC "autoloop.kinds"))
    ])
    (shelf 3 [
      (cell "queue" "Queue" "queue" 2 (tableC "autoloop.queue"))
      (cell "proposals" "Proposals" "proposals" 2 (tableC "autoloop.proposals"))
    ])
    (shelf 3 [
      (cell "repos" "Repositories" "repos" 2 (tableC "autoloop.repos"))
      (cell "limits" "Limits" "limits" 2 (tableC "autoloop.limits"))
    ])
    (shelf 3 [
      (cell "prompts" "Prompts" "prompts" 2 (tableC "autoloop.prompts"))
      (cell "terms" "Vocabulary" "terms" 2 (tableC "autoloop.terms"))
    ])
  ];
in
mkTopos {
  spec_version = 1;
  id = "topos-autoloop-control";
  regime = "omp auto-loop control";

  object_sorting = identity controlSorts // to "elsewhere" traceKinds;
  morphism_sorting = identity [ "targets" "governs" "revises" ] // to "elsewhere" traceMorphisms;

  stalks = {
    objects = {
      claim = { label = "Claim"; badge = "status"; };
      proposal = { label = "Proposal"; badge = "doctype"; };
      queued_goal = { label = "Queued goal"; badge = "tag"; };
      repo = { label = "Repository"; badge = "folder"; };
      repo_policy = { label = "Repository policy"; badge = "status"; };
      term = { label = "Term"; badge = "tag"; };
      param = { label = "Limit"; badge = "generic"; };
      prompt = { label = "Prompt"; badge = "doctype"; };
      elsewhere = { label = "Trace"; badge = "generic"; };
    };
    morphisms = {
      targets = { label = "targets"; };
      governs = { label = "governs"; };
      revises = { label = "revises"; };
      elsewhere = { label = "trace"; };
    };
    stages = {
      chain = "chain";
      branched = "branched";
      ring = "ring";
      sheet = "sheet";
      tube = "tube";
      membrane = "membrane";
    };
  };

  sheaves = {
    "autoloop.claims" = table "claim" [
      (col "claim" "Goal" 3 "title")
      (col "session" "Session" 2 "field:session")
      (col "reason" "Settled" 2 "field:reason")
      (col "judge" "Judge" 2 "field:judge")
      (col "decision" "Decision" 1 "field:decision")
      (col "note" "Note" 2 "field:note")
    ] [ "ratify" ];
    "autoloop.proposals" = table "proposal" [
      (col "target" "Target" 2 "title")
      (col "before" "Before" 1 "field:before")
      (col "after" "After" 1 "field:after")
      (col "rationale" "Rationale" 3 "field:rationale")
      (col "status" "Status" 1 "field:status")
    ] [ "decide" ];
    "autoloop.queue" = table "queued_goal" [
      (col "goal" "Goal" 3 "title")
      (col "repo" "Repository" 2 "field:repo")
      (num "priority" "Priority" "field:priority")
      (col "status" "Status" 1 "field:status")
      (col "claimed_by" "Claimed by" 2 "field:claimed_by")
    ] [ "prioritize" "retire" ];
    "autoloop.repos" = table "repo_policy" [
      (col "repo" "Repository" 2 "title")
      (col "verification" "Verification" 1 "tag:0")
      (col "gates" "Gates" 3 "field:gates")
      (col "criteria" "Done criteria" 3 "field:criteria")
    ] [ "set_gates" "set_criteria" ];
    "autoloop.limits" = table "param" [
      (col "limit" "Limit" 3 "title")
      (num "value" "Value" "field:value")
      (num "min" "Min" "field:min")
      (num "max" "Max" "field:max")
    ] [ "set_value" ];
    "autoloop.prompts" = table "prompt" [
      (col "prompt" "Prompt" 1 "title")
      (col "text" "Text" 4 "field:text")
    ] [ "set_text" ];
    "autoloop.terms" = table "term" [
      (col "term" "Term" 2 "title")
      (col "vocabulary" "Vocabulary" 1 "field:vocabulary")
      (col "usage" "Usage" 1 "tag:1")
      (num "count" "Count" "field:count")
      (col "hidden" "Hidden" 1 "field:hidden")
      (col "description" "Description" 3 "field:description")
    ] [ "describe" "hide" ];
    "autoloop.kinds" = { kind = "badges"; group = "kinds"; restrict = controlSorts; };
  };

  edits = {
    ratify = {
      label = "Ratify";
      sorts = [ "claim" ];
      set = { decision = { type = "enum"; values = [ "accepted" "overturned" ]; }; note = { type = "text"; max = 400; }; };
      inherit grammar;
    };
    decide = {
      label = "Decide";
      sorts = [ "proposal" ];
      set.status = { type = "enum"; values = [ "accepted" "rejected" ]; };
      inherit grammar;
    };
    prioritize = {
      label = "Prioritize";
      sorts = [ "queued_goal" ];
      set.priority = { type = "int"; min = -1000000; max = 1000000; };
      inherit grammar;
    };
    retire = {
      label = "Retire";
      sorts = [ "queued_goal" ];
      set.status = { type = "enum"; values = [ "retired" ]; };
      inherit grammar;
    };
    set_gates = {
      label = "Set gates";
      sorts = [ "repo_policy" ];
      set.gates = { type = "lines"; max = 32; };
      inherit grammar;
    };
    set_criteria = {
      label = "Set done criteria";
      sorts = [ "repo_policy" ];
      set.criteria = { type = "text"; max = 8000; };
      inherit grammar;
    };
    set_value = {
      label = "Set";
      sorts = [ "param" ];
      # Widest param range (timeoutMs, 6 h); the loop checks each param's own bounds.
      set.value = { type = "int"; min = 0; max = 21600000; };
      inherit grammar;
    };
    set_text = {
      label = "Edit";
      sorts = [ "prompt" ];
      set.text = { type = "text"; max = 8000; };
      inherit grammar;
    };
    describe = {
      label = "Describe";
      sorts = [ "term" ];
      set.description = { type = "text"; max = 400; };
      inherit grammar;
    };
    hide = {
      label = "Hide";
      sorts = [ "term" ];
      set.hidden = { type = "enum"; values = [ "true" "false" ]; };
      inherit grammar;
    };
  };

  workspace = L.mkWorkspace {
    inherit surface glyphs shelves;
    id = "topos-autoloop-control-ws";
    persistKey = "panel_kit_topos_autoloop_control_${surface}";
    theme = defaultTheme;
  };
}
