# Step 22: Web spec canary example and second repository spec

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 7
**Model:** sonnet
**Agent:** developer
**Depends on:** `14-web-painters`, `19-plan-coverage`, `20-canary-spec`, `21-flake-lanes`
**Parallel with:** `23-tui-native-canary`
**Note:** None

**Goal:** Convert `examples/workspace.rs` into the web spec canary (design §15 slice 17, HR-17): a host-owned composition consuming the Nix-authored spec as a BUILD-TIME store path (`include_str!(env!("PANEL_KIT_WORKSPACE_SPEC"))` pattern per design data-flow) — decode + `validate`/`resolve` + `web::lower_spec` at startup, seed `Snapshot`/catalog from `ResolvedWorkspace`, and run the parts loop with the web painters rendering every content kind the spec exercises. Author the SECOND repository spec `nix/specs/web-workspace.nix` exercising the remaining content kinds the canary doesn't cover (per §10.4 matrix — both specs together cover all 12; repository specs reject `Unsupported`).

Add `checks.workspace-spec-web-wasm` to the flake: builds the example for wasm32 with the spec JSON injected as a store path via `pkgs.writeText` (host lane passes the env; wasm lane builds the artifact). The one-panel standalone smoke (HR-6 web half) rides here too: include a single-panel spec variant build (a second store-path input or a build-time feature) proving a lone surface builds without controller chrome.

#### Expected Output

- `examples/workspace.rs` — spec-driven host-owned rewrite (web canary)
- `nix/specs/web-workspace.nix` — second repository spec (remaining content kinds)
- `flake.nix` — `checks.workspace-spec-web-wasm` (+ one-panel variant build input)
- Checker green over BOTH repository specs

#### Success Criteria

- [X] `checks.workspace-spec-web-wasm` builds the example for wasm32 from the Nix canary spec (store-path JSON input; no impure env)
- [X] The example renders every content kind present in its spec via the web painters (charts/table/meter/status included — new web capability exercised)
- [X] `checks.spec-parity` green over both repository specs; together they exercise all 12 content kinds with zero `Unsupported`
- [X] One-panel standalone web build succeeds (HR-6 smoke half) — a lone `panel_surface` with no implicit chrome
- [X] No controller API usage anywhere in the example

#### Subtasks

- [X] Rewrite `examples/workspace.rs`: spec decode → resolve → `lower_spec` → catalog/snapshot seeding → parts loop with all widget painters
- [X] Author `nix/specs/web-workspace.nix` covering the remaining §10.4 content kinds; run the checker over it
- [X] Add `checks.workspace-spec-web-wasm` to the flake with the store-path spec input + the one-panel variant
- [X] Verify: `nix build` the check attr; checker green over both specs

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Store-path env not available in every build context → impure fallback temptation | Med | Med | The env var is required at build time (`env!` panics if absent); both check variants pass it explicitly |
| Risk | Second spec invents content kinds not yet painted on web | Med | Med | Author only from the §10.4 matrix; a missing painter surfaces as a checker disposition failure, not a silent gap |

#### Implementation Notes

- Files changed:
  - `examples/workspace.rs` — rewritten as the spec-driven web canary, using `include_str!(env!("PANEL_KIT_WORKSPACE_SPEC"))`, host-owned reducer/projection state, and direct web part/widget painters.
  - `examples/support/spec_workspace.rs` — added example-only host-owned spec workspace wiring for decode/resolve, restore/persist, viewport events, reducer events, and projection scratch.
  - `nix/specs/web-workspace.nix` — added the second repository spec for the remaining content kinds (`editor`, `meter`, `status`, `spinner`).
  - `tools/spec-parity/fixtures/web-workspace-provider-manifest.json` — added provider manifest for the web spec check.
  - `flake.nix` — updated `checks.workspace-spec-web-wasm` to build canary, web remainder, and one-panel variants from store-path JSON inputs; added web spec to `checks.spec-parity`.
- Verification evidence:
  - `nix eval --raw --file nix/specs/web-workspace.nix json` ✅
  - `nix eval .#packages.aarch64-darwin.web-workspace --no-write-lock-file --apply 'x: x.name'` ✅ (`"panel-kit-web-workspace.json"`)
  - `nix eval .#packages.aarch64-darwin.workspace-one-panel --no-write-lock-file --apply 'x: x.name'` ✅ (`"panel-kit-workspace-one-panel.json"`)
  - `nix build .#checks.aarch64-darwin.workspace-spec-web-wasm --no-write-lock-file` ✅
  - `nix build .#checks.aarch64-darwin.spec-parity --no-write-lock-file` ✅
- Phase 7 re-review 1 clippy fix:
  - `examples/support/spec_workspace.rs` removed the stale `ResizePolicy` import flagged by direct root web canary clippy.
  - `examples/workspace.rs` removed the unnecessary mutable `header_clicks` binding and replaced `EventHandler` clone calls with direct copies in the panel chrome, traffic lights, resize grip, and dock paths.
  - `flake.nix` now includes the root `panel-kit` web canary example in `checks.clippy` through a focused wasm clippy lane with `PANEL_KIT_WORKSPACE_SPEC` set to the canary store path.
  - Verification evidence: direct `PANEL_KIT_WORKSPACE_SPEC=/nix/store/i5sp9fxl67mb06qpaqib8d64xnxmxr79-panel-kit-workspace-canary.json nix develop --no-write-lock-file --command cargo clippy -p panel-kit --features web-runtime --example workspace --target wasm32-unknown-unknown -- -D warnings` succeeded.
  - Verification evidence: final `nix build .#checks.aarch64-darwin.clippy --no-write-lock-file --no-link --print-build-logs` succeeded and logs include `cargo clippy --release --locked -p panel-kit --features web-runtime --example workspace --target wasm32-unknown-unknown -- -D warnings`.

- Phase 7 re-review 1 spec-parity fix:
  - `flake.nix` now feeds the `web-workspace` JSON store path and a store-path copy of `tools/spec-parity/fixtures/web-workspace-provider-manifest.json` into `checks.spec-parity`.
  - `tools/spec-parity/src/repository.rs` now runs the `web-workspace` repository entry through strict `WorkspaceSpec` decode, provider-manifest comparison, and backend plan coverage, so the second repository spec is not only exercised by the wasm build.
  - TDD red: `cargo test -p spec-parity repository::tests` initially failed with unresolved imports for the new repository-matrix helper functions.
  - Verification evidence: `cargo test -p spec-parity repository::tests` succeeded with 4 repository tests.
  - Verification evidence: `cargo test -p spec-parity` succeeded with 19 tests.
  - Verification evidence: `cargo run -p spec-parity -- check` succeeded.
  - Verification evidence: `nix eval --json .#checks.aarch64-darwin.spec-parity.drvAttrs | jq '{SPEC_PARITY_WEB_WORKSPACE_JSON, SPEC_PARITY_WEB_WORKSPACE_PROVIDER_MANIFEST_JSON}'` showed both web inputs as `/nix/store/...` paths.
  - Verification evidence: `nix build .#checks.aarch64-darwin.spec-parity` succeeded after building the new `panel-kit-web-workspace-provider-manifest.json` derivation.

- Phase 7 re-review 2 cleanup:
  - `examples/workspace.rs` now stays as the executable canary entrypoint and host loop only (107 lines), with spec decode/project/reduce support and content/provider rendering moved out of the entrypoint.
  - `examples/support/spec_workspace.rs` keeps the spec-specific host-owned reducer/projection/persistence support in one focused support module; the entrypoint no longer mixes these concerns with widget rendering.
  - `examples/support/composable_workspace.rs` now owns the web-canary-specific content rendering and demo binding data used by the Step 22 executable entrypoint, preserving the existing widget painter coverage without changing the Nix check attr or executable path.
  - Verification evidence: `PANEL_KIT_WORKSPACE_SPEC=/nix/store/i5sp9fxl67mb06qpaqib8d64xnxmxr79-panel-kit-workspace-canary.json nix develop --no-write-lock-file --command cargo clippy -p panel-kit --features web-runtime --example workspace --target wasm32-unknown-unknown -- -D warnings` ✅
  - Verification evidence: `nix build .#checks.aarch64-darwin.workspace-spec-web-wasm --no-write-lock-file --no-link --print-build-logs` ✅
