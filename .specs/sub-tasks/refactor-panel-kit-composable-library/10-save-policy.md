# Step 10: Save policy write-count semantics

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 3
**Model:** sonnet
**Agent:** developer
**Depends on:** `09-persistence-core`
**Parallel with:** `08-frame-proofs`
**Note:** None

**Goal:** Implement `SavePolicy { Manual, OnSettle, OnChange }` in `crates/panel-kit-core/src/persist.rs` with the EXACT design §7 write-count transition table (design §7, slice 8, HR-9, D-5): Manual 0/0/0, OnSettle 0/1/0, OnChange 1/1/0 across Continuous/Settled/Unchanged reduction kinds — proven with a fake store — with reset-clear NOT immediately re-saving.

The policy is a pure decision function over `Reduction` (from step 03/04): given a policy and a reduction (phase + changed), decide `Save`/`NoSave`/`Clear`. It replaces the implicit Dioxus effect (today's settle behavior lives in an effect at src/lib.rs — the "save whenever something changed" anti-pattern, rubric Persistence Lifecycle Exactness score_2). Wheel-as-Settled: each accepted workspace-wheel event is one write under OnSettle/OnChange; hosts choosing Manual coalesce themselves (design trade-off, D-1). Reset-clear: a `Reset`/clear reduction clears the store and does NOT re-save in the same breath.

#### Expected Output

- `SavePolicy` + decision function in `crates/panel-kit-core/src/persist.rs`
- Fake-store write-count test (all 9 cells of the 3×3 table + reset-clear case)

#### Success Criteria

- [X] `save_policy_fake_store_write_counts` passes (slice 8 first-failing test): 3 policies × 3 reduction kinds = exact 0/1 write counts, asserted via recorded `save_calls()`
- [X] Reset-clear: `store.clear()` observable, and no save occurs immediately after the clear
- [X] Unchanged reductions never write under ANY policy; OnChange writes exactly once per Continuous change
- [X] `cargo test -p panel-kit-core` green

#### Subtasks

- [X] Write the failing table-driven test `save_policy_fake_store_write_counts` in `crates/panel-kit-core/src/persist.rs` with an in-memory fake `LayoutStore` recording calls
- [X] Implement `SavePolicy` decision over `Reduction { changed, phase, .. }`
- [X] Add the reset-clear case and wheel-as-Settled case to the table test
- [X] Run `cargo test -p panel-kit-core`

#### Implementation Notes

- Implemented `SavePolicy::decide` as a pure decision over `Reduction`, returning `SaveDecision::{NoSave, Save}` for normal reductions.
- Implemented `SavePolicy::reset_decision` plus `apply_save_decision`, so reset returns `SaveDecision::Clear` and applies one clear with no immediate save.

#### Review Iteration 2 Notes

- [X] Moved the `save_policy_fake_store_write_counts` table and reset-clear proof into `crates/panel-kit-core/src/persist/tests/save_policy.rs`, preserving the 3×3 policy table and reset-clear no-resave assertions.


#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Table drifts from design §7 (e.g. OnSettle writing on Continuous) | High | Low | The test IS the table transcribed from design §7; reviewer checks cell-by-cell |
| Risk | Policy grows hidden timers/heuristics (design §16 forbidden) | Med | Low | Pure function over Reduction only; no clock, no debounce — review guard |
