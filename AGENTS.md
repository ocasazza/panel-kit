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
