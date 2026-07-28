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
    let
      # Systems the Schrodinger Hydra farm can actually build. Deliberately a
      # subset of eachDefaultSystem: the farm has no x86_64-darwin machine, and
      # a job with no capable builder is not "pending", it is permanently red
      # (Hydra reports it as unsupported(9)).
      #
      # aarch64-linux is omitted for a different reason — the only builder is a
      # 1-job GCP VM, and this crate cross-compiles to wasm32 so its output does
      # not vary by host anyway. x86_64-linux carries the farm's capacity;
      # aarch64-darwin is kept because that is what developers here build on, so
      # a toolchain break on macOS should turn CI red rather than surface as a
      # local surprise.
      hydraSystems = [ "x86_64-linux" "aarch64-darwin" ];

      perSystem = flake-utils.lib.eachDefaultSystem (system:
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

        # Declarative layout DSL: turns a Nix description of a panel
        # workspace into JSON matching panel-kit-core's `SavedLayout<K>`
        # serde schema (see nix/lib/mkLayout.nix). Renderer-agnostic, so the
        # web (localStorage) and TUI (JSON file) shells can both seed from it.
        mkLayoutLib = import ./nix/lib/mkLayout.nix { inherit (pkgs) lib; };

        # The canary layout, evaluated from nix/examples/workspace-canary.nix
        # (a declarative mirror of the TUI canary's `defaults()`).
        canaryLayout = import ./nix/examples/workspace-canary.nix {
          inherit (pkgs) lib;
        };

        # `nix build .#layout-canary` writes this SavedLayout JSON file — the
        # concrete demonstration of the DSL.
        layout-canary = pkgs.writeText "panel-kit-layout-canary.json"
          canaryLayout.json;
      in {
        packages.default = panel-kit;
        packages.layout-canary = layout-canary;

        # Version bumper for semantic-release's @semantic-release/exec step
        # (`nix run .#update-version -- 1.2.3`), mirroring how nixstation
        # drives its own lib/version.nix.
        #
        # cargo set-version rather than sed: this is a workspace, so a bump has
        # to touch the root package, both member crates, AND the `version` field
        # of the path-dependency on panel-kit-core — which a naive
        # search-and-replace gets wrong as soon as two crates disagree on
        # version. It rewrites Cargo.lock in the same pass.
        packages.update-version = pkgs.writeShellApplication {
          name = "update-version";
          runtimeInputs = [ pkgs.cargo-edit rustWasm ];
          text = ''
            if [ $# -ne 1 ]; then
              echo "usage: update-version <semver>" >&2
              exit 1
            fi
            cargo set-version --workspace "$1"
          '';
        };

        # mkLayout for downstream flakes:
        # `inputs.panel-kit.lib.${system}.mkLayout { ... }`.
        lib = { inherit (mkLayoutLib) mkLayout winStates tileWMax tileHMax; };

        checks = {
          inherit panel-kit;
          # Schema sanity check: the generated canary JSON must parse and
          # carry the exact SavedLayout / PanelWin keys the Rust serde
          # deserializer expects.
          layout-canary-schema =
            pkgs.runCommand "panel-kit-layout-canary-schema"
              { nativeBuildInputs = [ pkgs.jq ]; } ''
              json=${layout-canary}
              jq -e '
                (.tiling | type == "boolean") and
                (.panels | type == "array") and
                (.panels | length == 7) and
                (.panels | all(
                  (.kind | type == "string") and
                  (.x | type == "number") and (.y | type == "number") and
                  (.w | type == "number") and (.h | type == "number") and
                  (.state | IN("Floating", "Minimized", "Maximized")) and
                  (.z | type == "number") and
                  (.tile_w | type == "number" and . >= 1 and . <= 4) and
                  (.tile_h | type == "number" and . >= 1 and . <= 6) and
                  ((keys | sort) == ["h","kind","state","tile_h","tile_w","w","x","y","z"])
                ))
              ' "$json" > /dev/null
              touch $out
            '';
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
    in
    perSystem // {
      # What Hydra builds. Hydra's flake jobsets evaluate the `hydraJobs`
      # output specifically — `checks` alone is invisible to it — so this
      # re-exports the same five checks (the crane build, clippy, rustdoc, the
      # browser_tui example, and the layout-canary schema check) per buildable
      # system. Job names come out as `<system>.<check>`.
      #
      # Jobsets themselves are declared in hydra-project.json; the project is
      # registered in schrodinger/hydra .hydra/declarative-projects.json.
      hydraJobs =
        let
          perSys = nixpkgs.lib.genAttrs hydraSystems (system: perSystem.checks.${system});
        in
        perSys // {
          # Single green/red summary over every check on every system.
          #
          # This is what the release pipeline hangs off. Hydra's RunCommand
          # plugin fires once per BUILD, so hooking it to a wildcard job
          # matcher would trigger a release ten times per commit (five checks
          # times two systems). An aggregate is one build that succeeds only if
          # all its constituents did, so `panel-kit:main:release` fires exactly
          # once — and only when everything is genuinely green.
          #
          # releaseTools.aggregate marks the job `_hydraAggregate`, which is
          # how Hydra knows to wait for the constituents rather than treat this
          # as an ordinary (and trivially empty) derivation.
          release =
            (import nixpkgs { system = "x86_64-linux"; }).releaseTools.aggregate {
              name = "panel-kit-release";
              constituents =
                builtins.concatMap builtins.attrValues (builtins.attrValues perSys);
            };
        };
    };
}
