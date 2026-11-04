# panel-kit Bevy Binding — Implementation Plan

## Goal

Add a first-class native Bevy renderer/binding for panel-kit: `panel-kit-bevy`.
It must consume `panel-kit-core` directly and expose the same workspace
semantics the Dioxus and ratatui renderers provide: panel catalog, floating and
tiling modes, panel chrome, minimized dock, drag/resize/reorder, traffic-light
window management, keyboard commands, and versioned persistence.

## Design rules

- Create `crates/panel-kit-bevy` inside the existing panel-kit workspace.
- Use `bevy` as a normal dependency, not a workspace-wide required dependency;
  `panel-kit` and `panel-kit-tui` must not require Bevy to build.
- Use `panel-kit-core` as the sole source of truth for state and geometry.
  The Bevy renderer must not invent its own panel state machine.
- Public API should look like:
  - `BevyPanelKind` trait or re-exported `PanelKind` for the app's panel enum.
  - `BevyPanelKit` resource holding `reducer::Snapshot`, `PanelCatalog`, and
    current `Mode`.
  - `PanelBodyContent`/callback or entity-marker mechanism so applications can
    place their own UI inside a panel body.
  - File-backed layout persistence using the shared `persist` types.
- Bevy UI is the target for this first pass. A WGPU canvas bridge can be a
  follow-up; this plan does not require one.

## Milestones

### Task 1 — Create the binding crate

- Add `crates/panel-kit-bevy` to the workspace and root dependency graph.
- Add Bevy as a target-specific/optional dependency where practical.
- Define the public resource/component vocabulary:
  - `BevyPanelKit`
  - `PanelBody`
  - `PanelChrome`
  - `MinimizedDock`
- Add a minimal `Snapshot::from_defaults`-based construction API.
- Add compile/test smoke tests proving the crate builds and that defaults
  produce the expected panel count.

### Task 2 — Render the workspace

- Translate `panel_kit_core::frame::project_into` output into Bevy UI nodes.
- Render floating/tiling panels, panel chrome, body slots, minimized dock, and
  panel titles.
- Use panel-kit role/token colors for panel background, border, and header.
- Reconcile UI entities when the snapshot changes instead of respawning the
  whole UI every frame.
- Add snapshot-to-UI tests and a small screenshot/manual example.

### Task 3 — Translate input into core events

- Implement pointer press/move/release, hover, and wheel translation into
  panel-kit core events.
- Wire traffic lights: minimize/maximize/restore, mode toggle.
- Implement drag/resize/reorder and viewport clamping through `apply_drag`,
  `begin_drag`, `begin_tile_resize`, and related core functions.
- Wire keyboard window-management commands through `command_for` and
  `apply_command`.
- Avoid consuming input when the pointer/body content should own it.

### Task 4 — Persistence and application API

- Add file-backed `LayoutStore` implementation suitable for native Bevy apps.
- Add save/restore of `SavedLayoutV2`, including view registry if practical.
- Add a stable app-facing resource API to update the catalog, change modes,
  trigger commands, and access panel body entity handles.
- Add round-trip persistence tests.

### Task 5 — Example and integration notes

- The binding's first application is the procgen-forge viewer, which hosts
  its Forge and Inspector panels through `PanelUiPlugin`, spawns content via
  `PanelBodyHandles`, and uses a transparent `PanelBackdrop` so the 3D
  generated object stays visible between panels.
- Document the API in `README.md` and update the crates section.
- Ensure `cargo fmt --check`, `cargo test`, and `cargo clippy --workspace
  --all-targets` pass.

## Verification

- `cargo test -p panel-kit-bevy`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets`
- Run the example and manually verify drag/resize/minimize/maximize/restore
  and floating/tiling mode toggle.

## Follow-up

- Add a procgen-forge integration using this binding.
- Add a Bevy WGPU render-target panel if needed later.
