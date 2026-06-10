{
  description = "panel-kit — generic Dioxus panel-workspace library (wasm32)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, crane, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        # The library only ever compiles to wasm32 (Dioxus web) — check it
        # for the target its consumers (jump-cannon, apple-notes-ocr-flow)
        # actually build.
        rustWasm = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain rustWasm;
        src = pkgs.lib.fileset.toSource {
          root = ./.;
          fileset = pkgs.lib.fileset.unions [
            ./Cargo.toml
            ./Cargo.lock
            ./src
            ./assets # panel-kit.css is include_str!'d into the lib
          ];
        };
        commonArgs = {
          inherit src;
          strictDeps = true;
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          doCheck = false; # no test runner on bare wasm32
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        panel-kit = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });
      in {
        packages.default = panel-kit;

        checks = {
          inherit panel-kit;
          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- -D warnings";
          });
        };

        devShells.default = pkgs.mkShell {
          packages = [ rustWasm ];
        };
      });
}
