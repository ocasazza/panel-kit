# Step 25: Migrate remaining web examples to host-owned loops

**Task File:** `.specs/tasks/in-progress/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 5
**Model:** sonnet
**Agent:** developer
**Depends on:** `06-theme-generation`, `09-persistence-core`, `10-save-policy`, `12-web-parts`
**Parallel with:** `16-workspace-spec`, `26-views-cutover`
**Note:** None

**Goal:** Rewrite the four remaining controller-using web examples as host-owned compositions (design §13.1 pattern, §Included "consumer migrations"): `examples/theming.rs` (controller at :138), `examples/loading.rs` (:70), `examples/loading_workspace.rs` (:53), `examples/editor.rs` (:143) — each replacing `use_workspace(...)` + `ws.render(body)` + `ws.dock()` with `Snapshot::from_defaults` + `LocalStorageLayoutStore` + explicit `SavePolicy` + a host render loop over the borrowed frame composing `panel_surface`/`panel_chrome`/app header/body/`traffic_lights`/optional grip + separate `dock` (design §13.2 "After" shape).

Preserve each example's storage key, `Panel` enum, and default layout (D-13). `examples/editor.rs` continues to exercise the RETAINED web-only `src/editor.rs` Monaco widget (this step is its proof of continued function — it must keep compiling and working unchanged). The web controller (`src/lib.rs`) still exists and stays untouched — the examples simply stop using it (deletion is step 27). The loading examples must keep honoring the AGENTS.md loading/hydration convention (chrome first, stores, `LoadingGate`, `GlobalLoadingBar`, `LoadingWorkspace`) — that subsystem is untouched by design.

#### Expected Output

- `examples/theming.rs`, `examples/loading.rs`, `examples/loading_workspace.rs`, `examples/editor.rs` — host-owned rewrites
- Each example still building for wasm32 (`cargo build -p panel-kit --example <name> --target wasm32-unknown-unknown` or the repo's dx profile)

#### Success Criteria

- [X] No example references `use_workspace`, `Workspace<`, `.render(`, or `.dock()` controller methods
- [X] Each example's storage key and Panel serde IDs unchanged (visible in the source + asserted by a comment-documented key constant)
- [X] Each example applies an explicit `SavePolicy` after reductions (Manual or OnSettle — matching its current persistence behavior)
- [X] All four examples build for wasm32 and the host-runnable test subset stays green
- [X] `examples/editor.rs` still uses `src/editor.rs` (retained widget) with no changes to the widget itself

#### Subtasks

- [X] Rewrite `examples/theming.rs` to Snapshot + parts loop (theme source = step 06 emitter)
- [X] Rewrite `examples/loading.rs` and `examples/loading_workspace.rs` preserving the loading convention and store behavior
- [X] Rewrite `examples/editor.rs` around the retained Monaco editor widget
- [X] Build each for wasm32; keep the focused host-runnable subset green

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Loading-example rewrite accidentally alters the loading/hydration convention surface | Med | Low | Convention is out of scope (§16.8-ish, AGENTS.md): keep `loading_store`/`LoadingGate`/`GlobalLoadingBar` usage as-is; only the workspace shell changes |
| Risk | Persistence behavior drift (key or policy) loses user layouts | High | Low | Key constants copied verbatim; OnSettle policy matches current effect timing (settled writes only) |

#### Implementation Notes

- Changed `examples/theming.rs`, `examples/loading.rs`, `examples/loading_workspace.rs`, and `examples/editor.rs` to host-owned snapshot/projection/render loops over composable panel parts plus separate dock rendering.
- Added shared example-only support in `examples/support/composable_workspace.rs` and `examples/support/workspace_events.rs` for repeated host-owned persistence/projection/reducer plumbing while keeping each example's render loop explicit.
- Added `panel_chrome_with_events` in `src/widgets/panel.rs` so examples can compose chrome directly without reintroducing controller-owned input handling.
- Updated `tests/web_parts.rs` to exercise the event-capable composable chrome path.
- Verification evidence:
  - `grep` for `use_workspace|Workspace<|\.render\(|\.dock\(` over the four target examples and support directory: no matches.
  - `cargo test --test web_parts web_panel_preserves_focus_aria_and_overflow`: passed (1/1 selected test, 7 filtered).
  - `nix develop /Users/casazza/Repositories/ocasazza/panel-kit --command cargo build --manifest-path /tmp/panel-kit-example-check/Cargo.toml --target wasm32-unknown-unknown --bin theming --bin loading --bin loading_workspace --bin editor`: passed, verifying the four migrated example sources as wasm32 binaries without the root package's unrelated `panel-kit-tui` dev-dependency lane.
  - Direct `cargo build -p panel-kit --target wasm32-unknown-unknown --example ...` currently reaches the root package dev-dependency lane and fails in `crates/panel-kit-tui/src/input.rs` (`KeyCode::BackTab`/`KeyEvent::meta` API mismatch), before this step's example code is the limiting factor.
