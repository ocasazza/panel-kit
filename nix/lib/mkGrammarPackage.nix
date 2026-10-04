# Grammar-package producer. Assembles a `panel-kit-grammar` GrammarPackage
# (format_version 1) and returns it as a Nix value plus its JSON serialization.
# The crate's `GrammarPackage::from_json_str`/`compile` enforce the full
# contract; this layer only shapes the envelope.
#
#   grammar = import ./mkGrammarPackage.nix { inherit lib } {
#     metadata = { id; name; version; description; };
#     limits   = { grammar_bytes; input_bytes; nodes; edges; };
#     parser   = { engine = "pest"; root_rule; grammar; captures; };
#   };
#   # grammar.value -> the GrammarPackage attrset
#   # grammar.json  -> the JSON string (pkgs.writeText'd by the flake)
#   # grammar.id    -> the package id

{ lib ? (import <nixpkgs> { }).lib }:

spec:
let
  value = {
    format_version = spec.format_version or 1;
    metadata = spec.metadata;
    limits = spec.limits;
    parser = spec.parser;
  };
in
{
  inherit value;
  json = builtins.toJSON value;
  id = spec.metadata.id;
}
