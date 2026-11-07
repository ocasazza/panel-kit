# JSON grammar for the merged trio site: agent sessions and the OKF objects
# they produce, verify, and read. Resolves /nodes and /edges arrays and maps
# object members by name. Produces the same site as
# nix/grammars/trace-lines.nix over nix/data/trace.* (trace.json is generated
# from trace.lines, which is the source of truth).

{ lib ? (import <nixpkgs> { }).lib }:

let
  mkGrammarPackage = import ../lib/mkGrammarPackage.nix { inherit lib; };
in
mkGrammarPackage {
  format_version = 1;
  metadata = {
    id = "trace-json";
    name = "Trace (json)";
    version = "1.0.0";
    description = "JSON trio site: { nodes: [...], edges: [...] }.";
  };
  limits = {
    grammar_bytes = 1024;
    input_bytes = 262144;
    nodes = 1000;
    edges = 5000;
  };
  parser = {
    engine = "json";
    nodes = "/nodes";
    edges = "/edges";
    node = {
      id = "id";
      title = "title";
      kind = "kind";
      tags = "tags";
      fields = [ "latency_ms" "energy" "t" "status" "trust" "stale_after" ];
    };
    edge = {
      source = "source";
      target = "target";
      kind = "kind";
    };
  };
}