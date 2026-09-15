# Step 18: Live spec-parity comparator and the captured initial red

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 6
**Model:** opus
**Agent:** developer
**Depends on:** `17-nix-producer`
**Parallel with:** None
**Note:** None

**Goal:** Build the full private parity checker (design §11.3–11.4, slice 13, D-9): `tools/spec-parity/src/main.rs` stages — (1) schema compare (exported vs committed, from step 17), (2) strict decode of `nix/specs/*.nix` JSON output, (3) bidirectional EXACT provider-manifest comparison (Rust provider list ↔ Nix spec panel set, both directions, with set summaries), (4) `validate`/`resolve` over decoded specs, (5) JSON-pointer diffs ordered and human-readable (`/panels/7/id: nix=<missing>, rust="Distribution"` — design §11.4 output format), (6) the `mkLayout` round-trip stage (mkLayout output decodes as `SavedLayoutV2`, HR-19). The checker depends on `panel-kit` with `default-features = false, features = ["spec-plan"]` — BUT `spec-plan` lands in step 19; this step compiles the checker against core-only lowering (spec decode/validate/resolve) and defers plan lowering to step 19's walker stage (add the stage then).

CRITICAL — the captured initial red (HR-14): BEFORE any canary migration (step 20), run the comparator against the CURRENT seven-panel Nix canary (`nix/examples/workspace-canary.nix` JSON via mkLayout) vs the Rust nine-panel provider list (`crates/panel-kit-tui/examples/workspace_canary.rs:52-290` manifest) and COMMIT the exact red output as the negative-comparator fixture. Note the drift is richer than count: Nix `Notes` sits at y=27 where Rust has `Flame` (workspace_canary.rs:36-50 vs workspace-canary.nix:24-32). The PERMANENT negative comparator test `spec_parity_rejects_live_seven_vs_nine_canary` replays that fixture pair and asserts the mismatch class + exact pointers (Flame, Distribution) — no editable expected count anywhere (CK-33).

#### Expected Output

- `tools/spec-parity/src/main.rs` — staged comparator (schema, decode, provider compare, resolve, pointer diffs, mkLayout round-trip)
- Negative comparator test + committed initial-red fixture (checker's own test module)
- `Cargo.lock` updated for the tools member

#### Success Criteria

- [X] `spec_parity_rejects_live_seven_vs_nine_canary` passes (slice 13 first-failing test): asserts mismatch class + ordered pointers (Flame, Distribution) + set summaries, with no hand-pinned count an editor could flip
- [X] The initial red was CAPTURED from the live tree before any migration and committed as a fixture (the run log/fixture shows `/panels/length: nix=7, rust=9`-style output)
- [X] `mkLayout` round-trip stage green: mkLayout JSON decodes as `SavedLayoutV2` and round-trips
- [X] Checker exits non-zero on every drift class it knows so far (schema mismatch, decode failure, provider mismatch) with ordered pointer diagnostics
- [X] `cargo test -p spec-parity` green; `cargo run -p spec-parity` is RED against the current tree and GREEN against a hand-fixed nine-panel spec in a test fixture

#### Subtasks

- [X] Implement provider-manifest extraction (parse the canary manifest's panel set — or add a tiny library API to enumerate it, avoiding source-grepping, CK-35)
- [X] Implement the staged comparator with ordered JSON-pointer diff output per design §11.4
- [X] Capture the initial red against the live seven-vs-nine tree; commit the fixture
- [X] Write the permanent negative comparator test over the fixture pair; add the mkLayout round-trip stage
- [X] Run `cargo test -p spec-parity` and one live manual run; update Cargo.lock

#### Files Changed

- `tools/spec-parity/src/main.rs` — command dispatcher for schema, strict decode, panel comparison, retained layout decode, and private drift-check commands.
- `tools/spec-parity/src/{schema,panel_compare,layout_roundtrip,field_coverage,repository,diagnostics}.rs` — focused private stages for schema canonicalization, live Rust canary provider extraction, ordered provider diffs, `mkLayout` round-trip, field-drift diagnostics, and default repository check orchestration.
- `tools/spec-parity/src/layout_roundtrip.rs` — fixed HR-19 proof to decode `mkLayout` JSON as `SavedLayoutV2<String>`, encode it, decode it again, and compare the complete JSON object with JSON-pointer diagnostics so lost/changed layout fields cannot pass behind version/kind substring checks.
- `tools/spec-parity/fixtures/{live-seven-layout,fixed-nine-layout,rust-nine-provider-manifest,workspace-spec-reference,workspace-spec-reference-provider-manifest,workspace-spec-nix-only-field}.json` and `tools/spec-parity/fixtures/initial-red.txt` — captured inputs and expected initial-red output.
- `tools/spec-parity/Cargo.toml`, `Cargo.lock` — added the TUI canary/provider module dependencies used by the private checker.

#### Implementation Notes

- The live Rust provider list is extracted by compiling the existing TUI canary data module and calling `defaults()`; the checker does not grep Rust source text.
- Step 19 plan lowering remains deferred: this step adds only the private field-drift mechanisms and fixtures needed for Rust-only, Nix-only, and backend-ignored red behavior, without adding a public plan trait, capability lattice, or semantic-manifest API.
- The default `cargo run -p spec-parity` path runs the schema check, evaluates the current Nix `mkLayout` canary, proves the retained layout JSON decodes/round-trips as `SavedLayoutV2<String>`, extracts the live Rust provider IDs, and reports the current seven-vs-nine drift.

#### Verification Evidence

- Red proof before implementation: `cargo test -p spec-parity spec_parity_rejects_live_seven_vs_nine_canary` failed with unresolved comparator functions.
- `cargo test -p spec-parity` → 10 passed, 0 failed.
- HR-19 review fix red proof: `cargo test -p spec-parity layout_roundtrip` first failed because the new complete-layout drift assertions had no implementation (`compare_complete_layout_json` missing).
- `cargo clippy -p spec-parity --all-targets -- -D warnings` → passed.
- HR-19 focused proof: `cargo test -p spec-parity layout_roundtrip` → 3 passed, 0 failed; added assertions that lost panel geometry (`/panels/0/w`) and changed viewport (`/viewport/0`) fail the complete round-trip comparison.
- `cargo run -p spec-parity -- check-schema nix/schema/workspace-spec.schema.json` → passed.
- HR-19 repository proof: `cargo test -p spec-parity` → 14 passed, 0 failed.
- `cargo run -p spec-parity -- decode tools/spec-parity/fixtures/workspace-spec-reference.json` → passed.
- HR-19 default-check proof: `cargo run -p spec-parity -- check` → passed; the checker now propagates a no-output round-trip proof result instead of discarding serialized text. Re-ran after the parallel Step 19 warning fix with no warnings emitted.
- `cargo run -p spec-parity -- check-spec tools/spec-parity/fixtures/workspace-spec-reference.json tools/spec-parity/fixtures/workspace-spec-reference-provider-manifest.json` → passed strict decode, validate, and resolve.
- `cargo run -p spec-parity -- decode-layout tools/spec-parity/fixtures/live-seven-layout.json` → passed.
- `cargo run -p spec-parity` → exited 1 against the live current tree with `/panels/2/id: nix="Notes", rust="Flame"`, `/panels/7/id: nix=<missing>, rust="Distribution"`, `/panels/length: nix=7, rust=9`, and `missing from Nix: Flame, Distribution`.
- `cargo run -p spec-parity -- compare-panels tools/spec-parity/fixtures/fixed-nine-layout.json tools/spec-parity/fixtures/rust-nine-provider-manifest.json` → passed.
- `cargo run -p spec-parity -- decode tools/spec-parity/fixtures/workspace-spec-nix-only-field.json` → exited 1 with `/chrome/new_field: unknown field; allowed fields: ...`.
- `cargo run -p spec-parity -- check-required-leaves '{"chrome":{"dock":true}}' /chrome/new_field` → exited 1 with `/chrome/new_field: required by Rust schema, missing from Nix output`.
- `cargo run -p spec-parity -- check-backend-fields workspace-canary web-wasm /chrome/new_field /chrome/dock` → exited 1 with `workspace-canary × web-wasm: backend did not lower /chrome/new_field`.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Blocker | Canary migrated before red capture loses the historical proof | High | Low | HARD ordering: this step precedes step 20; the fixture must be committed with this step |
| Risk | Provider extraction greps Rust source text (CK-35 violation) | Med | Med | Expose the manifest as data (const/library fn) and consume it programmatically |
| Risk | Pointer formatting drifts from §11.4 examples | Med | Low | The negative-comparator fixture pins the exact format byte-for-byte |
