# OKF grammar for the GoogleCloudPlatform/knowledge-catalog bundles.
#
# OKF has no pest grammar, so its rules are data: `kinds` routes the
# frontmatter `type` every concept must carry, `link_rules` names the
# path-valued frontmatter fields OKF §6.2 permits, and `body_links` types
# CommonMark body links. `fields` projects the spec's own frontmatter
# vocabulary; the trust tier is derived by the engine, not projected.
#
# `kinds` enumerates every `type:` value the four pinned bundles carry; an
# unregistered OKF type is not malformed (§4.1), it takes `default_kind`.

{ lib ? (import <nixpkgs> { }).lib }:

let
  mkGrammarPackage = import ../lib/mkGrammarPackage.nix { inherit lib; };
in
mkGrammarPackage {
  format_version = 1;
  metadata = {
    id = "okf";
    name = "Open Knowledge Format";
    version = "1.0.0";
    description = "The Open Knowledge Format bundles of GoogleCloudPlatform/knowledge-catalog: bundles of YAML-frontmatter Markdown concepts, linked by provenance, execution and citation paths.";
  };
  limits = {
    # The rule table is the grammar here; the corpus is the input.
    grammar_bytes = 8192;
    input_bytes = 8388608;
    nodes = 4096;
    edges = 16384;
  };
  parser = {
    engine = "okf";
    # `Log` maps to no object at this rev: the corpus' only `Log` concept is
    # `acme_retail/log.md`, and `log.md` is reserved (§3.1). The mapping stays
    # so a corpus that ships a real log document still routes.
    kinds = {
      "Attested Computation" = "computation";
      "BigQuery Dataset" = "dataset";
      "BigQuery Table" = "table";
      "Log" = "log";
      "Metric" = "metric";
      "Policy" = "policy";
      "Reference" = "reference";
      "Skill" = "skill";
    };
    # OKF keeps no closed registry of type values (§4.1), so a type this
    # table does not map is a generic concept rather than an error.
    default_kind = "concept";
    # The OKF §6.2 path-valued fields. `sources[*].resource` iterates a
    # sequence; `attester.resource` and `executor.resource` walk a mapping;
    # a bare `resource` reads the concept's own asset URI.
    link_rules = {
      "attester.resource" = "attested_by";
      "computation" = "computes";
      "executor.resource" = "executed_by";
      "resource" = "references";
      "sources[*].resource" = "derived_from";
    };
    body_links = "references";
    # The spec's own frontmatter vocabulary minus the three keys the engine
    # maps structurally (`type`, `title`, `tags`) and the derived `trust`.
    fields = [
      "attester"
      "description"
      "executor"
      "generated"
      "parameters"
      "resource"
      "runtime"
      "sources"
      "stale_after"
      "status"
      "usage_window"
      "verified"
    ];
    bundle_kind = "bundle";
    contains_kind = "contains";
  };
}
