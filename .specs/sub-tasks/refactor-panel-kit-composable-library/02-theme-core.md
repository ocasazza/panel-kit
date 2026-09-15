# Step 02: One theme source in core

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 1
**Model:** sonnet
**Agent:** developer
**Depends on:** None
**Parallel with:** `01-panel-identity`
**Note:** None

**Goal:** Create `crates/panel-kit-core/src/theme.rs` as the single editable source for palette (dark/paper, including `focus_ring`), typography, and density, absorbing the values currently seeded in `crates/panel-kit-core/src/tokens.rs:32-231` (design §8.1, slice 9 core half, D-6).

Implement `Color` with the canonical lowercase `#rrggbb` codec (`try_from`/`into` String), `ColorRef`, `ThemeColor`, `ColorTokens`, `ThemeTokens { dark, paper }`, `TypographyTokens`, `DensityTokens` per design §8.1 struct definitions. `focus_ring` must exist in the token set (absent from TUI today — HR-11). Absorb `Token`/`DARK`/`MONO` seed values from `tokens.rs`; keep `by_name` (or re-export) so `build.rs` and existing consumers keep working. Do NOT touch CSS, TUI literals, or DESIGN.md here — those become generated consumers in step `06-theme-generation`. Serde derives: only behind the later spec features? No — theme types need serde for the spec later; give them plain `Serialize/Deserialize` (serde is already a core dependency for V1/V2 types) and leave `JsonSchema` derives to step 16's opt-in feature wiring (`#[cfg_attr(feature = "spec-schema", derive(JsonSchema))]`).

#### Expected Output

- `crates/panel-kit-core/src/theme.rs` — Color/ColorRef/ThemeColor/ColorTokens/ThemeTokens/TypographyTokens/DensityTokens
- `crates/panel-kit-core/src/tokens.rs` — absorbed/re-exporting (values moved, public API preserved)
- `crates/panel-kit-core/src/lib.rs` — `pub mod theme;` declaration only
- Unit tests in `theme.rs` (tokens tests move/follow from `tokens.rs`)

#### Success Criteria

- [X] `dark_theme_emits_every_web_and_tui_token` (core half) passes: `ThemeTokens::dark()` carries every token key the current web `:root` block and TUI `Theme::DARK` derive from, PLUS `focus_ring`
- [X] `Color` codec accepts and emits only canonical lowercase `#rrggbb` (rejects `#RRGGBB`, `fff`, `#ffff`, named colors)
- [X] `Theme::DARK` values in `crates/panel-kit-tui/src/theme.rs:48-83` are byte-equal derivable from `ThemeTokens::dark()` (assert in a test; the literal deletion itself happens in step 06)
- [X] `cargo test -p panel-kit-core` green; `build.rs` token generation still compiles and produces identical boot CSS
#### Subtasks

- [X] Write failing tests in `crates/panel-kit-core/src/theme.rs`: token completeness incl. `focus_ring`, `#rrggbb` codec strictness, dark/paper derivable-equality with today's literals (5 new tests observed red on `todo!()` stubs; the paper-literals equality half lives in `crates/panel-kit-tui/src/theme.rs::core_parity` — core cannot name the TUI types)
- [X] Implement theme types per design §8.1 in `crates/panel-kit-core/src/theme.rs` (JsonSchema derives deferred to step 16 per goal note)
- [X] Fold `tokens.rs:32-231` values into `theme.rs`; keep `tokens.rs` re-exporting (`by_name`, `Token`, `DARK`, `MONO` + the 15 seed consts) so `build.rs` and pinning tests stay source-compatible
- [X] Move/extend the existing tokens tests; add `pub mod theme;` to core `lib.rs` (declaration only — coordinate with parallel step 01)
- [X] Run `cargo test -p panel-kit-core` and `nix build .#panel-kit` locally if available, else `cargo build --workspace` — both run: workspace tests 57/57 green, wasm32 crane lane (`checks.aarch64-darwin.panel-kit`) green, boot CSS byte-identical (md5 unchanged)

Discoveries:
- Paper's inverse pair comes from the web theming example (`--inv-bg:#1a1a1a`/`--inv-fg:#f4f1ea`); TUI PAPER never carried one. `focus_ring` aliases `fg` in both presets, mirroring `--focus-ring:var(--fg)`.
- TUI PAPER and the web theming example differ on `line2`/`yellow` (#8c8578/#b8860b vs #99907f/#a06f00); the TUI literals were transcribed verbatim per the risk table, and the TUI parity test now pins them. Step 06's parity machinery adjudicates the pair.
- `git+file` flakes cannot see untracked files, so new modules must be `git add`-ed before any nix gate can compile them; review-iteration split files under `crates/panel-kit-core/src/theme/` are currently new working-tree files for the orchestrator to stage with the rest of the Phase 1 changes.
- Typography/density defaults pinned from design §8.1's example values, cross-checked against DESIGN.md's frontmatter (13px/1.5 body, .72rem/700/.06em label, 4/999 radius, 4/6/8 spacing).
- Review iteration 2 split the theme implementation into focused submodules (`theme/color.rs`, `theme/schema.rs`, `theme/semantic.rs`, `theme/presets.rs`, `theme/tokens.rs`, `theme/tests.rs`); `Token::check` now delegates through the single `Color::parse` codec, and every new theme file is under 200 lines.

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | PAPER palette literals (tui theme.rs:66-82) have no core seed yet — value transcription errors | Med | Med | Transcribe values verbatim; the step-06 theme-parity comparison will mechanically verify every token |
| Risk | Removing tokens.rs values breaks `build.rs` (root build-dependency on core) | High | Low | Keep `tokens.rs` as a re-export shim THIS step; step 06 regenerates consumers and can inline further |
| Risk | Parallel step 01 edits core `lib.rs` | Low | Med | This step's `lib.rs` edit is `pub mod theme;` + re-export lines only |
