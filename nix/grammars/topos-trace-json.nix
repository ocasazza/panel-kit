# JSON grammar for the agent trace graph. Resolves /nodes and /edges arrays
# and maps object members by name. Produces the same graph as
# nix/grammars/trace-lines.nix over nix/data/trace.*.

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
    description = "JSON agent trace: { nodes: [...], edges: [...] }.";
  };
  limits = {
    grammar_bytes = 1024;
    input_bytes = 131072;
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
      fields = [ "latency_ms" "energy" "t" "status" ];
    };
    edge = {
      source = "source";
      target = "target";
      kind = "kind";
    };
  };
}
