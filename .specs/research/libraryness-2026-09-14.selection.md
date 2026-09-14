# Pruning selection — libraryness (2026-09-14)

## Vote tally (ranked choice: 1st = 3 pts, 2nd = 2, 3rd = 1)

| Proposal | Judge 1 | Judge 2 | Judge 3 | Points | Avg score |
| --- | --- | --- | --- | --- | --- |
| **A1** Headless core reducer + pluggable store + escape-hatch shells | 1st (3) | 1st (3) | 2nd (2) | **8** | 4.14 |
| **C-B** Headless Event Reducer + Frame Projection; Backends as Widget Kits | 3rd (1) | 2nd (2) | 3rd (1) | **4** | 3.86 |
| **C-F** Spec Compiler with Capability-Assembled Shells + Parity Matrix | — | 3rd (1) | 1st (3) | **4** | 3.51 |
| C-E Shell Deletion: Capability Kit and Metadata-as-Data | 2nd (2) | — | — | 2 | 3.87 |

Per-judge score tables: `pruning.1.md`, `pruning.2.md`, `pruning.3.md`.

## Selected for expansion

1. **A1** — headless `WorkspaceState` reducer in core + `LayoutStore` trait promoted to core + shells reduced to adapters with escape hatches. Unanimous top-2; highest average.
2. **C-B** — core owns an event reducer and emits a projected *frame* (regions + z + chrome) that backends only paint; backends become widget kits.
3. **C-F** — Nix spec compiler: a full `WorkspaceSpec` surface compiled from Nix, with a parity matrix check that mechanically proves Nix ↔ Rust equivalence.

Diversity: three distinct mechanisms (state ownership / projection boundary / spec-and-parity). Judges 1 and 2 both dropped **C-A** as a mechanism-duplicate of A1; judge 2 also dropped **C-E** as a duplicate of C-B.

## Judge-raised concerns every expansion agent must address

- **Repo drift (mandatory).** All three judges found the proposals were written against `HEAD 6120e9b` while the working tree carries a large uncommitted refactor. Already landed: `SurfaceProfile`/`SurfaceClass`/`effective_mode`, the `Key`/`KeyChord`/`PanelCommand`/`command_for`/`apply_command` keyboard layer, `SavedLayoutV2`/`StoredLayout`/`Units`/`migrate_v1`/`reconcile_units`, `mkLayout` emitting V2, and **`pub trait LayoutStore` already existing in `panel-kit-tui`**. Do not re-propose these; build on them, and fix the fact that `LayoutStore` sits in a backend crate rather than core.
- **Shape-only parity is proven insufficient.** `flake.nix` `checks.layout-canary-schema` is green while `nix/examples/workspace-canary.nix` (7 panels) has drifted from `workspace_canary.rs::defaults()` (9 panels: `Flame` and `Distribution` added). A design that only extends the jq assertions is rejected; parity must be mechanized against Rust types.
- **No god-object replacement.** Do not trade `Workspace<K>` for an equally monolithic `WorkspaceState` that apps must route everything through; name the escape hatches and show a single-panel / partial-chrome composition.
- **Justify each abstraction with a concrete composition it enables**, and name what is deleted (duplicated per-backend load/save, hand-mirrored canary, per-backend theme sources).
- **Migration must be concrete** for `jump-cannon` and `apple-notes-ocr-flow`, aligned with the existing `MIGRATION.md` 1.0.0 clean-cutover style (no deprecated aliases).
- **Widget/theme asymmetry** (TUI-only `Theme`, `charts`/`table`/`meter`/`scroll`, double `Badge`) must be addressed or explicitly declared a non-goal with rationale.
- **No runtime Nix dependency** in consuming apps; Nix evaluates at build time only.
