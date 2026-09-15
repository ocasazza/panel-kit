# Step 24: Browser-TUI canary and geometry-mirror deletion

**Task File:** `.specs/tasks/in-progress/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 7
**Model:** sonnet
**Agent:** developer
**Depends on:** `13-tui-parts`, `19-plan-coverage`, `20-canary-spec`, `21-flake-lanes`, `23-tui-native-canary`
**Parallel with:** None
**Note:** None

**Goal:** Convert `crates/panel-kit-tui/examples/browser_tui.rs` to a ratzilla host-owned loop and complete §12.1.12 (design §15 slices 15+17, HR-17): delete `TuiWorkspace` usage; the `BrowserStore` (browser_tui.rs:62-99) becomes a consumer of `tui::store` (or is replaced by the spec-appropriate adapter); the loop mirrors the native canary (spec store-path → catalog/snapshot → input → reduce → project → draw). Add `checks.workspace-spec-browser-tui-wasm`: builds the example for wasm32 INCLUDING the ASCII override path (charset override via the spec's `glyphs` field lowering onto `Charset` — design "Adaptations Needed"), from the same Nix canary spec.

THEN, with BOTH TUI examples off the mirror (step 23 removed `workspace.rs`'s imports; this step removes `browser_tui.rs`'s import at line 56), delete the geometry mirror from `crates/panel-kit-tui/examples/workspace_canary.rs:7-50` (§12.1.12): the `Panel` enum, `title` match, and `defaults()` — keeping ONLY the provider manifest functions (node_rows, demo_badges, noise, Metrics, STAGES, capacity_items) as the parity provider source. This ordering is deliberate: the mirror's replacement (the Nix-owned spec) now drives both examples, so the deletion lands in the same slice as its last consumer's replacement.

#### Expected Output

- `crates/panel-kit-tui/examples/browser_tui.rs` — ratzilla host-owned loop, no controller
- `crates/panel-kit-tui/examples/workspace_canary.rs` — provider manifest ONLY (geometry mirror deleted)
- `flake.nix` — `checks.workspace-spec-browser-tui-wasm`

#### Success Criteria

- [X] `checks.workspace-spec-browser-tui-wasm` builds for wasm32 from the same Nix canary spec, exercising the ASCII/glyph override
- [X] No `TuiWorkspace` reference in `browser_tui.rs`; `BrowserStore` replaced by the store adapter or spec-driven persistence
- [X] `workspace_canary.rs` contains NO `Panel` enum, NO title match, NO `defaults()` — provider manifest functions intact and still consumed by the comparator (checker stays green)
- [ ] `cargo build --workspace` green (no example references the deleted items) — not run here per Phase 7 slice policy; project-wide validation is owned by the phase/main orchestrator
- [ ] All three backend canaries now build from the ONE Nix canary spec (HR-17 complete) — browser-TUI lane proven here; web/native sibling lanes are owned by Steps 22/23

#### Subtasks

- [X] Rewrite `crates/panel-kit-tui/examples/browser_tui.rs` (ratzilla loop, spec store-path, store adapter, glyphs override)
- [X] Add `checks.workspace-spec-browser-tui-wasm` to the flake (wasm lane, ASCII override variant)
- [X] Delete the geometry mirror from `workspace_canary.rs:7-50`; fix any remaining manifest consumers
- [ ] Re-run `checks.spec-parity` (provider manifest still matches the Nix spec) and `cargo build --workspace` — `checks.spec-parity` passed; `cargo build --workspace` deferred per focused-slice validation policy

#### Files Changed

| File | Action | Notes |
|------|--------|-------|
| `crates/panel-kit-tui/examples/browser_tui.rs` | Modified | Replaced controller usage with host-owned `WorkspaceSpec` decode/resolve, `Snapshot`, reducer, projection buffer, TUI hit buffer, independently callable draw parts, and browser localStorage adapter keyed from spec persistence. Phase7ReReview2 cleanup splits storage/body drawing/app sections into coherent example-private modules and removes the per-frame action-log `Vec<String>` clone by drawing from borrowed action slices. |
| `crates/panel-kit-tui/examples/workspace_canary.rs` | Modified | Deleted `Panel`, `PanelKind` title match, and `defaults()` geometry mirror; retained provider data functions and now owns the executable provider manifest declarations. |
| `tools/spec-parity/src/provider_manifest.rs` | Modified | Now reads the live provider manifest from `workspace_canary.rs` instead of carrying an editable Rust expected list. |
| `crates/panel-kit-tui/examples/workspace_canary.rs` | Modified | Phase8ReReview2 fix: provider declarations remain the always-compiled live source, while renderer-only demo/content data is gated behind `spec-plan` (and native-only ratatui content behind non-wasm) so `tools/spec-parity` can include the manifest without compiling ratatui example bodies. |
| `crates/panel-kit-tui/examples/browser_tui.rs` | Modified | Removed a stale wasm-only `Mode` import exposed while rechecking the browser-TUI canary. |
| `tools/spec-parity/Cargo.toml` | Modified | Adds a private `unexpected_cfgs` check-cfg allowance for the imported canary module's `spec-plan` feature gate without declaring an enableable local spec-parity feature. |
| `flake.nix` | Modified | Added an ASCII-derived workspace canary JSON from the authoritative `workspaceCanary.value`; coordinated follow-up wiring runs `spec-parity check` with store-path JSON env overrides instead of the obsolete Rust provider fixture. |
| `tools/spec-parity/fixtures/rust-nine-provider-manifest.json` | Deleted | Obsolete duplicated Rust expected-provider fixture; tests now use the live executable canary provider manifest. |
| `.specs/sub-tasks/refactor-panel-kit-composable-library/24-browser-tui-canary.md` | Modified | Recorded completion status and verification evidence. |

#### Verification Evidence

- `nix build .#checks.aarch64-darwin.workspace-spec-browser-tui-wasm` — passed after the host-owned browser-TUI rewrite and ASCII spec-store path wiring.
- `nix build .#checks.aarch64-darwin.spec-parity` — passed after updating the provider manifest consumer for the deleted geometry mirror.
- `cargo test -p spec-parity live_manifest_comes_from_workspace_canary_provider_source` — passed after proving `tools/spec-parity` reads provider declarations from the executable canary source.
- `cargo test -p spec-parity` — passed (17 tests) after removing the obsolete Rust provider-manifest fixture use from panel comparator tests.
- `cargo run -p spec-parity -- check` — passed, comparing the Nix workspace canary to the live Rust provider manifest.
- `nix build .#checks.aarch64-darwin.spec-parity` — passed with `spec-parity check` store-path env wiring and no `rust-nine-provider-manifest.json` reference.
- `cargo clippy -p panel-kit-tui --features spec-plan --examples -- -D warnings` — passed after wiring native/browser TUI examples to consume the live canary provider manifest so the declarations are not dead code under host clippy.
- `grep` evidence: no `TuiWorkspace`, `BrowserStore`, `defaults(`, `pub enum Panel`, `impl PanelKind`, or `Panel::` references remain in `crates/panel-kit-tui/examples/browser_tui.rs` or `crates/panel-kit-tui/examples/workspace_canary.rs`.
- `nix build .#checks.aarch64-darwin.workspace-spec-browser-tui-wasm` — passed after Phase7ReReview2 cleanup, proving the inline module split, borrowed action-log draw path, ASCII glyph override, and browser-TUI wasm canary lane remain green.
- `grep` evidence: no `actions.clone()` or `action.clone()` remains in `crates/panel-kit-tui/examples/browser_tui.rs`; the badge action log renders recent lines from borrowed `&[String]`.
- Phase8ReReview2 initial red reproduced: `nix build .#checks.aarch64-darwin.spec-parity --print-build-logs` failed because `tools/spec-parity` included `workspace_canary.rs` and tried to compile ratatui-only content plus `crate::workspace_canary` paths.
- `cargo check -p spec-parity` — passed after gating renderer-only canary content away from the spec-parity provider-manifest compile path.
- `cargo check -p panel-kit-tui --features spec-plan --example workspace --example browser_tui` — passed, proving the native/browser TUI examples still compile with the gated provider module.
- `cargo test -p spec-parity live_manifest_comes_from_workspace_canary_provider_source` — passed after the provider-manifest gate change.
- `nix build .#checks.aarch64-darwin.spec-parity --print-build-logs` — passed after the Phase8ReReview2 fix.
- `nix build .#checks.aarch64-darwin.clippy --print-build-logs` — passed after the Phase8ReReview2 fix.
- `nix build .#checks.aarch64-darwin.workspace-spec-browser-tui-wasm .#checks.aarch64-darwin.workspace-spec-tui-native --print-build-logs` — passed, proving both TUI canary build lanes still compile from the shared provider source.
- `nix flake check --no-build --print-build-logs` — passed (`all checks passed!`) for the local evaluatable subset.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Blocker | Must run AFTER step 23 (mirror has two consumers; deletion needs both off) | High | Low | Dependency declared on `23-tui-native-canary` |
| Risk | Deleting the mirror breaks the comparator's provider source | High | Low | Provider manifest functions are kept verbatim; `checks.spec-parity` re-run is an explicit subtask |
| Risk | Ratzilla/wasm specifics (event loop, storage) differ from the reference `BrowserStore` | Med | Med | Port the existing BrowserStore behavior into the adapter consumer; wasm build check is the proof |
