# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primary users are **`ocasazza` and `olivecasazza` GitHub consumers** — the
author and the small set of applications in those two accounts that depend on
panel-kit as a git dependency. Today that is `jump-cannon` and
`apple-notes-ocr-flow`; the audience is "my own apps and the people running
them," not the open crates.io public.

Two distinct jobs sit behind that one audience:

- **Integrator** (the same person, in library-authoring mode): wire a panel
  enum and a body renderer into an app and get a complete workspace shell —
  geometry, z-order, drag state, persistence, clamping — without writing any
  of it. Consumes the crate through a Nix flake and a pinned git revision.
- **Operator** (the person using a consuming app): sits in front of the
  workspace for long sessions, arranging panels around a task — inspecting a
  graph, reviewing OCR output — in either a browser or a terminal.

## Product Purpose

panel-kit provides a reusable panel-workspace shell: every view is a panel the
operator can move, resize, minimize, maximize, tile, and reorder, with layout
persisted between sessions. It exists because that shell was built twice (in
`jump-cannon` and `apple-notes-ocr-flow`) and was extracted so it is built
once. Success is an app getting a credible, complete window-management surface
from two impl items and one CSS injection.

## Positioning

**One core, many renderers.** A single renderer-neutral state machine
(`panel-kit-core`) drives every backend with identical semantics —
`PanelWin`, `WinState`, `Mode`, the drag/resize/reorder math, viewport
clamping, and the persisted `SavedLayout` shape are shared, and platform
events are translated at the backend boundary. A neighboring library could
copy the chrome; it could not truthfully claim the shared-core parity.

The durable ambition is **one unified tileable / floatable / muxable window
manager offering the same experience and interface on most platforms** —
web, terminal, iPad, Android, native desktop, and embedded. Two of those
exist today:

- `panel-kit` (Dioxus/web) — DOM workspace.
- `panel-kit-tui` (ratatui) — terminal workspace; via ratzilla it also
  targets the browser DOM, so one panel codebase already ships two skins on
  the same platform.

The remaining platforms are stated direction, not shipped capability. Design
work should not foreclose them: anything a future backend must agree on
belongs in core, and "web and terminal both do it this way" is not the same
as "core owns it".

## Operating Context

- Consumed as a **git dependency pinned by revision**, resolved through a Nix
  flake, not from crates.io.
- Local development against a checkout uses a Cargo `[patch]` entry pointing at
  `../../panel-kit`.
- The dev shell (`nix develop`) provides a matching dioxus-cli 0.6.x, lld, and
  `wasm-bindgen-cli`; `Cargo.lock` pins `wasm-bindgen` to the exact nixpkgs
  `wasm-bindgen-cli` version because `dx` refuses to bindgen on a mismatch.
- Web examples run with `dx serve --example <name> --platform web`; the TUI
  runs natively with `cargo run -p panel-kit-tui --example workspace`; the
  browser-TUI canary runs under `trunk`.
- Documentation is rustdoc-first (`cargo doc --no-deps --open`), built for
  `wasm32-unknown-unknown`.

## Capabilities and Constraints

**Confirmed capabilities**

- Workspace modes: floating (free placement) and tiling (auto grid), toggled
  workspace-globally.
- Per-panel controls inset into the top border row: mode toggle, minimize,
  maximize/restore.
- Floating drag-move, corner resize, z-raise on interaction; tiling
  drag-header reorder.
- Minimized panels collapse to a dock strip and restore from it.
- Viewport clamping on resize, with vertical overflow reachable by
  workspace-level scroll and manual scroll chaining into panel bodies.
- Layout persistence: localStorage on web, JSON file in the terminal, with
  new panel kinds merged into an older saved layout so added features appear
  for existing users.
- Narrow-viewport shell (< 760 px): forced tiling, single-column stack, window
  management hidden.
- Shipped widgets: `Badge` (ten kinds, click/hover actions, tint overrides) and
  `Spinner`; the terminal renderer additionally ships charts, meters, tables,
  status and scroll helpers.
- `is_editing()` shortcut gate and `tip_pos()` viewport-aware tooltip
  placement as app-facing utilities.

**Constraints**

- Architectural rule (AGENTS.md): renderer-neutral state, geometry, pointer
  semantics, and persisted shape live in `panel-kit-core`; the root crate is
  web-only concerns; `panel-kit-tui` is terminal-only concerns. Behavior that
  should match across backends is encoded in core first.
- The root crate is wasm-only and expects a browser at runtime.
- Theming is CSS-variable driven from one injected stylesheet (`panel_kit::CSS`),
  with the terminal palette (`Theme`) mirroring the same variable names field
  for field.
- Examples are treated as canary tests for backend drift, not decoration.

**Breaking-change policy (confirmed)**

The public API is **not frozen**. Because consumers pin panel-kit by revision
through their Nix flakes, a breaking change is shipped by: (1) opening a GitHub
issue notifying `ocasazza` / `olivecasazza` consumers, and (2) leaving those
consumers on their existing pinned revision until they migrate. Migration is
pull-based, per consumer. Design work may therefore change public surface when
the design calls for it, provided the notify-and-pin path is followed.

## Evidence on Hand

- Two real consumers: `jump-cannon` and `apple-notes-ocr-flow` (both depend on
  panel-kit as a git dependency).
- Four web examples: `workspace`, `badge`, `spinner`, `theming`
  (`examples/`).
- Three terminal examples: `workspace`, `workspace_canary`, `browser_tui`
  (`crates/panel-kit-tui/examples/`).
- Incumbent visual system: `assets/panel-kit.css` (186 lines; 15 `:root`
  declarations — 14 colors plus the `--mono` stack — and the panel /
  light / dock / badge / spinner / tooltip / mobile rules) and
  `crates/panel-kit-tui/src/theme.rs` (`Theme::DARK`, `Theme::PAPER`).
- No usage analytics, no user research, no testimonials, no published
  crates.io release, and no performance benchmarks exist. Future work must not
  invent them.

## Product Principles

1. **Core owns behavior; backends only translate.** If web and terminal should
   agree, the agreement is encoded in `panel-kit-core` and each backend adapts
   to it. A renderer-specific state or event type is a design failure unless
   the concept genuinely cannot exist in core.
2. **Two impl items, whole shell.** Adopting panel-kit must never require the
   app to reimplement geometry, z-order, drag, clamping, or persistence.
3. **Examples are canaries.** Every supported behavior is exercised by a
   buildable example; prose documentation never substitutes for one.
4. **Density over decoration.** The workspace is a long-session working
   surface; vertical space and legibility beat chrome. Panel controls are inset
   into the border row precisely to buy back content height.
5. **Breaking changes are announced and pull-based.** Consumers pin revisions;
   the crate may evolve, but never silently under a consumer's feet.

## Accessibility & Inclusion

No product-specific standard has been established. Existing signals only:
`:focus-visible` treatments on traffic lights and badges, and a mobile shell
that removes pointer-only affordances. Keyboard operability of panel
management, reduced-motion handling, and screen-reader semantics are
**undecided** — not claimed, not designed.
