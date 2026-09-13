# AGENTS.md — panel-kit project instructions for AI coding agents

## Architecture Rule

`panel-kit-core` is the abstract/base interface and state-machine shape for
panel-kit. Every renderer must implement that shared shape rather than
inventing its own parallel semantics.

Concretely:

- Put renderer-neutral data, layout state, pointer/input semantics, geometry
  math, persisted layout shape, and shared state transitions in
  `crates/panel-kit-core`.
- Treat the root `panel-kit` crate as the Dioxus web backend: DOM events,
  signals, CSS, localStorage, and web rendering only.
- Treat `crates/panel-kit-tui` as the ratatui backend: terminal/browser-cell
  rendering, terminal persistence adapters, and platform event adapters only.
- Web and TUI versions should be different backends over the same core
  interface. If behavior should match, encode the behavior in core first and
  make each backend adapt to it.
- Avoid adding renderer-specific event or state types when a core abstraction
  can represent the concept. Platform events should be translated at backend
  boundaries into core input types.

## Loading / Hydration Convention

Page hydration is generic and lives in `src/loading.rs` (web) over
`panel-kit-core/src/loading.rs` (shared state shape):

- **Chrome first, data lazily.** Render the workspace immediately; every
  async data source gets a store (`use_loading_store(id, label)`,
  pinia-style: same id, same store, actions `begin` / `update` / `succeed` /
  `fail`). Panel bodies mount behind `LoadingGate`, the workspace-level
  aggregate is `GlobalLoadingBar`, and the pre-chrome page state is
  `LoadingWorkspace` (with its static `BOOT_HTML`/`BOOT_CSS` twin).
- **Bars over spinners.** A pending load renders a `ProgressBar`, never a
  bare spinner; `Spinner` is only for tiny inline waits. A determinate bar
  MUST show its percentage text; `fraction: None` is the honest
  indeterminate state — animate, never fabricate a number.
- **Stores are ephemeral.** In-flight status only: never persisted, never
  undoable. Rewind/undo/history of application state belongs to the
  consuming app's snapshot-timeline framework; a rewind is modelled as an
  ordinary store transition (`begin` → `succeed`) so history replays surface
  on the same bars with no parallel status vocabulary.

## Design Language

UI/UX work in this repo follows the design vocabulary and principles of
https://impeccable.style/ — applied within the panel-kit monospace aesthetic
(Courier Prime, `:root` theme variables). Concretely for this codebase:
honest progress (measured % or explicit indeterminate, never a fake number),
`prefers-reduced-motion` variants for every animation, correct ARIA roles on
status/progress surfaces, one clear hierarchy per surface (label → detail →
percentage), and no decorative AI-slop chrome (no gradient hero cards, no
pulsing-dot "AI is thinking" theater).

## Examples as Canary Tests

Examples are executable documentation and should be comprehensive enough to
catch drift between backends.

- Keep examples broad when they document shared behavior: workspace chrome,
  floating/tiling mode, drag/resize/reorder, dock restore, traffic lights,
  badges, spinner, theming, charts, scrolling, and persistence where relevant.
- Prefer buildable examples over prose-only docs. If an example demonstrates a
  supported backend, add or maintain a build check for it when practical.
- Browser examples should compile to `wasm32-unknown-unknown`; terminal
  examples should continue to build natively.
