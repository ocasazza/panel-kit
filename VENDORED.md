# Vendored components

Phase 1 of the auto-loop panel-kit migration: shared UI components lifted out
of jump-cannon's web shell (`app/ui`) into this library, so Phase 2 can delete
the jump-cannon copies and adopt the library API. jump-cannon sources were
read at origin/main `44ad08a9f693b53e3f4f784235896f0a3a158579` (read-only;
nothing was committed to jump-cannon in this phase).

| Component | jump-cannon source | panel-kit model (core) | panel-kit painter (web) |
|---|---|---|---|
| Selectable card + chip | `app/ui/src/selection_card.rs` | `widgets::selection::SelectionCard` | `widgets::selection::{SelectableCard, card_chip}` |
| Build progress | `app/ui/src/build_progress.rs` | `widgets::build_progress::{BuildFeed, ProgressEvent, event_line, fmt_elapsed}` | `widgets::build_progress::{progress_bar, PanelHydrating}` |
| Hint registry + hint bar | `app/ui/src/hints.rs` | `widgets::hints::{Hint, HintRegistry, node_hint, NodeHintMeta}` | `widgets::hints::{publish, clear, HintBar}` |
| Badge detectors + status colour | `app/ui/src/badges.rs` (pure layers only) | `widgets::detect::{parse_wikilink, host_from_url, is_url, is_iso_date, parse_ticket_id, StatusColor, status_color}` | `widgets::detect::status_color` (CSS variables) |
| Form rows | `app/ui/src/panels/layout.rs` (~2060–2196) | `widgets::forms::{parse_clamped_f64, parse_clamped_i64, fmt_f, log_position, log_value, slider_range}` | `widgets::forms::{Slider, IntRow, CheckRow, TextRow}` |
| Neighbour pills | `app/ui/src/panels/inspector.rs` (~262) | `widgets::selection::{Pill, PillDirection, short_id_for_pill, build_pills, neighbor_set}` | `widgets::pills::pill_list` |
| Frontmatter leftover grid | `app/ui/src/panels/inspector.rs` (~538) | `widgets::frontmatter::{GRID_SKIP_KEYS, grid_skipped, chip_walker_handles, leftover_rows}` | `widgets::frontmatter::{frontmatter_grid, fm_value_cell}` |
| Frame ring | `app/ui/src/panels/timeline.rs` (`FrameRing`) | `widgets::frame_ring::FrameRing` | none — pure buffer; it has no painter by nature |

## What was stripped

- jump-cannon's `NodeMeta`, `NodeFact`, `proto`, `Ctx`, `api`, and the
  global `filter`/appstate signals. Every model carries plain fields now;
  hosts map their wire formats onto them (`BuildFeed::fold` takes
  [`ProgressEvent`]s, `build_pills` takes a `tint_of` closure instead of the
  `community_tint` palette, `node_hint` takes `NodeHintMeta`).
- jump-cannon CSS class names became the `pk-` namespace in
  `assets/panel-kit.css` (role tokens only; a reduced-motion variant kept for
  the indeterminate bar). The selectable card's regression `data-*`
  attributes (`data-source-id`, `data-default`, `data-viewing`,
  `data-ghost`, `data-kind`) are preserved, and the state classes
  (`active` / `default` / `ghost` / `viewing`) stay host-composed.

## Deliberately not vendored

- `app/ui/src/palette.rs` — `community_tint` and the palette registry are
  threaded through jump-cannon's `Ctx` (metric state + action registry), so
  the colour arrives as a parameter instead.
- `SeedRow` (`panels/layout.rs`) — a u64 re-roll row tied to the generate
  panel's seed semantics; jump-cannon keeps it.
- The timeline panel itself (scrub UI, capture ticker, localStorage knobs) —
  only the `FrameRing` buffer is shared; the panel is a jump-cannon surface.

## Ratatui painters

Deferred, and noted here so Phase 2 can schedule them: the selectable card,
form rows, pills, hint bar, and frontmatter grid are interactive DOM surfaces
(buttons, range inputs, text entry, wrapping click targets) without trivial
terminal counterparts. The core models are renderer-neutral, so terminal
painters can be added later against the same types; the frame ring, detectors,
form parsing, hint registry, pill builder, and feed fold are logic with no
painter at all.

## Example

`examples/vendored.rs` exercises every painter; it is built by the flake
check `vendored-web-wasm` and linted by the `clippy` aggregate's
`vendored-web` lane.
