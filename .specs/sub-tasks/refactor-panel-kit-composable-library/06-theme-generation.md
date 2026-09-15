# Step 06: Generated theme consumers and theme parity

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 2
**Model:** sonnet
**Agent:** developer
**Depends on:** `02-theme-core`
**Parallel with:** `04-reducer-input`, `05-widget-models`
**Note:** None

**Goal:** Make core `ThemeTokens` the ONLY editable source with generated consumers (design §8.1, slice 9 backend half, D-6): extend `build.rs` (reuse the substitution engine, build.rs:54-93) to emit the web `:root` CSS variables (`assets/panel-kit.css:5-29` becomes generated), the DESIGN.md token region (`DESIGN.md:4-63` frontmatter values, structure/keys preserved — `.impeccable/design.json` tooling must keep working), and the TUI `ThemeTokens → ResolvedTuiTheme` conversion with NO default literals left in `crates/panel-kit-tui/src/theme.rs` (delete `Theme::DARK`/`Theme::PAPER` literals, tui theme.rs:48-83, §12.1.9). Add the `checks.theme-parity` comparison logic (flake wiring happens in step 21) and the new `src/theme.rs` CSS custom-properties emitter.

TUI typography/density become explicit dispositions (`Approximated("terminal host font")` etc.), never silent omission (HR-11) — the disposition model itself lands with the TUI plan in step 19; here the TUI conversion function marks what it converts and what it approximates via a small core-side enum if needed, or defers disposition reporting to step 19 — implement the conversion + focus_ring coverage now, dispositions in step 19. Keep CSS class names and var names stable (consumer CSS overrides depend on them). The web test `injected_stylesheet_declares_every_token` (src/lib.rs:1508) becomes a generated-emitter assertion; `assets/panel-kit.css` `:root` pinning test (src/lib.rs:1508-1552) becomes generated-vs-core comparison.

#### Expected Output

- `build.rs` — ThemeTokens → CSS vars + DESIGN.md region emitters
- `assets/panel-kit.css` — generated `:root` block (structural CSS unchanged)
- `src/theme.rs` (NEW) — `ThemeTokens` → CSS custom properties one-time init emitter
- `crates/panel-kit-tui/src/theme.rs` — conversion-only; literals deleted
- Theme-parity comparison logic (a `tools/`-independent test or checker stage usable by step 21's flake check)
- Tests updated in `src/lib.rs` boot-contract block

#### Success Criteria

- [X] `dark_theme_emits_every_web_and_tui_token` fully green: every token including `focus_ring` emitted to CSS vars and converted for TUI
- [X] Theme-parity comparison proves: core tokens == generated CSS `:root` vars == TUI conversion == generated DESIGN.md region, for BOTH dark and paper
- [X] No editable palette/typography/density literal remains in `assets/panel-kit.css`, TUI theme.rs, or DESIGN.md's value region (CK-34: generated regions, no hand edits)
- [X] Existing web boot-contract tests pass with the emitter-based assertions; DESIGN.md structure/keys unchanged (`.impeccable/design.json` still parses)
- [X] `cargo test -p panel-kit -p panel-kit-core -p panel-kit-tui` (host-compilable subset) green; workspace builds

#### Subtasks

- [X] Write failing theme-parity test comparing core `ThemeTokens::{dark,paper}` against the three consumer artifacts
- [X] Extend `build.rs` substitution engine to emit `assets/panel-kit.css` `:root` and the DESIGN.md token region; regenerate and commit
- [X] Create `src/theme.rs` emitter; rewire `src/lib.rs` boot/CSS tests to assert generation (update `injected_stylesheet_declares_every_token`, CSS pinning test at src/lib.rs:1508-1552)
- [X] Rewrite `crates/panel-kit-tui/src/theme.rs` as conversion-only (`ThemeTokens`/`ResolvedTuiTheme`), delete `DARK`/`PAPER` literal blocks, update TUI painters' theme access
- [X] Implement the parity comparison as a reusable check (core unit test + a small deterministic comparison consumable by step 21's `checks.theme-parity`)

#### Implementation Notes

- Completed via a shared core theme emitter used by `build.rs` and the web `src/theme.rs` wrapper, avoiding duplicated CSS/DESIGN formatting logic.
- Phase 2 review iteration 2 added explicit TUI typography/density dispositions (`Applied`/`Approximated(reason)`) and expanded theme parity tests to assert every `ThemeTokens` color, typography, and density field across CSS, DESIGN, and TUI.
- Phase 2 review iteration 1 removed the redundant generated-region borrow and expanded the crate-level theming docs to name the full generated `ThemeTokens` CSS override surface.
- Phase 2 review iteration 3 fixed the Nix source closure by including `DESIGN.md` in the crane fileset, so `build.rs` can read the generated token region during package, clippy, and doc builds.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Visual regression on paper theme / consumer CSS overrides | Med | Med | Keep var names and class names byte-stable; parity check compares values, not formatting |
| Risk | DESIGN.md regeneration breaks `.impeccable/design.json` tooling | Med | Med | Regenerate values only; preserve structure/keys; verify the tooling config still parses |
| Risk | TUI painters still reference deleted `Theme::DARK` paths | Med | Low | Conversion function keeps the same public shape painters consume; compile is the proof |
