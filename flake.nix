{
  description = "panel-kit — generic Dioxus panel-workspace library (wasm32)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # nixos-25.05 is the last channel shipping dioxus-cli 0.6.x, which must
    # match the dioxus 0.6 the library (and its examples) build against.
    nixpkgs-dioxus.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, nixpkgs-dioxus, flake-utils, crane, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        pkgsDioxus = import nixpkgs-dioxus { inherit system; };
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
            ./crates
            ./assets # panel-kit.css is include_str!'d into the lib
            ./examples # one browser demo per component, clippy'd by checks
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
          browser-tui-example = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
            cargoExtraArgs = "-p panel-kit-tui --example browser_tui";
          });
          # Docs must build clean (missing_docs is warn-level in lib.rs;
          # -D warnings promotes it + broken intra-doc links to errors).
          doc = craneLib.cargoDoc (commonArgs // {
            inherit cargoArtifacts;
            RUSTDOCFLAGS = "-D warnings";
          });
        };

        devShells.default = pkgs.mkShell {
          packages = [
            rustWasm
            # `dx serve --example <name> --platform web` runs the demos.
            # dx 0.6 shells out to lld for debug wasm links and expects a
            # wasm-bindgen-cli on PATH matching Cargo.lock's wasm-bindgen
            # (0.2.121 — kept in lockstep with nixpkgs' wasm-bindgen-cli).
            pkgsDioxus.dioxus-cli
            pkgs.trunk
            pkgs.wasm-bindgen-cli
            pkgs.lld
          ];
        };
      });
}
