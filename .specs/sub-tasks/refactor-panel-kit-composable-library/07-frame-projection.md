# Step 07: Borrowed frame projection

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 3
**Model:** opus
**Agent:** developer
**Depends on:** `04-reducer-input`
**Parallel with:** `09-persistence-core`
**Note:** None

**Goal:** Implement single-sourced geometry and the borrowed projected frame (design §5, slices 4, D-3): `crates/panel-kit-core/src/frame.rs` with `FrameStatus` (incl. honest `TooSmall`), `Placement`, `PanelChromeProjection`, `TileLayoutMetrics`, `TileGridProjection`, `PanelProjection`, `DockProjection`, `ProjectionBuffer<K>` (caller-owned scratch), `ProjectedFrame<'a, K>` (borrows ONLY the scratch), `ProjectionInput`, `project_into`, and `hit_test` — reusing `effective_rect`/`front_z`/`floating_content_height`/`clamp_scroll` (core lib.rs:674-721), never recomputing them.

This is the task's most delicate new subsystem: the frame must carry chrome, deterministic paint order (floating: `(z, source_index)`; tiling: source order; maximized: only the deterministic frontmost; minimized: dock-only), placement/z/state/focus/drag, body/control hit regions, dock entries, and content extent (HR-3). Non-finite viewports rejected; too-small viewports yield `FrameStatus::TooSmall` with no negative/NaN rectangles. `visible_panels` semantics (maximized hides others) are preserved through projection. Design the buffer layout NOW so the warmed path can be zero-allocation (step 08 proves it): numeric records in reusable scratch, no owned keys/strings/content cloned into the frame. Also add the standalone core example (a `panel-kit-core` example that projects one panel and links no backend — the HR-2/HR-18 Cargo-only proof).

#### Expected Output

- `crates/panel-kit-core/src/frame.rs` — full frame machinery per design §5.1
- `crates/panel-kit-core/examples/standalone_panel.rs` (or crate-convention equivalent) — core-only projection example
- `crates/panel-kit-core/src/lib.rs` — `pub mod frame;`
- Unit tests in `frame.rs`

#### Success Criteria

- [X] `projected_frame_resolves_all_semantic_regions` passes (slice 4 first-failing test — write it first, watch it fail)
- [X] TooSmall at/below minimum viewport: no negative or NaN rectangles anywhere in the frame
- [X] Paint-order tests: floating `(z, source_index)`; tiling source order; maximized emits only deterministic frontmost; minimized appears only in dock (EP per mode)
- [X] `hit_test` resolves body vs control regions consistently with today's web/TUI hit behavior (fixed-vector tests)
- [X] Standalone core example builds with `cargo build -p panel-kit-core --example <name>` and links no backend (no dioxus/ratatui in its dependency closure)
- [X] `cargo test -p panel-kit-core` green

#### Subtasks

- [X] Write the failing test `projected_frame_resolves_all_semantic_regions` in `crates/panel-kit-core/src/frame.rs`
- [X] Implement `ProjectionBuffer`/`ProjectedFrame`/`project_into` over `ProjectionInput` (Snapshot + viewport + chrome metrics), reusing `effective_rect`/`front_z`/`floating_content_height`/`clamp_scroll`
- [X] Implement tile resolver (`TileGridProjection`/`TileLayoutMetrics`) feeding both future backends (HR-5 single source)
- [X] Implement `hit_test` over projected regions; TooSmall/non-finite handling
- [X] Add the standalone core example; add `pub mod frame;` to core `lib.rs`
- [X] Run `cargo test -p panel-kit-core && cargo build -p panel-kit-core --examples`

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Borrow design forces per-frame clones, defeating step 08's zero-alloc proof | High | Med | Frame exposes iterators/slices over scratch records; NO owned `Vec<K>`/`String` fields; design §5.2 contract is the acceptance bar |
| Risk | Paint-order/tile placement drifts from today's backend-computed geometry | High | Med | Fixed-vector tests transcribed from current web/TUI rendering of the same layouts (TUI canary nine-panel layout is a ready fixture) |
| Risk | Maximized/minimized semantics subtly wrong (visible_panels parity) | Med | Med | Reuse/align with core `visible_panels` (lib.rs:855-870) behavior; EP-per-mode tests |
