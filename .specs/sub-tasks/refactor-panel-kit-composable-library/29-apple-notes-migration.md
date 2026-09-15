# Step 29: apple-notes-ocr-flow consumer migration (reviewer/editor priority)

**Task File:** `.specs/tasks/in-progress/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 8
**Model:** sonnet
**Agent:** developer
**Depends on:** `27-controller-cutover`
**Parallel with:** `28-jump-cannon-migration`
**Note:** None

**Goal:** Migrate the external git-pinned consumer **apple-notes-ocr-flow** per design §13.3 (HR-21): preserve reviewer/editor event priority (editor/text input and document shortcuts consume events FIRST — bare panel commands never steal typing) and the persisted key/serde IDs; its REAL wasm reviewer target (resolved from the actual checkout — design §14 AC-21 explicitly forbids inventing one or mocking it) builds before the pin update.

The design is explicit that this repo contains no apple-notes source tree (`README.md:9-16`, `src/lib.rs:1-13` provenance only): INVENTORY FIRST. Locate the checkout (git pin references, org checkouts, or the consumer's own repo); inventory every panel-kit call site, the localStorage key, panel serde IDs, and the existing wasm reviewer target name. Then apply §13.3's nine migration steps (catalog with identical stable IDs → editor/document handlers first → translate only unconsumed pointer/key/wheel → explicit `ViewportChanged` policy → reviewer bodies/header actions in parts → portable badges/charts/tables/status to core models with OCR data staying borrowed app providers → same-key restore + phase-based save + reset/clear safety → optional later Nix authoring → wasm build, then pin update).


#### Inventory (recorded before edits)

- Checkout resolved: `../snake-pit` is the reachable renamed checkout for `ocasazza/apple-notes-ocr-flow` (GitHub resolves that repository name to `ocasazza/snake-pit`).
- Reviewer source: `../snake-pit/app/ui/src/main.rs`.
- Removed controller call sites present before migration:
  - `../snake-pit/app/ui/src/main.rs:22` imports controller-era root re-exports only (`Mode`, `PanelWin`, `WinState`) and no composable core APIs.
  - `../snake-pit/app/ui/src/main.rs:853` calls `panel_kit::use_workspace("snake_pit_layout_v4", default_layout)`.
  - `../snake-pit/app/ui/src/main.rs:879-887` reads/writes `ws.panels` for auth projection.
  - `../snake-pit/app/ui/src/main.rs:1280` calls `ws.effective_mode()`.
  - `../snake-pit/app/ui/src/main.rs:1326-1330` calls `ws.root_class()`, `ws.handle_mouse_move(&e)`, and `ws.handle_mouse_up()`.
  - `../snake-pit/app/ui/src/main.rs:1394-1416` calls `ws.render(...)`.
  - `../snake-pit/app/ui/src/main.rs:1418` calls `ws.dock()`.
- Persisted layout key: `snake_pit_layout_v4` from the `use_workspace` call; related app-owned auth key `snake_pit_layout_v4_auth_minimized` remains separate.
- Panel serde IDs: `Account`, `Cluster`, `Files`, `Original`, `Controls`, `Charts`, `Markdown`, `Assembled`, `Rendered`, `Jobs`, `Settings`, `Help`, `Debug` from the existing `Panel` enum (`Serialize`/`Deserialize` unit variants, no rename attribute).
- Resolved real reviewer wasm target: Cargo package `snake-pit-ui` in `../snake-pit/app/ui/Cargo.toml`; focused build target is `wasm32-unknown-unknown` (`cargo check --manifest-path app/Cargo.toml -p snake-pit-ui --target wasm32-unknown-unknown --tests`).

#### Phase 8 scope boundary

- HR-21 migration-owned behavior is limited to reviewer workspace ownership: the `Snapshot`/catalog/projection/reducer wiring, editor-first event refusal, unchanged `snake_pit_layout_v4` persistence key, unchanged `Panel` serde IDs, explicit `SavePolicy` persistence, and the real `snake-pit-ui` wasm build proof.
- `Account`/`Cluster` surfaces, rendered Markdown, node health, pipeline progress, model inventory, and cluster/account API/UI behavior are app-owned Snake Pit functionality present in the reachable consumer checkout at the Step 29 inventory boundary, not panel-kit surfaces added by HR-21. They are preserved only so the stable panel ID set and existing app UI keep working; they are not counted as new panel-kit migration behavior.
- The app-domain helper modules (`account_cluster.rs`, `pipeline_progress.rs`, `node_status.rs`, `markdown_rendering.rs`, `model_inventory.rs`) remain outside the HR-21 migration proof. Focused checks for those helpers may be run to avoid regressing pre-existing app behavior, but the panel-kit migration evidence is the reviewer workspace tests plus the resolved wasm target build.


#### Files Changed and Verification Evidence

| File | Action | Evidence |
| --- | --- | --- |
| `../snake-pit/app/Cargo.toml` | Modified | Adds a workspace `[patch."https://github.com/ocasazza/panel-kit"]` for `panel-kit` and `panel-kit-core` to the sibling post-cutover checkout because no committed post-cutover git revision exists yet. |
| `../snake-pit/app/ui/Cargo.toml` | Modified | Adds the direct `panel-kit-core` dependency required by the composable reducer/projection/persistence APIs while retaining the git-pinned dependency shape until a true post-cutover rev can replace the old rev. |
| `../snake-pit/app/ui/src/main.rs` | Modified | Wires the inventoried reviewer panel set through host-owned composition: `Snapshot` restore, viewport event reduction, `ProjectionBuffer` projection, panel parts, dock parts, and app-first keyboard refusal. App-domain panel bodies/helpers remain outside the HR-21 migration boundary above. |
| `../snake-pit/app/ui/src/review_workspace.rs` | Created | Consumer-local reviewer panel-kit module containing the migration-owned catalog, restore/save, explicitly named reduce-and-persist helper, projection, event-priority, auth-layout projection, and reviewer persistence tests. |
| `../snake-pit/app/Cargo.lock` | Modified | Resolves the patched local `panel-kit`/`panel-kit-core` crates at version `1.0.0` for the focused consumer build. |

Focused checks run:

- `cargo test --manifest-path app/Cargo.toml -p snake-pit-ui reviewer_workspace_tests` — PASS (`3 passed; 0 failed`).
- `cargo test --manifest-path app/Cargo.toml -p snake-pit-ui pipeline_progress_tests && cargo test --manifest-path app/Cargo.toml -p snake-pit-ui model_inventory_ui_tests && cargo test --manifest-path app/Cargo.toml -p snake-pit-ui markdown_security_tests` — PASS (`6 passed; 0 failed` across preserved app-domain helper checks; boundary-only, not HR-21 migration evidence).
- `cargo check --manifest-path app/Cargo.toml -p snake-pit-ui --target wasm32-unknown-unknown --tests` — PASS.
- Built-in repository search for `reduce_review_event` in `../snake-pit/app/ui/src` — no matches; call sites now use `reduce_and_persist_review_event`.
- Built-in repository search for `panel_kit::use_workspace|ws.render|ws.dock|ws.handle_mouse|ws.root_class|Workspace<` in `../snake-pit/app/ui/src` and `../snake-pit/app/ui/Cargo.toml` — no matches.

Pin boundary:

- The real reviewer target builds against the sibling post-cutover panel-kit working tree through the app workspace patch above. A true git `rev` pin cannot be updated in this step because the post-cutover panel-kit tree is not committed; updating the old rev to the current panel-kit `HEAD` would point at the pre-cutover controller API. This is coordinated with Step 28 and should be replaced by the eventual committed post-cutover panel-kit revision.

#### Expected Output

- apple-notes-ocr-flow reviewer UI migrated (external repo)
- Inventory record (call sites, key, IDs, resolved wasm target)
- `apple-notes-ocr-flow-reviewer-wasm` build proof

#### Success Criteria

- [X] Inventory recorded BEFORE edits: call sites, localStorage key, panel serde IDs, real reviewer wasm target name
- [X] Reviewer/editor event priority preserved: typing/editor/document shortcuts consume events first; panel input sees only unconsumed events (asserted by `reviewer_workspace_tests::reviewer_text_inputs_keep_first_refusal_over_panel_commands`)
- [X] Persisted key and serde IDs unchanged; restore/save/reset-clear behavior safe (V1/V2 compatibility kept by `reviewer_workspace_tests::reviewer_layout_uses_same_key_and_reset_clears_without_resave`)
- [X] The resolved REAL wasm reviewer target builds against post-cutover panel-kit before any true git pin update (`cargo check --manifest-path app/Cargo.toml -p snake-pit-ui --target wasm32-unknown-unknown --tests`); temporary workspace patch recorded because the committed post-cutover rev does not exist yet
- [X] No `use_workspace`/`ws.render`/`ws.dock()` controller usage remains in its sources
- [X] Phase 8 re-review scope boundary documented: app-domain Account/Cluster/Markdown/node/progress/model helpers are preserved consumer behavior and not counted as HR-21 migration evidence
- [X] Persistence side effect is visible at call sites through the `reduce_and_persist_review_event` helper name

#### Subtasks

- [X] Locate the apple-notes-ocr-flow checkout; record the inventory (key, IDs, call sites, wasm target)
- [X] Apply the design §13.3 nine-step migration to the reviewer UI
- [X] Add the event-priority preservation check (editor-first consumption observable)
- [X] Build the resolved wasm reviewer target against this repo's revision; record the proof and the blocker for replacing the temporary workspace patch with a true git rev pin
- [X] Address Phase 8 re-review scope and call-site honesty findings: documented the Snake Pit app-domain boundary and renamed the reducer helper to expose persistence

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Blocker | No accessible checkout (design says none exists in this repo) | High | High | Inventory-first; if unreachable, deliver the inventory recipe + patch plan + explicit blocker — never invent file names or fake a target (AC-21) |
| Risk | Wrong wasm target chosen (mock/app instead of the real reviewer app) | High | Med | Target name must come from the checkout's own build config, not from design guesses; record it in the inventory |
| Risk | Editor-first priority regression (panel commands steal keystrokes) | High | Med | §13.3 step 3 is explicit; add the priority check; reviewer smoke-run if build tooling permits |
