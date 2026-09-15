# Step 21: Dual crane lanes and the named checks

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 7
**Model:** sonnet
**Agent:** developer
**Depends on:** `06-theme-generation`, `10-save-policy`, `18-spec-parity-checker`, `19-plan-coverage`, `20-canary-spec`
**Parallel with:** None
**Note:** None

**Goal:** Split the flake into dual crane lanes and add the named checks (design §11.5, slice 16, D-10, HR-16): a HOST lane — separate `rustHost` toolchain (the existing `rustWasm` at flake.nix:42-44 installs ONLY wasm32; reusing it silently cross-compiles), `hostArgs` with `CARGO_BUILD_TARGET` removed via `removeAttrs`, `doCheck = true`, `hostSrc` fileset unioning `./tools` and the Nix spec inputs (store-path JSON via `pkgs.writeText`), running: `checks.core-unit-tests` (`cargo test -p panel-kit-core --features spec-json,spec-schema`), `checks.spec-parity` (runs the checker over repository specs), `checks.theme-parity` (step 06's comparison wired as a check); and the WASM lane unchanged. The three canary build checks (`workspace-spec-web-wasm`, `workspace-spec-browser-tui-wasm`, `workspace-spec-tui-native`) are added by steps 22–24 when their examples exist — this step lays the lane both groups build on.

Also update `hydra-project.json`'s jobset description to enumerate the new checks (optional per analysis), and ensure `hydraJobs` aggregation (flake.nix:191-214) picks up new check attrs automatically on both Hydra systems (`x86_64-linux`, `aarch64-darwin`). The matrix (`spec × backend`) runs inside ONE native process — no per-cell derivations (design §11.5).

#### Expected Output

- `flake.nix` — `rustHost` toolchain, `hostArgs`/`wasmArgs` split, `hostSrc` (+= `./tools`), checks `core-unit-tests` + `spec-parity` + `theme-parity`
- `hydra-project.json` — description updated
- `Cargo.lock` — settled for the tools member

#### Success Criteria

- [X] `checks.core-unit-tests` is the first check that actually RUNS Rust tests natively (slice 16 first-failing condition: today's lane has `doCheck=false` + `CARGO_BUILD_TARGET=wasm32`, flake.nix:61-62) — verify no `CARGO_BUILD_TARGET` in the host lane env and `doCheck=true`
- [X] `checks.spec-parity` runs the checker natively and is green; `checks.theme-parity` green
- [X] The wasm lane is untouched: `nix build .#panel-kit` still produces the wasm32 artifact
- [X] `nix flake check` passes locally for the evaluatable subset; Hydra aggregation attrs present for both systems
- [X] The spec store-path JSON input reaches the checker via `pkgs.writeText` (no impure paths)

#### Subtasks

- [X] Add `rustHost` toolchain (stable, host target only) and `hostArgs` = `commonArgs` minus `CARGO_BUILD_TARGET` (removeAttrs), `doCheck=true`, with `hostSrc` fileset unioning `./tools` + nix spec files
- [X] Add `checks.core-unit-tests` (host crane `cargoTest` over panel-kit-core with features), `checks.spec-parity`, `checks.theme-parity`
- [X] Wire the Nix spec JSON into the checker input via `pkgs.writeText`
- [X] Update `hydra-project.json` description; confirm hydraJobs aggregate consumes the new attrs
- [X] Run `nix flake check` (or the per-check builds) and `nix build .#panel-kit`


#### Files Changed

- `flake.nix` — added host crane lane, native checks, store-path spec JSON packages, and Phase 7 canary build package/check lanes.
- `hydra-project.json` — updated the jobset description to enumerate the new native checks and workspace-spec canary lanes.
- `crates/panel-kit-tui/src/input.rs` — adjusted ratzilla key translation to the actual wasm `KeyEvent` shape so the browser-TUI wasm package lane builds.
- `Cargo.lock` — already contained the `tools/spec-parity` workspace member and schemars entries; no lockfile edit was required.
- `tools/spec-parity/src/repository.rs` — added store-path/env override plumbing so `spec-parity check` can run the repository matrix inside a Nix derivation without falling back to checkout-relative Nix evaluation.

#### Verification Evidence

- Initial red: `nix eval .#checks.aarch64-darwin.core-unit-tests.drvPath --raw` failed because `checks.aarch64-darwin.core-unit-tests` did not exist.
- `nix-instantiate --parse flake.nix` succeeded.
- `nix build .#checks.aarch64-darwin.core-unit-tests --print-build-logs` succeeded: 84 core unit tests, 1 dependency-graph test, and 3 doctests passed natively.
- `nix build .#checks.aarch64-darwin.spec-parity --print-build-logs` succeeded.
- `nix build .#checks.aarch64-darwin.theme-parity --print-build-logs` succeeded: 3 theme parity tests passed.
- `nix build .#panel-kit --print-build-logs` succeeded on the existing wasm lane.
- `nix build .#packages.aarch64-darwin.workspace-spec-web-wasm --print-build-logs` succeeded and installed `target/wasm32-unknown-unknown/release/examples/workspace.wasm`.
- `nix build .#packages.aarch64-darwin.workspace-spec-browser-tui-wasm --print-build-logs` succeeded and installed `target/wasm32-unknown-unknown/release/examples/browser_tui.wasm`.
- `nix build .#packages.aarch64-darwin.workspace-spec-tui-native --print-build-logs` succeeded and installed the native `workspace` example binary.
- `nix build .#checks.aarch64-darwin.workspace-spec-web-wasm .#checks.aarch64-darwin.workspace-spec-browser-tui-wasm .#checks.aarch64-darwin.workspace-spec-tui-native --print-build-logs` succeeded from the finalized named check attrs.
- `nix flake check --no-build --print-build-logs` succeeded for the local evaluatable subset and evaluated Hydra attrs for both `x86_64-linux` and `aarch64-darwin`.
- `nix derivation show`/`jq` verified `checks.aarch64-darwin.core-unit-tests` has `CARGO_BUILD_TARGET` absent and `doCheck = 1`.
- `nix derivation show`/`jq` verified `packages.aarch64-darwin.workspace-spec-web-wasm` receives a `/nix/store/...-panel-kit-workspace-canary.json` `PANEL_KIT_WORKSPACE_SPEC` value; `workspace-spec-browser-tui-wasm` also receives `PANEL_KIT_TUI_CHARSET=ascii`.
- Phase 7 review fix: `checks.spec-parity` now invokes `spec-parity check` once with `SPEC_PARITY_SCHEMA_PATH`, `SPEC_PARITY_WORKSPACE_CANARY_JSON`, `SPEC_PARITY_WORKSPACE_REFERENCE_JSON`, and `SPEC_PARITY_FIXED_LAYOUT_JSON` pointing at store paths; this runs the repository matrix (`check_committed_schema`, strict decode, live provider comparison, plan coverage/backend dispositions, and layout round-trip) instead of the previous partial static command list.
- Phase 7 review fix: `hostSrc` now includes `./nix`, so native spec-parity/tool test targets can compile their `include_str!` schema fixture under crane.
- Phase 7 review fix: `checks.clippy` now uses the host crane lane with `cargo clippy --release --locked -p panel-kit-tui -p spec-parity --features panel-kit-tui/spec-plan --all-targets -- -D warnings`, covering the changed TUI examples and `tools/spec-parity` workspace member.
- TDD red: `cargo test -p spec-parity repository::tests` initially failed with `no json_text_from_override in repository` before the store-path override helper was implemented.
- `cargo test -p spec-parity repository::tests` succeeded: 2 env/store-path override tests passed, including a missing-path guard so a mistyped store path cannot be silently treated as inline JSON.
- `cargo test -p spec-parity` succeeded: 17 tests passed.
- `cargo run -p spec-parity -- check` succeeded against the repository-local full matrix.
- `nix derivation show .#checks.aarch64-darwin.spec-parity` verified the derivation exports the four `SPEC_PARITY_*` store-path overrides and its build command is `spec-parity check`.
- `nix build .#checks.aarch64-darwin.spec-parity --print-build-logs` succeeded with the full repository matrix.
- `nix derivation show .#checks.aarch64-darwin.clippy` verified the clippy build phase uses `cargoWithProfile clippy --locked -p panel-kit-tui -p spec-parity --features panel-kit-tui/spec-plan --all-targets -- -D warnings`.
- `nix build .#checks.aarch64-darwin.clippy --print-build-logs` succeeded after the host lane exposed and sibling fixes resolved panel-kit-tui example warnings.
- Phase 7 re-review 1 clippy fix:
  - `flake.nix` now aggregates `checks.clippy` from the existing host clippy lane plus a focused wasm web-example clippy lane for `-p panel-kit --features web-runtime --example workspace --target wasm32-unknown-unknown -- -D warnings`.
  - The web-example clippy lane receives `PANEL_KIT_WORKSPACE_SPEC` as the `workspace-canary` store path, so `examples/workspace.rs` is warning-denied under the same build-time spec input used by the web canary.
  - Verification evidence: direct `PANEL_KIT_WORKSPACE_SPEC=/nix/store/i5sp9fxl67mb06qpaqib8d64xnxmxr79-panel-kit-workspace-canary.json nix develop --no-write-lock-file --command cargo clippy -p panel-kit --features web-runtime --example workspace --target wasm32-unknown-unknown -- -D warnings` succeeded.
  - Verification evidence: final `nix build .#checks.aarch64-darwin.clippy --no-write-lock-file --no-link --print-build-logs` succeeded; logs show both the host `panel-kit-tui`/`spec-parity` clippy lane and the new root `panel-kit` web-example clippy lane.
  - Verification evidence: `nix derivation show /nix/store/drdycfgf6ij7ij8l86xsnj5r1rppzrqa-panel-kit-clippy-1.0.0.drv | jq -r '.derivations[] | .env | {buildPhase, CARGO_BUILD_TARGET, PANEL_KIT_WORKSPACE_SPEC}'` showed the final web clippy derivation runs the root example command with `CARGO_BUILD_TARGET = "wasm32-unknown-unknown"` and `PANEL_KIT_WORKSPACE_SPEC = "/nix/store/i5sp9fxl67mb06qpaqib8d64xnxmxr79-panel-kit-workspace-canary.json"`.

- Phase 7 re-review 1 spec-parity fix:
  - `flake.nix` now writes `tools/spec-parity/fixtures/web-workspace-provider-manifest.json` into a store-path provider-manifest input and passes both `SPEC_PARITY_WEB_WORKSPACE_JSON` and `SPEC_PARITY_WEB_WORKSPACE_PROVIDER_MANIFEST_JSON` to `checks.spec-parity`.
  - `tools/spec-parity/src/repository.rs` now includes `web-workspace` in `spec-parity check`'s permanent repository matrix: strict decode, exact provider-manifest comparison, and concrete backend plan coverage.
  - TDD red: `cargo test -p spec-parity repository::tests` initially failed with unresolved imports for `check_workspace_spec_against_provider_manifest` and `web_workspace_provider_manifest`.
  - Verification evidence: `cargo test -p spec-parity repository::tests` succeeded with 4 repository tests, including web manifest fixture decode and provider-drift rejection.
  - Verification evidence: `cargo test -p spec-parity` succeeded with 19 tests.
  - Verification evidence: `cargo run -p spec-parity -- check` succeeded against the repository-local matrix including `web-workspace`.
  - Verification evidence: `nix-instantiate --parse flake.nix` succeeded.
  - Verification evidence: `nix eval --json .#checks.aarch64-darwin.spec-parity.drvAttrs | jq '{SPEC_PARITY_WEB_WORKSPACE_JSON, SPEC_PARITY_WEB_WORKSPACE_PROVIDER_MANIFEST_JSON}'` showed both web inputs as `/nix/store/...` paths.
  - Verification evidence: `nix build .#checks.aarch64-darwin.spec-parity` succeeded; its plan built the `panel-kit-web-workspace-provider-manifest.json` derivation and `panel-kit-spec-parity`.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Host lane silently reuses the wasm-only toolchain → cross-compile failures | High | Med | Dedicated `rustHost`; assert in the derivation env that `CARGO_BUILD_TARGET` is absent; design §11.5 recipe followed verbatim |
| Risk | `./tools` missing from hostSrc hides the checker | High | Low | hostSrc fileset union explicitly includes `./tools`; the spec-parity check failing to find the crate would fail loudly |
| Risk | Hydra aggregation doubles check count / slows release gate | Med | Low | Mechanical aggregation (flake.nix:208-213); matrix runs in one native process |
