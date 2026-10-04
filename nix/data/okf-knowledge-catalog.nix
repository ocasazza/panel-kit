# The pinned OKF corpus as one JSON envelope for the `okf` grammar engine.
#
# `fetchFromGitHub` supplies the bytes; this file only *enumerates* them:
# `readDir` over `okf/bundles`, `readFile` per `*.md`. No YAML and no Markdown
# is parsed here — frontmatter and link semantics belong to
# `crates/panel-kit-grammar/src/okf.rs`, so a Nix change can never quietly
# redefine what a concept is. Bundles and their files are sorted by path, so
# the envelope is byte-identical on every evaluation.
#
#   json = import ./nix/data/okf-knowledge-catalog.nix { inherit pkgs; };
{ pkgs ? (import <nixpkgs> {}), lib ? pkgs.lib }:
let
  repository = "GoogleCloudPlatform/knowledge-catalog";
  rev = "58e16bdb7a34430f055ea57e84655cff37000c03";
  corpus = pkgs.fetchFromGitHub {
    owner = "GoogleCloudPlatform";
    repo = "knowledge-catalog";
    inherit rev;
    hash = "sha256-WwZSi5m9T2yUjEQJZH0L7lcUFb/IKvyD1HisAjZ/3nc=";
  };
  bundlesRoot = "${corpus}/okf/bundles";

  # Every `*.md` under `dir`, as bundle-relative `{ path, text }` records
  # sorted by path. Subdirectories are walked depth-first; anything that is
  # neither a directory nor a Markdown file is dropped. `readDir` maps each
  # entry name to its kind ("directory", "regular", "symlink").
  markdownFiles = dir: prefix:
    let
      entries = builtins.readDir dir;
      byName = name:
        let
          relative = if prefix == "" then name else "${prefix}/${name}";
        in
        if entries.${name} == "directory" then
          markdownFiles "${dir}/${name}" relative
        else if entries.${name} == "regular" && lib.hasSuffix ".md" name then
          [ { path = relative; text = builtins.readFile "${dir}/${name}"; } ]
        else
          [ ];
    in
    builtins.sort (left: right: left.path < right.path)
      (builtins.concatLists (map byName (builtins.attrNames entries)));
in
builtins.toJSON {
  inherit repository rev;
  bundles =
    map
      (name: {
        inherit name;
        files = markdownFiles "${bundlesRoot}/${name}" "";
      })
      (builtins.sort builtins.lessThan (builtins.attrNames (builtins.readDir bundlesRoot)));
}
