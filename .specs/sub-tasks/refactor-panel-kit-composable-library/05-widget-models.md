# Step 05: Unified widget and badge models in core

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 2
**Model:** sonnet
**Agent:** developer
**Depends on:** `02-theme-core`
**Parallel with:** `04-reducer-input`, `06-theme-generation`
**Note:** None

**Goal:** Move every renderer-neutral widget/badge model into core (design §8.2, slice 10 core half, D-7): `crates/panel-kit-core/src/widgets/` with `mod.rs` (`ContentSpec` covering all 12 content kinds, `DataSource<T>`, borrowed `ContentView<'a>`, `ScrollPolicy`), `badge.rs` (unified color math), `charts.rs`, `table.rs`, `meter.rs`, `status.rs`, `scroll.rs`, `spinner.rs` — and extend `crates/panel-kit-core/src/badge.rs` to the complete `BadgeSpec` superset with the three web core-symbol tests moved in (§12.1.10).

Data to move (current-tree verified): TUI chart models `Series/GaugeItem/FlameSpan/BoxItem` + `five_num` (tui charts.rs:29-362), `meter::bar` (tui meter.rs:11-15), `scroll::wrap`/`max_offset` (tui scroll.rs:24-26,72-78), badge color math `hue_color` (tui badge.rs:104-125) + `perceived_brightness`/`tint_over` (src/badge.rs:25-39). `BadgeSpec` carries every field both surfaces used: kind, field, value, active, with_x, with_plus, small, override_color, accent_color, click_kind, emit_hover (web-only escape hatches stay explicitly web-painter-side). Move the three core-symbol tests from `src/badge.rs:272-308` into core (web keeps only render/action tests — those land in step 14). Runtime views borrow (`ContentView<'a>`) — no per-paint cloning of points/rows/strings (CK-38 discipline starts here). TUI painter files are NOT touched in this step (painter conversion is step 15); TUI data structs are re-exported from their old paths this step? No — clean cutover: painters migrate in step 15, so this step keeps TUI compiling by leaving the TUI-local structs in place and core models additive; step 15 deletes the TUI-local declarations (§12.1.8 "after their core equivalents land").

#### Expected Output

- `crates/panel-kit-core/src/widgets/{mod,badge,charts,table,meter,status,scroll,spinner}.rs`
- `crates/panel-kit-core/src/badge.rs` — `BadgeSpec` superset + moved tests
- `crates/panel-kit-core/src/lib.rs` — `pub mod widgets;`
- Unit tests inside each new module

#### Success Criteria

- [X] `badge_spec_covers_web_and_tui_fields` passes (slice 10 first-failing test): one model carries every field the current web props (src/badge.rs) and TUI `Badge` (tui badge.rs:13-102) carry
- [X] The three moved tests (`tag_hue` etc.) pass from core; `src/badge.rs` test block deleted in the same edit
- [X] Chart/table/meter/status/scroll models in core are pure data + math: no ratatui/Dioxus imports (design §3.2 rule 1); `five_num` output matches TUI's current values on fixed inputs
- [X] `ContentSpec` enumerates all 12 content kinds (§10.3/§10.4); repository specs must be able to reject `Unsupported` — the enum has no open escape
- [X] `cargo test -p panel-kit-core` and `cargo build --workspace` green; TUI widget painters import/re-export core semantic models and math instead of retaining local copies

#### Subtasks

- [X] Write failing tests: `badge_spec_covers_web_and_tui_fields`, `five_num` fixed-vector parity, `hue_color`/`perceived_brightness`/`tint_over` moved-math parity, ContentSpec completeness
- [X] Create `crates/panel-kit-core/src/widgets/mod.rs` with `ContentSpec`, `DataSource<T>`, `ContentView<'a>`, `ScrollPolicy`
- [X] Move chart/table/meter/status/scroll/spinner models and `five_num` into `crates/panel-kit-core/src/widgets/`; unify badge color math in `widgets/badge.rs`
- [X] Extend `crates/panel-kit-core/src/badge.rs` to `BadgeSpec`; move the three web core-symbol tests in and delete them from `src/badge.rs:272-308`
- [X] Add `pub mod widgets;` to core `lib.rs`; run `cargo test -p panel-kit-core && cargo build --workspace`

#### Review Iteration 1 Notes

- [X] Added `ContentView::is_empty` beside the public count API to satisfy the clippy `len_without_is_empty` contract.
- [X] Replaced TUI-local chart/scroll/meter/spinner semantic math with imports/re-exports from `panel_kit_core::widgets::*`; TUI modules now keep ratatui painting/adaptation only for these widgets.
- [X] Strengthened `ContentSpec` completeness proof by instantiating every variant and asserting the observable serialized `kind` matches `ContentSpec::kind()`, rather than pinning only a static kind-name list.

#### Review Iteration 2 Notes

- [X] Converted `panel-kit-tui::badge::Badge` into a type alias for core `BadgeSpec`; TUI now keeps only ratatui color/span/width paint helpers over the shared spec.
- [X] Removed TUI-local badge action routing from examples; badge clicks now use `BadgeSpec::primary_action()` from core.
- [X] Added focused TUI badge tests proving the painter consumes `BadgeSpec` directly and click actions come from core helpers; renamed/expanded the core action proof to `badge_actions_match_across_backends`.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | BadgeSpec misses a web-only or TUI-only field, forcing painter rework | Med | Med | Enumerate fields from both painter call sites before writing the struct; the coverage test lists them explicitly |
| Risk | Moving math subtly changes rounding/behavior | Med | Low | Fixed-input parity tests against the TUI originals before deleting anything |
| Risk | Borrowed `ContentView<'a>` fights later spec-owned data | Med | Low | Design §8.2 pins the shape: borrowed runtime views over owned spec data; keep `DataSource<T>` generic |
