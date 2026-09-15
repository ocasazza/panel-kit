# Step 20: Authoritative nine-panel Nix canary and stale-mirror deletion

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 6
**Model:** sonnet
**Agent:** developer
**Depends on:** `18-spec-parity-checker`
**Parallel with:** `19-plan-coverage`
**Note:** None

**Goal:** Author the authoritative nine-panel canary spec and delete the stale seven-panel mirror (design §10/§15 slice 15, §12.1.11, D-9): `nix/specs/workspace-canary.nix` — a FULL `mkWorkspaceSpec` spec (not layout-only) authored from the Rust provider manifest (`crates/panel-kit-tui/examples/workspace_canary.rs:52-290`: node_rows, demo_badges, noise, Metrics, STAGES, capacity_items) covering all nine panels (Workspace, Badges, Activity, Capacity, Flame, Distribution, Nodes, Notes, Theme) with stable IDs exactly the serde variant names, exercising every applicable content kind, theme, chrome, input, and persistence row.

Delete in the SAME edit (§12.1.11, same-slice replacement): `nix/examples/workspace-canary.nix` (the stale seven-panel layout-only mirror) AND the flake's `checks.layout-canary-schema` jq check plus its key/count assertions and stale mirror comments (flake.nix:118-150 region) — deleting the file without removing its consumer breaks flake eval, so both happen atomically. The `nix build .#layout-canary` demo attr is replaced by the spec-emitting path (or removed with the check — follow what flake.nix:79-86 actually wires). The comparator must go GREEN against the new spec (`workspace_canary_provider_manifest_matches_nix_spec_exactly`, slice 15 proof). The Rust-side geometry mirror deletion (`workspace_canary.rs` Panel/titles/defaults) does NOT happen here — it lands in step 24 after both TUI examples stop importing it (verified: `workspace.rs:35` and `browser_tui.rs:56` import from the shared module).

#### Expected Output

- `nix/specs/workspace-canary.nix` — authoritative nine-panel full spec
- `nix/examples/workspace-canary.nix` — DELETED
- `flake.nix` — `layout-canary-schema` check + mirror references removed (minimal edit; the lane split is step 21)
- Checker green-run proof

#### Success Criteria

- [X] `workspace_canary_provider_manifest_matches_nix_spec_exactly` passes: comparator green — provider manifests match the Nix spec exactly, bidirectionally, no drift classes
- [X] `nix/examples/workspace-canary.nix` deleted; no flake reference remains; `nix flake check`-equivalent eval of remaining attrs still succeeds (pre-step-21 check set)
- [X] The new spec strictly decodes, validates, resolves, and lowers (checker stages all green)
- [X] All nine panels present with stable IDs = current serde variant names; content kinds exercised per the §10.4 matrix
- [X] The step-18 negative comparator still passes (it consumes the committed fixture, not the live tree — proving permanence)

#### Subtasks

- [X] Author `nix/specs/workspace-canary.nix` from the provider manifest (all §10.4 rows; explicit nulls)
- [X] Run the comparator red→green; fix authoring until every stage is green (no expected-value edits — CK-33)
- [X] Delete `nix/examples/workspace-canary.nix` and the `layout-canary-schema` check/jq assertions/mirror comments from `flake.nix` atomically
- [X] Verify `nix eval .#checks` (or `nix flake check` subset) still evaluates; verify negative comparator still green

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Blocker | Must run AFTER step 18's red capture | High | Low | Dependency declared; the fixture is already committed |
| Risk | Flakiness: content-kind coverage gaps surface as checker failures late | Med | Med | That is the machinery working — author against the §10.4 matrix row by row; no shortcuts (no expected-field lists) |
| Risk | Removing the flake check disturbs the step-21 lane split baseline | Low | Low | Keep the flake edit minimal (delete check + attr only); step 21 restructures lanes from that baseline |

#### Files Changed

- `nix/specs/workspace-canary.nix` — created authoritative nine-panel `mkWorkspaceSpec` canary with stable IDs `Workspace`, `Activity`, `Flame`, `Notes`, `Badges`, `Nodes`, `Capacity`, `Distribution`, `Theme`.
- `nix/examples/workspace-canary.nix` — deleted stale seven-panel layout-only mirror.
- `flake.nix` — removed `layout-canary-schema`, stale `layout-canary` package, jq key/count assertions, and old mirror comments; added `workspace-canary` spec JSON package and `mkWorkspaceSpec`/schema lib exports.
- `tools/spec-parity/src/provider_manifest.rs` — added live Rust canary provider-manifest comparison.
- `tools/spec-parity/src/panel_compare.rs` — kept the permanent seven-vs-nine negative comparator focused on panel-list drift.
- `tools/spec-parity/src/repository.rs` — made default `spec-parity check` evaluate the new Nix canary, compare it to the Rust provider manifest, decode/resolve/lower it, and retain the `mkLayout` round-trip proof via the fixed fixture.
- `tools/spec-parity/src/layout_roundtrip.rs` — strengthened the retained `mkLayout` proof used by the default repository check: the fixed-nine layout fixture must survive decode → encode → decode with the complete JSON object unchanged, not merely contain `version`/`kind` substrings.

#### Verification Evidence

- RED before migration: `cargo run -p spec-parity -- check` failed with the historical drift (`/panels/2/id: nix="Notes", rust="Flame"`, missing `Flame, Distribution`).
- `nix eval --raw --file nix/specs/workspace-canary.nix json` emitted normalized WorkspaceSpec JSON.
- `cargo run -p spec-parity -- check` passed.
- `cargo test -p spec-parity spec_parity_rejects_live_seven_vs_nine_canary` passed (`1 passed`).
- HR-19 retained-API proof after review fix: `cargo test -p spec-parity layout_roundtrip` → 3 passed, including negative cases for lost panel geometry and changed viewport; `cargo test -p spec-parity` → 14 passed.
- `nix-instantiate --parse flake.nix` parsed successfully.
- `cargo run -p spec-parity -- check` → passed with the strengthened round-trip proof on the retained fixed-nine `mkLayout` fixture; re-ran after the parallel Step 19 warning fix with no warnings emitted.
- Flake evaluation proof without mutating the Git index: copied the working tree without `.git` to `/private/tmp/panel-kit-flake-eval`, then `nix eval path:/private/tmp/panel-kit-flake-eval#packages.aarch64-darwin.workspace-canary.name` returned `"panel-kit-workspace-canary.json"` and `nix eval path:/private/tmp/panel-kit-flake-eval#checks.aarch64-darwin --apply builtins.attrNames` returned `[ "browser-tui-example" "clippy" "doc" "panel-kit" ]`.
