# Pest grammar for the pipe-delimited agent trace:
#   N|id|title|kind|tag,tag|key=value;key=value
#   E|source|target|edge_kind
# Produces the same graph as nix/grammars/trace-json.nix over nix/data/trace.*.

{ lib ? (import <nixpkgs> { }).lib }:

let
  mkGrammarPackage = import ../lib/mkGrammarPackage.nix { inherit lib; };

  grammar = ''
    file = { SOI ~ (record ~ NEWLINE)* ~ record? ~ EOI }
    record = _{ node | edge }

    node = { "N" ~ "|" ~ id ~ "|" ~ title ~ "|" ~ kind ~ "|" ~ tags ~ "|" ~ props }
    edge = { "E" ~ "|" ~ source ~ "|" ~ target ~ "|" ~ edge_kind }

    id = { field }
    title = { field }
    kind = { field }
    source = { field }
    target = { field }
    edge_kind = { field }

    tags = _{ (tag ~ ("," ~ tag)*)? }
    tag = { tagchar+ }

    props = _{ (property ~ (";" ~ property)*)? }
    property = { key ~ "=" ~ value }
    key = { keychar+ }
    value = { valchar+ }

    field = { fieldchar+ }
    fieldchar = _{ !("|" | NEWLINE) ~ ANY }
    tagchar = _{ !("," | "|" | NEWLINE) ~ ANY }
    keychar = _{ !("=" | ";" | "|" | NEWLINE) ~ ANY }
    valchar = _{ !(";" | "|" | NEWLINE) ~ ANY }
  '';
in
mkGrammarPackage {
  format_version = 1;
  metadata = {
    id = "trace-lines";
    name = "Trace (lines)";
    version = "1.0.0";
    description = "Pipe-delimited agent trace: N|id|title|kind|tags|props and E|src|dst|kind.";
  };
  limits = {
    grammar_bytes = 8192;
    input_bytes = 65536;
    nodes = 1000;
    edges = 5000;
  };
  parser = {
    engine = "pest";
    root_rule = "file";
    inherit grammar;
    captures = {
      node = "node";
      id = "id";
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
