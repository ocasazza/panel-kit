# Step 23: Native TUI canary with offscreen verification

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 7
**Model:** sonnet
**Agent:** developer
**Depends on:** `13-tui-parts`, `15-tui-painters`, `19-plan-coverage`, `20-canary-spec`, `21-flake-lanes`
**Parallel with:** `22-web-canary-example`
**Note:** Phase 7 review fix: viewport sync now propagates reducer/layout/persistence errors through draw/offscreen checks instead of discarding them.

**Goal:** Convert `crates/panel-kit-tui/examples/workspace.rs` into the native TUI spec canary (design §15 slice 17, HR-17): delete ALL `TuiWorkspace` usage (workspace.rs:30-31,108-173,310-311) and run a host-owned loop — spec decode/resolve from the same `PANEL_KIT_WORKSPACE_SPEC` store-path input, `Snapshot` + `reduce` via `tui::input`, `project_into` into a `ProjectionBuffer`, `draw_*` parts over the borrowed frame — plus a `--check-offscreen` mode: render a deterministic fixed-size frame via ratatui `TestBackend` and assert expected body/chrome regions (exit 0 on match, non-zero + diff on mismatch).

Because this example stops importing `defaults`/`Panel` from the shared `workspace_canary` module (workspace.rs:13,35), the geometry mirror becomes single-consumer — step 24 (the other consumer, `browser_tui.rs`) performs the mirror deletion. This step switches the example's panel identity to `SpecPanelId` + `PanelCatalog` from the resolved spec (Nix-owned runtime strings — D-2), while keeping provider data wired through the manifest functions. Add `checks.workspace-spec-tui-native` to the flake (host lane): build the example natively and RUN `--check-offscreen` as the check command (HR-17 native half).

#### Expected Output

- `crates/panel-kit-tui/examples/workspace.rs` — host-owned loop + `--check-offscreen`
- `flake.nix` — `checks.workspace-spec-tui-native` (build + offscreen run)
- Offscreen golden assertions (deterministic fixed-size frame)

#### Success Criteria

- [X] `checks.workspace-spec-tui-native` builds natively and `--check-offscreen` passes from the same Nix canary spec as the web canary (HR-17: same spec, three backends)
- [X] No `TuiWorkspace` reference remains in the example; input goes through `tui::input` → `reduce`
- [X] Offscreen mode is deterministic: fixed size, fixed seed/order, asserts body/chrome/dock regions (golden rects), non-zero exit on mismatch with a readable diff
- [X] Every content kind in the canary spec renders through the TUI painters
- [X] Focused native cargo build green; `nix build` of the check attr green (project-wide `cargo build --workspace` left to the main-agent final validation per concurrent Phase 7 work)

#### Subtasks

- [X] Rewrite `crates/panel-kit-tui/examples/workspace.rs`: spec store-path input → catalog/snapshot → event loop (`tui::input` → `reduce` → `SavePolicy`) → `project_into` → `draw_*` parts
- [X] Implement `--check-offscreen` (TestBackend, fixed frame, region assertions, deterministic providers)
- [X] Switch panel identity to `SpecPanelId`/`PanelCatalog`; keep provider manifest functions as the data source
- [X] Add `checks.workspace-spec-tui-native` to the flake (host lane, runs the offscreen check)
- [X] Run the offscreen mode natively and via the flake check

#### Files Changed

- `crates/panel-kit-tui/examples/workspace.rs` — rewrote the native canary as a host-owned workspace-spec example using `SpecPanelId`, `Snapshot`, `tui::input` → `reduce`, reusable `ProjectionBuffer`/`TuiHitBuffer`, and explicit `draw_*` TUI parts; added deterministic `--check-offscreen` golden region checks; Phase 7 review fix makes viewport sync return reducer/persistence errors through draw and offscreen check paths; Phase 7 ReReview2 cleanup now keeps per-frame body jobs as stable `SpecPanelId` + body rect pairs, records evidence by stable key, and ends the projection borrow before rendering panel bodies without cloning panel content/title strings.
- `crates/panel-kit-tui/examples/workspace_canary.rs` — moved native canary demo-state/content rendering into the existing example-private provider module so `workspace.rs` owns the host loop and projection/offscreen shell while provider-backed content rendering stays with the canary data source.
- `flake.nix` — changed `checks.workspace-spec-tui-native` from build-only to a native run check that executes the built example with `--check-offscreen`.

#### Verification Evidence

- Initial red: `cargo run -p panel-kit-tui --features spec-plan --example workspace -- --check-offscreen` failed because the old example ignored the flag and attempted terminal initialization (`failed to initialize terminal: Device not configured`).
- `nix build .#workspace-canary --print-out-paths` produced `/nix/store/i5sp9fxl67mb06qpaqib8d64xnxmxr79-panel-kit-workspace-canary.json`.
- `PANEL_KIT_WORKSPACE_SPEC=/nix/store/i5sp9fxl67mb06qpaqib8d64xnxmxr79-panel-kit-workspace-canary.json cargo build -p panel-kit-tui --features spec-plan --example workspace` succeeded.
- `PANEL_KIT_WORKSPACE_SPEC=/nix/store/i5sp9fxl67mb06qpaqib8d64xnxmxr79-panel-kit-workspace-canary.json cargo run -p panel-kit-tui --features spec-plan --example workspace -- --check-offscreen` succeeded.
- `nix build .#checks.aarch64-darwin.workspace-spec-tui-native --print-build-logs` succeeded, building the native example and running `${workspace-spec-tui-native}/bin/workspace --check-offscreen`.
- `nix-instantiate --parse flake.nix` succeeded after the check attr edit.
- Phase 7 review red: `PANEL_KIT_WORKSPACE_SPEC=/nix/store/i5sp9fxl67mb06qpaqib8d64xnxmxr79-panel-kit-workspace-canary.json cargo test -p panel-kit-tui --features spec-plan --example workspace viewport_sync_reports_persistence_errors` failed with `no method named expect_err found for unit type ()`, proving viewport sync could not report persistence failures.
- Phase 7 review green: same focused cargo test passed (`1 passed`) after `sync_viewport` and `draw` returned `Result<(), LayoutError>`.
- Phase 7 review focused canary: `PANEL_KIT_WORKSPACE_SPEC=/nix/store/i5sp9fxl67mb06qpaqib8d64xnxmxr79-panel-kit-workspace-canary.json cargo run -p panel-kit-tui --features spec-plan --example workspace -- --check-offscreen` succeeded.
- Phase 7 review Nix check: final `nix build .#checks.aarch64-darwin.workspace-spec-tui-native --print-out-paths` produced `/nix/store/8mafxg077v27hif2jl3nzfn4afva8s5q-panel-kit-workspace-spec-tui-native-check`.
- Phase 7 review clippy follow-up: `cargo clippy -p panel-kit-tui --features spec-plan --example workspace -- -D warnings` passed after replacing the explicit `drop(projected)` with a scope that ends the borrowed projection before content rendering.
- Phase 7 ReReview2 focused native TUI canary check: `nix build .#checks.aarch64-darwin.workspace-spec-tui-native --print-out-paths --print-build-logs` succeeded and produced `/nix/store/53cfipqhiz22089ba2k2rsmamj3bj4v6-panel-kit-workspace-spec-tui-native-check`.
- Phase 7 ReReview2 focused native example tests: `PANEL_KIT_WORKSPACE_SPEC=/nix/store/i5sp9fxl67mb06qpaqib8d64xnxmxr79-panel-kit-workspace-canary.json nix develop .# --command cargo test -p panel-kit-tui --features spec-plan --example workspace` passed (`2 passed`).
- Phase 7 ReReview2 focused clippy: `PANEL_KIT_WORKSPACE_SPEC=/nix/store/i5sp9fxl67mb06qpaqib8d64xnxmxr79-panel-kit-workspace-canary.json nix develop .# --command cargo clippy -p panel-kit-tui --features spec-plan --example workspace -- -D warnings` passed.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Offscreen golden drift (terminal width/rounding) makes the check flaky | Med | Med | Fixed TestBackend size, deterministic provider data, region-level (not glyph-level) assertions |
| Risk | Identity switch breaks stable-ID expectations in provider wiring | Med | Low | Catalog stable IDs = serde variant names (D-2/D-13); provider functions key off the same IDs |
