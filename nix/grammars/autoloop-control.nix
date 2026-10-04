# JSON grammar for the omp auto-loop control site (GET /api/site):
#   { "objects": [{ id, title, sort, tags, <field>... }],
#     "morphisms": [{ domain, codomain, sort }] }
# `fields` must list every record field the loop serves (its SITE_FIELDS): a
# member missing here is dropped on parse, and the emitter refuses to write it.
# The emitter writes revised records back for POST /api/site.

{ lib ? (import <nixpkgs> { }).lib }:

let
  mkGrammarPackage = import ../lib/mkGrammarPackage.nix { inherit lib; };
in
mkGrammarPackage {
  format_version = 1;
  metadata = {
    id = "autoloop-control";
    name = "omp auto-loop control";
    version = "1.0.0";
    description = "The omp auto-loop control site: claims, proposals, queued goals, repos and their policy, terms, params and prompts.";
  };
  limits = {
    grammar_bytes = 2048;
    input_bytes = 4194304;
    nodes = 20000;
    edges = 20000;
  };
  parser = {
    engine = "json";
    nodes = "/objects";
    edges = "/morphisms";
    node = {
      id = "id";
      title = "title";
      kind = "sort";
      tags = "tags";
      fields = [
        "after" "at" "before" "category" "claimed_by" "count" "criteria" "decision" "description"
        "gates" "hidden" "judge" "max" "min" "note" "priority" "rationale" "reason" "repo" "root"
        "session" "source" "status" "target" "term" "text" "ts" "value" "vocabulary"
      ];
    };
    edge = {
      source = "domain";
      target = "codomain";
      kind = "sort";
    };
  };
  emitter = { engine = "json"; };
}
