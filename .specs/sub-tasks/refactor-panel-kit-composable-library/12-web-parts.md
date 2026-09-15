# Step 12: Web composition parts, input, and surface adapters

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 4
**Model:** sonnet
**Agent:** developer
**Depends on:** `11-partial-projectors`
**Parallel with:** `13-tui-parts`
**Note:** None

**Goal:** Port the web backend's composition parts out of the controller monolith (design §6.1, slice 6 web half, D-4): `src/widgets/root.rs` (`root_class`), `src/widgets/panel.rs` (`panel_surface`, `panel_chrome`, `traffic_lights`, `resize_grip` — markup ported from src/lib.rs:1120-1341), `src/widgets/dock.rs` (dock ported from src/lib.rs:1362-1404); plus `src/input.rs` (DOM key/pointer/wheel/focus → `WorkspaceEvent` translation, `panel_body_absorbs_wheel` from src/lib.rs:411-445, pointer capture/release, `core_pointer_event`) and `src/surface.rs` (`observe_viewport` adapter — ResizeObserver logic from src/lib.rs:595-655 — plus `viewport_size`/`browser_capabilities`/`surface_profile` from src/lib.rs:294-329).

Native fidelity is the bar (HR-7): semantic DOM, focus handling, native overflow, ARIA attributes, and wheel disposition must match today's controller rendering exactly. `panel_surface` never calls other parts — chrome is opt-in per call. CSS Grid consumes `TileGridProjection` explicit track/placement values. `ProjectedFrame` is render-scoped: borrowed in the render body via a non-reactive `RefCell` hook pattern (design §6.1) and dropped before any event mutation — NEVER stored in a signal/memo/prop/closure (CK-36). Wire `pub mod input; pub mod surface; pub mod widgets;` into `src/lib.rs` (the controller still exists and stays untouched — it dies in step 27; examples adopt parts from step 22/25 onward).

#### Expected Output

- `src/widgets/root.rs`, `src/widgets/panel.rs`, `src/widgets/dock.rs`
- `src/input.rs`, `src/surface.rs`
- `src/lib.rs` — module declarations only (no controller edits)
- SSR component tests (dioxus-ssr is already a dev-dependency)

#### Success Criteria

- [X] `web_panel_preserves_focus_aria_and_overflow` passes: SSR markup of `panel_shell`+`panel_chrome_with_controls`+`panel_body`+`resize_grip` carries the same roles/focus/overflow semantics and wheel disposition as the controller's rendering of the same state
- [X] Parts render a standalone single panel with no dock/title row when composed alone (SSR test); `traffic_lights`/`resize_grip` callable independently
- [X] `web_and_tui_use_same_tile_grid_projection` (web half): CSS Grid conversion consumes only `TileGridProjection` values (asserted by the conversion function's signature + a fixed-vector CSS output test)
- [X] Input adapter translates DOM events to `WorkspaceEvent` with `panel_body_absorbs_wheel` preserved (unit test over translated events)
- [X] No `ProjectedFrame` retained in any signal/memo/prop (code-shape assertion by review; the compile-fail proof from step 08 guards the borrow)
- [X] `cargo test -p panel-kit` (host-runnable subset) and `cargo build --workspace` green

#### Subtasks

- [X] Write failing SSR tests: focus/ARIA/overflow preservation vs controller output, standalone-surface composition, dock-only replacement
- [X] Port `panel_surface`/`panel_chrome`/`traffic_lights`/`resize_grip` into `src/widgets/panel.rs` from src/lib.rs:1120-1341; `dock` into `src/widgets/dock.rs`; `root_class` into `src/widgets/root.rs`
- [X] Create `src/input.rs` (DOM→`WorkspaceEvent`, wheel absorption, pointer capture) and `src/surface.rs` (`observe_viewport`, capabilities, profile)
- [X] Wire module declarations into `src/lib.rs`; keep the controller compiling untouched
- [X] Add the tile-grid conversion test (explicit track/placement from `TileGridProjection`)

#### Review Iteration 1 Notes

- [X] Replaced substring-only native-fidelity proof with a controller-era semantic DOM golden comparison for the same projected state.
- [X] Reworked viewport observation to keep observer/listener handles in hook state, remove listeners/disconnect observers on unmount, and expose registration status to callers.
- [X] Removed Step 12 clippy warnings in web tests/input/surface code.

#### Review Iteration 3 Notes

- [X] Added a `panel_shell`/`panel_body` slot API so host-selected chrome, traffic lights, header actions, body, and resize grip render inside the positioned `.panel` container while `panel_surface` remains a standalone body-only convenience.
- [X] Updated the native-fidelity golden to require controller-era nesting (`section.panel > header.panel-head > .lights`, body, resize grip) and added a negative SSR proof that sibling-only composition no longer satisfies HR-7.
- [X] Split panel placement/class helpers and web test fixtures into focused modules while preserving the public `panel::panel_style` path.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Fidelity drift vs controller (ARIA/focus/overflow subtleties) | High | Med | Diff SSR output of parts-composition vs controller rendering for identical snapshots (golden comparison during this step, before the controller dies) |
| Risk | Frame lifetime violation in Dioxus reactivity | High | Med | Follow design §6.1 pattern exactly (non-reactive RefCell hook, drop guard before mutation); step-08 compile-fail proof + review |
| Risk | Parallel step 13 shares no files but both add module wiring to sibling crates | Low | Low | Different crates; only core types are shared read-only |
