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
        rustHost = pkgs.rust-bin.stable.latest.default;
        wasmCraneLib = (crane.mkLib pkgs).overrideToolchain rustWasm;
        src = pkgs.lib.fileset.toSource {
          root = ./.;
          fileset = pkgs.lib.fileset.unions [
            ./Cargo.toml
            ./Cargo.lock
            ./build.rs # generates OUT_DIR/panel-kit-boot.css from core tokens
            ./DESIGN.md # generated token region read by build.rs during crane builds
            ./src
            ./crates
            ./assets # panel-kit.css + panel-kit-boot.css.in, include_str!'d/generated
            ./tools # workspace member manifest only; wasm builds still target panel-kit
            ./examples # one browser demo per component, clippy'd by checks
          ];
        };
        hostSrc = pkgs.lib.fileset.toSource {
          root = ./.;
          fileset = pkgs.lib.fileset.unions [
            ./Cargo.toml
            ./Cargo.lock
            ./build.rs
            ./DESIGN.md
            ./src
            ./crates
            ./assets
            ./examples
            ./tools
            ./nix # schema, producer, and repository specs used by native parity/tool tests
          ];
        };
        commonArgs = {
          inherit src;
          strictDeps = true;
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          doCheck = false; # no test runner on bare wasm32
        };
        cargoArtifacts = wasmCraneLib.buildDepsOnly commonArgs;
        panel-kit = wasmCraneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });
        hostCraneLib = (crane.mkLib pkgs).overrideToolchain rustHost;
        hostArgs = (builtins.removeAttrs commonArgs [ "CARGO_BUILD_TARGET" ]) // {
          src = hostSrc;
          doCheck = true;
        };
        hostCargoArtifacts = hostCraneLib.buildDepsOnly hostArgs;

        mkLayoutLib = import ./nix/lib/mkLayout.nix { inherit (pkgs) lib; };
        workspaceSpecSchema = builtins.fromJSON (
          builtins.readFile ./nix/schema/workspace-spec.schema.json
        );
        mkWorkspaceSpecLib = import ./nix/lib/mkWorkspaceSpec.nix {
          inherit (pkgs) lib;
          inherit workspaceSpecSchema;
        };
        workspaceCanary = import ./nix/specs/workspace-canary.nix {
          inherit (pkgs) lib;
        };
        workspaceCanaryAscii = mkWorkspaceSpecLib.mkWorkspaceSpec
          (workspaceCanary.value // {
            glyphs = "ascii";
          });
        workspace-canary = pkgs.writeText "panel-kit-workspace-canary.json"
          workspaceCanary.json;
        workspace-canary-ascii = pkgs.writeText "panel-kit-workspace-canary-ascii.json"
          workspaceCanaryAscii.json;
        webWorkspace = import ./nix/specs/web-workspace.nix {
          inherit (pkgs) lib;
        };
        web-workspace = pkgs.writeText "panel-kit-web-workspace.json"
          webWorkspace.json;
        web-workspace-provider-manifest = pkgs.writeText
          "panel-kit-web-workspace-provider-manifest.json"
          (builtins.readFile ./tools/spec-parity/fixtures/web-workspace-provider-manifest.json);
        onePanelWorkspace = mkWorkspaceSpecLib.mkWorkspaceSpec (workspaceCanary.value // {
          id = "workspace-canary-one-panel";
          chrome = workspaceCanary.value.chrome // {
            hit_target_min = 0.0;
            panel_header_h = 0.0;
            panel_frame = false;
            title_in_border = false;
            mode_control = false;
            minimize_control = false;
            maximize_control = false;
            resize_grip = false;
            dock = false;
            dock_label = "";
          };
          persistence = workspaceCanary.value.persistence // {
            key = "panel_kit_canary_one_panel";
            restore = false;
            save_policy = "manual";
          };
          panels = [ (builtins.elemAt workspaceCanary.value.panels 0) ];
        });
        workspace-one-panel = pkgs.writeText "panel-kit-workspace-one-panel.json"
          onePanelWorkspace.json;
        workspaceSpecReference = import ./nix/tests/workspace-spec.nix {
          inherit (pkgs) lib;
        };
        workspace-spec-reference = pkgs.writeText "panel-kit-workspace-spec-reference.json"
          workspaceSpecReference.json;
        specParityTool = hostCraneLib.buildPackage (hostArgs // {
          cargoArtifacts = hostCargoArtifacts;
          cargoExtraArgs = "-p spec-parity";
          doCheck = false;
        });
        hostClippy = hostCraneLib.cargoClippy (hostArgs // {
          cargoArtifacts = hostCargoArtifacts;
          cargoClippyExtraArgs = "-p panel-kit-tui -p spec-parity --features panel-kit-tui/spec-plan --all-targets -- -D warnings";
        });
        webExampleClippy = wasmCraneLib.cargoClippy (commonArgs // (workspaceSpecEnv workspace-canary) // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "-p panel-kit --features web-runtime --example workspace --target wasm32-unknown-unknown -- -D warnings";
        });
        clippy = pkgs.runCommand "panel-kit-clippy" { } ''
          mkdir -p "$out"
          ln -s ${hostClippy} "$out/host"
          ln -s ${webExampleClippy} "$out/web-example"
        '';
        workspaceSpecEnv = spec: {
          PANEL_KIT_WORKSPACE_SPEC = "${spec}";
        };
        workspaceSpecBuildEnv = workspaceSpecEnv workspace-canary;
        workspaceSpecAsciiBuildEnv = workspaceSpecEnv workspace-canary-ascii;
        workspace-spec-web-canary-wasm = wasmCraneLib.buildPackage (commonArgs // (workspaceSpecEnv workspace-canary) // {
          inherit cargoArtifacts;
          cargoExtraArgs = "--example workspace";
        });
        workspace-spec-web-remainder-wasm = wasmCraneLib.buildPackage (commonArgs // (workspaceSpecEnv web-workspace) // {
          inherit cargoArtifacts;
          cargoExtraArgs = "--example workspace";
        });
        workspace-spec-web-one-panel-wasm = wasmCraneLib.buildPackage (commonArgs // (workspaceSpecEnv workspace-one-panel) // {
          inherit cargoArtifacts;
          cargoExtraArgs = "--example workspace";
        });
        workspace-spec-web-wasm = pkgs.runCommand "workspace-spec-web-wasm" { } ''
          mkdir -p "$out"
          ln -s ${workspace-spec-web-canary-wasm} "$out/canary"
          ln -s ${workspace-spec-web-remainder-wasm} "$out/web-workspace"
          ln -s ${workspace-spec-web-one-panel-wasm} "$out/one-panel"
        '';
        workspace-spec-browser-tui-wasm = wasmCraneLib.buildPackage (commonArgs // workspaceSpecAsciiBuildEnv // {
          inherit cargoArtifacts;
          cargoExtraArgs = "-p panel-kit-tui --features spec-plan --example browser_tui";
        });
        workspace-spec-tui-native = hostCraneLib.buildPackage (hostArgs // workspaceSpecBuildEnv // {
          cargoArtifacts = hostCargoArtifacts;
          cargoExtraArgs = "-p panel-kit-tui --features spec-plan --example workspace";
          doCheck = false;
        });
      in {
        packages.default = panel-kit;
        packages.panel-kit = panel-kit;
        packages.workspace-canary = workspace-canary;
        packages.workspace-spec-reference = workspace-spec-reference;
        packages.workspace-canary-ascii = workspace-canary-ascii;
        packages.web-workspace = web-workspace;
        packages.workspace-one-panel = workspace-one-panel;
        packages.workspace-spec-web-wasm = workspace-spec-web-wasm;
        packages.workspace-spec-browser-tui-wasm = workspace-spec-browser-tui-wasm;
        packages.workspace-spec-tui-native = workspace-spec-tui-native;

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
          runtimeInputs = [ pkgs.cargo-edit rustHost ];
          text = ''
            if [ $# -ne 1 ]; then
              echo "usage: update-version <semver>" >&2
              exit 1
            fi
            cargo set-version --workspace "$1"
          '';
        };

        # mkLayout is retained for layout-only downstream flakes; mkWorkspaceSpec
        # is the full authored workspace-spec producer used by the canary.
        lib = {
          inherit (mkLayoutLib)
            mkLayout winStates modes unitKinds schemaVersion tileWMax tileHMax;
          inherit (mkWorkspaceSpecLib) mkWorkspaceSpec defaultTheme;
          inherit workspaceSpecSchema;
        };

        checks = {
          inherit panel-kit;
          clippy = clippy;
          core-unit-tests = hostCraneLib.cargoTest (hostArgs // {
            cargoArtifacts = hostCargoArtifacts;
            cargoTestExtraArgs = "-p panel-kit-core --features spec-json,spec-schema";
          });
          spec-parity = pkgs.runCommand "panel-kit-spec-parity" {
            nativeBuildInputs = [ specParityTool ];
            SPEC_PARITY_SCHEMA_PATH = "${./nix/schema/workspace-spec.schema.json}";
            SPEC_PARITY_WORKSPACE_CANARY_JSON = "${workspace-canary}";
            SPEC_PARITY_WORKSPACE_REFERENCE_JSON = "${workspace-spec-reference}";
            SPEC_PARITY_WEB_WORKSPACE_JSON = "${web-workspace}";
            SPEC_PARITY_WEB_WORKSPACE_PROVIDER_MANIFEST_JSON = "${web-workspace-provider-manifest}";
            SPEC_PARITY_FIXED_LAYOUT_JSON = "${./tools/spec-parity/fixtures/fixed-nine-layout.json}";
          } ''
            spec-parity check
            touch "$out"
          '';
          theme-parity = hostCraneLib.cargoTest (hostArgs // {
            cargoArtifacts = hostCargoArtifacts;
            cargoTestExtraArgs = "-p panel-kit --features web-runtime theme_parity";
          });
          workspace-spec-web-wasm = workspace-spec-web-wasm;
          workspace-spec-browser-tui-wasm = workspace-spec-browser-tui-wasm;
          workspace-spec-tui-native = pkgs.runCommand "panel-kit-workspace-spec-tui-native-check" { } ''
            ${workspace-spec-tui-native}/bin/workspace --check-offscreen
            touch "$out"
          '';
          # Docs must build clean (missing_docs is warn-level in lib.rs;
          # -D warnings promotes it + broken intra-doc links to errors).
          doc = wasmCraneLib.cargoDoc (commonArgs // {
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
      # re-exports the current check set per buildable system. Job names come
      # out as `<system>.<check>`.
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
          # matcher would trigger a release once for each check on each system.
          # An aggregate is one build that succeeds only if
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
