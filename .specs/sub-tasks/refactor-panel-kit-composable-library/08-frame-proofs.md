# Step 08: Allocation and lifetime proofs

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 3
**Model:** sonnet
**Agent:** developer
**Depends on:** `07-frame-projection`
**Parallel with:** `10-save-policy`
**Note:** None

**Review iteration 1 note:** Frame internals and frame proofs were split into cohesive `frame/` submodules after Phase 3 review flagged the monolithic `frame.rs` maintainability size limit. The public `panel_kit_core::frame::*` surface and projection behavior remain unchanged.

**Goal:** Prove and enforce the allocation/lifetime contract (design §5.2, slice 5, HR-4): after reserve/warm-up, `project_into` allocates zero bytes, clones no keys/strings/content, and a `ProjectedFrame` cannot outlive a second mutable scratch borrow.

Add the allocator-counted test `project_into_allocates_zero_after_reserved_warmup` (serial, warmed, unchanged capacity — count allocations between two `project_into` calls after warm-up and assert zero). Add the compile-fail doctest proving the frame cannot survive a second `&mut ProjectionBuffer` borrow (Rust compiletest-style `compile_fail` doctest in `frame.rs`; a `tests/ui/` harness or the crate's doctest ````compile_fail` — use the doctest form the crate toolchain supports, with an explicit assertion that the error is E0499/E0502-class). Scrub any remaining owned vectors/clones from the frame path introduced in step 07 (the demonstrated waste this refactor deletes: web clone at src/lib.rs:1077-1083 and TUI scratch at tui lib.rs:402-452,628-650 are backend-side; here only the CORE path must be clean — backend paths convert in steps 12/13).

#### Expected Output

- Allocator-counted zero-alloc test in `crates/panel-kit-core/src/frame.rs`
- Compile-fail doctest (or `tests/` compile-fail harness) in/next to `frame.rs`
- Any allocation-scrub edits inside the core frame path

#### Success Criteria

- [X] `project_into_allocates_zero_after_reserved_warmup` passes: serial test, warm-up call, then a second call with unchanged panel count asserts zero allocations (global allocator counter)
- [X] No `clone()` of keys/strings/content on the projection path (audited by the test's allocation count + code review; content borrows only)
- [X] The compile-fail doctest FAILS compilation today (frame outliving second mutable borrow) and is wired so `cargo test` runs it
- [X] `cargo test -p panel-kit-core` green (including the new proofs)

#### Subtasks

- [X] Add a counting global allocator in the test module; write `project_into_allocates_zero_after_reserved_warmup` (serial, `#[test]` with capacity warm-up)
- [X] Add the compile-fail doctest: `let f = project_into(i, &mut buf); let f2 = project_into(i2, &mut buf); drop((f, f2));` must not compile
- [X] Profile/scrub step-07 code paths for stray allocations (format!, to_string, Vec::push past capacity on warm path) and fix
- [X] Run `cargo test -p panel-kit-core` with the proofs enabled

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Blocker | Step 07's buffer design leaks an allocation on the warm path | High | Med | Reserve-by-capacity design; if a stray allocation is inherent, refactor the buffer layout within this step (it owns the frame path) |
| Risk | Compile-fail doctest not executed by default cargo test config | Med | Low | Use `/// ```compile_fail` doctest (run by `cargo test --doc`); verify it runs by temporarily breaking it |
| Risk | Test flakiness from parallel allocator noise | Med | Low | Serial execution + warm-up call; assert on delta between two consecutive warmed calls |
