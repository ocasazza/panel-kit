# Pest grammar for the omp auto-loop projection (graph.lines):
#   N|id|title|kind|tags|props[|body]
#   E|source|target[|kind]
# The grammar text is jump-cannon's charts/jump-cannon/packages/omp-auto-loop.toml,
# so both read the same file identically. The trailing body is accepted and not
# captured; a bare edge stays untyped. Read-only: no emitter.

{ lib ? (import <nixpkgs> { }).lib }:

let
  mkGrammarPackage = import ../lib/mkGrammarPackage.nix { inherit lib; };

  grammar = ''
    document = { SOI ~ (record ~ NEWLINE?)* ~ EOI }
    record = _{ node | edge }
    node = { "N|" ~ node_id ~ "|" ~ title ~ "|" ~ kind ~ "|" ~ tags ~ "|" ~ properties ~ body? }
    node_id = @{ field }
    title = @{ field }
    kind = @{ field }
    tags = _{ (tag ~ ("," ~ tag)*)? }
    tag = @{ atom }
    properties = _{ (property ~ (";" ~ property)*)? }
    property = { key ~ "=" ~ value }
    key = @{ atom }
    value = @{ atom }
    body = _{ "|" ~ body_text }
    body_text = @{ (!NEWLINE ~ ANY)+ }
    edge = { "E|" ~ source ~ "|" ~ target ~ ("|" ~ edge_kind)? }
    source = @{ field }
    target = @{ field }
    edge_kind = @{ atom }
    field = _{ (!("|" | NEWLINE) ~ ANY)+ }
    atom = _{ (!("," | ";" | "=" | "|" | NEWLINE) ~ ANY)+ }
  '';
in
mkGrammarPackage {
  format_version = 1;
  metadata = {
    id = "autoloop-trace";
    name = "omp auto-loop trace";
    version = "1.0.0";
    description = "The omp auto-loop projection: session, event, goal, repo and gate objects with typed edges.";
  };
  limits = {
    grammar_bytes = 4096;
    input_bytes = 33554432;
    nodes = 100000;
    edges = 500000;
  };
  parser = {
    engine = "pest";
    root_rule = "document";
    inherit grammar;
    captures = {
      node = "node";
      id = "node_id";
      title = "title";
      kind = "kind";
      tag = "tag";
      property = "property";
      key = "key";
      value = "value";
      edge = "edge";
      source = "source";
      target = "target";
      edge_kind = "edge_kind";
    };
  };
}
