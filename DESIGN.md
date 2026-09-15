---
name: panel-kit
description: A unified tiling, floating, muxable window-manager shell — one near-black console, rendered on every platform.
# BEGIN GENERATED THEME TOKENS
colors:
  bg: "#0a0a0a"
  panel: "#0d0d0d"
  fg: "#ededed"
  dim: "#7a7a7a"
  line: "#262626"
  line2: "#5f5f5f"
  inv-bg: "#ededed"
  inv-fg: "#0a0a0a"
  accent: "#5ef38c"
  red: "#ff5f56"
  yellow: "#ffbd2e"
  green: "#27c93f"
  blue: "#3b9bff"
  pink: "#ff5fc3"
  badge-info: "#83b7cc"
  focus-ring: "#ededed"
typography:
  title:
    fontFamily: "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace"
    fontSize: "0.8rem"
    fontWeight: 600
    lineHeight: 1.5
    letterSpacing: "0.06em"
  label:
    fontFamily: "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace"
    fontSize: "0.72rem"
    fontWeight: 700
    lineHeight: 1.5
    letterSpacing: "0.06em"
  body:
    fontFamily: "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
  caption:
    fontFamily: "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace"
    fontSize: "0.72rem"
    fontWeight: 400
    lineHeight: 1.3
    letterSpacing: "0.02em"
  micro:
    fontFamily: "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace"
    fontSize: "0.65rem"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
rounded:
  chip: "3px"
  panel: "4px"
  overlay: "5px"
  pill: "999px"
  dot: "50%"
spacing:
  hair: "2px"
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "12px"
  xl: "16px"
# END GENERATED THEME TOKENS
components:
  panel:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.fg}"
    rounded: "{rounded.panel}"
    padding: "0"
    width: "min 180px"
    height: "min 110px"
  panel-body:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.fg}"
    typography: "{typography.body}"
    padding: "28px 8.8px 8.8px"
  panel-title:
    textColor: "{colors.fg}"
    typography: "{typography.label}"
    padding: "0"
  light-mode:
    backgroundColor: "{colors.blue}"
    rounded: "{rounded.dot}"
    size: "12px"
  light-minimize:
    backgroundColor: "{colors.yellow}"
    rounded: "{rounded.dot}"
    size: "12px"
  light-maximize:
    backgroundColor: "{colors.pink}"
    rounded: "{rounded.dot}"
    size: "12px"
  dock-chip:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.fg}"
    rounded: "{rounded.chip}"
    typography: "{typography.caption}"
    padding: "3.2px 8px"
  dock-chip-hover:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.fg}"
    rounded: "{rounded.chip}"
    padding: "3.2px 8px"
  badge:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.fg}"
    rounded: "{rounded.pill}"
    typography: "{typography.caption}"
    padding: "4px 10px"
  badge-small:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.fg}"
    rounded: "{rounded.pill}"
    typography: "{typography.caption}"
    padding: "2px 6px"
  badge-active:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.fg}"
    rounded: "{rounded.pill}"
    padding: "4px 10px"
  spinner-label:
    textColor: "{colors.accent}"
    typography: "{typography.caption}"
    padding: "0"
  tooltip-overlay:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.fg}"
    rounded: "{rounded.overlay}"
    typography: "{typography.caption}"
    padding: "8px 9.6px"
    width: "228px"
---

# Design System: panel-kit

## Overview

**Creative North Star: "The Lit Console"**

panel-kit is a near-black console in which the only saturated color is a lit
indicator. Everything structural is graphite — three stops of near-black for
surface, two hairline greys for seams, one bright grey for text, one mid grey
for de-emphasis. Against that, five saturated hues exist and each one *means*
something: printer-CMY traffic lights that name a window operation, and a
phosphor-green accent that only ever says "this is live right now." The
restraint is not minimalism for its own sake; it is what makes a single lit
pixel readable across a wall of panels.

The system is a **window manager, not a page**. Its job is to render a unified
tileable, floatable, muxable workspace with the same experience and interface
on every platform it reaches — web and terminal today, iPad, Android, native
desktop, and embedded as stated direction. That ambition sets the hardest
constraint in this document: *the design language must be expressible in
cells.* A terminal has no shadows, no sub-pixel radii, no gradients, no
opacity. So the visual system is built from what every target can draw —
tonal layering, hairline borders, a fixed monospace grid, and color — and any
finish beyond that set is a web-only garnish, never load-bearing.

Density is the third pillar. This is an Operate surface: an operator arranges
panels around a long-running task and reads dense content inside them. Chrome
therefore yields to content. Panel controls and titles are inset *into* the
top border row rather than occupying a header band, following ratatui's
`Block::title` treatment, precisely to buy back vertical space. Nothing in
this system decorates; everything either labels, separates, or signals.

**Key Characteristics:**

- Near-black graphite base (`#0a0a0a` / `#0d0d0d`) with exactly one text grey and one dim grey.
- Monospace everywhere — no proportional font exists in the system.
- Five saturated hues, each semantically assigned; never used as decoration.
- Flat by rule: depth comes from tone and hairlines, not shadow.
- Chrome inset into borders; content gets the height.
- Every visual decision must survive translation into terminal cells.

## Colors

A graphite console with one phosphor accent and a CMY control cluster; the
palette is small enough to hold in your head and every saturated entry is a
signal, not a style.

### Primary

- **Phosphor Green** (`#5ef38c`): the live-system accent. It appears only where
  something is *happening or selected right now* — the spinner ring and label,
  the skeleton pulse, the topbar activity indicator, the active badge halo, and
  the dashed outline on a panel mid-reorder-drag. It is the brightest, most
  saturated value in the system, which is exactly why it must stay rare.

### Secondary — The Control Cluster

Three hues that exist only in the traffic-light cluster and name window
operations. They are deliberately **not** macOS red/yellow/green, because none
of panel-kit's lights destroys anything; the cluster uses a printer-CMY scheme
instead.

- **Signal Blue** (`#3b9bff`): the workspace-global floating ⇄ tiling mode toggle.
- **Amber** (`#ffbd2e`): minimize to dock. Keeps its macOS meaning.
- **Magenta** (`#ff5fc3`): maximize / restore.

### Tertiary — Status

- **Alert Red** (`#ff5f56`): errors and unresolved references (`badge-wikilink.badge-unresolved`).
- **Confirm Green** (`#27c93f`): resolved/valid state and URL badges. Distinct from the accent: this is a *verdict*, the accent is *activity*.
- **Slate Info** (`#83b7cc`): informational metadata badges (doctype, author, wikilink), and the resting tint for tag badges — tags used to be accent-colored, which spent a live-state signal on static metadata. Declared on `:root`, so web theming reaches it and the terminal mirror (`theme.rs`) stays in step.

### Neutral

- **Console Black** (`#0a0a0a`): the page and the recessed field behind chips and badges.
- **Panel Black** (`#0d0d0d`): every raised surface — panels, topbar, dock, tooltip. Only two stops separate it from the page; that 3-value gap is the entire elevation system.
- **Terminal White** (`#ededed`): body and title text. Also the inverted background for selection.
- **Graphite** (`#7a7a7a`): de-emphasized text — hints, dock labels, max-hints, folder badges.
- **Seam** (`#262626`): structural hairlines that divide regions (topbar bottom, dock top).
- **Edge** (`#5f5f5f`): object outlines that bound a thing (panel border, chip border, scrollbar thumb, resize grip). Chosen by measurement, not taste: the previous `#3a3a3a` computed **1.71:1** against `--panel`, below the 3:1 non-text threshold, which mattered doubly because the resize grip is drawn *only* in this color and so was invisible as a control. `#5f5f5f` computes **3.04:1** against `--panel` and **3.10:1** against `--bg`.

### Named Rules

**The Lit Indicator Rule.** Saturation is reserved for meaning. The accent and
the five status hues together occupy ≤10% of any screen. If a surface reads as
green, something is wrong — not something is live.

**The Two Greys Rule.** There is one text grey (`fg`) and one dim grey (`dim`).
A third intermediate grey is never introduced; if text needs to sit between
them, the answer is weight or case, not a new value.

**The Seam/Edge Rule.** `--line` divides *regions*; `--line2` bounds *objects*.
Never swap them — the two-step ramp is the only cue that says "this is a thing"
versus "this is a boundary between areas."

**The One Source Rule.** The editable palette, typography, and density defaults
live in `panel_kit_core::theme::ThemeTokens` and nowhere else. Every generated
consumer is mechanically derived from that source:

| Consumer | How it stays true |
|---|---|
| `assets/panel-kit.css` `:root` | generated by `build.rs` from `ThemeTokens::dark()` and checked by the theme emitter test |
| `panel-kit-tui`'s `ResolvedTuiTheme` | converted from `ThemeTokens` with no backend-local preset literals |
| The pre-WASM boot stylesheet | generated by `build.rs` from `assets/panel-kit-boot.css.in` |
| Monaco's `panel-kit-dark` theme | built from token values plumbed in from Rust, not a JS table |

This rule was written after the failure it prevents. The palette had three
hand-maintained copies held together by discipline, and the discipline had
already failed: the boot shell carried `#09090a` where the token says
`#0a0a0a`, `#efeff0` for `#ededed`, `#242427` for `#262626`, `#77777d` for
`#7a7a7a`, `#389cf6` for `#3b9bff`, `#f6be3c` for `#ffbd2e`, and `#ee60c2`
for `#ff5fc3` — seven colours, every one wrong by an amount no reviewer would
catch by eye. Plus its own font stack.

Near-miss drift is the worst kind precisely because nothing looks broken. A
copy that is obviously wrong gets fixed on sight; a copy that is one hex digit
off just quietly stops being the same product. With four backends planned,
hand-copying does not scale past the copies that already existed, so the
values moved into the crate both renderers already depend on and a new
backend is now correct by construction.

## Typography

**Display Font:** none — the system has no display face by design.
**Body Font:** `ui-monospace, "SF Mono", "JetBrains Mono", "Menlo", "Consolas", monospace`
**Label/Mono Font:** the same stack. One family, no exceptions, including on `button`.

**Character:** Deliberately instrumental. A single monospace stack across every
role means the web surface shares a metric with the terminal surface, so the
same layout reasoning applies in both; it also means hierarchy has to be earned
through size, weight, case, and tracking rather than through contrast between
typefaces.

### Hierarchy

- **Title** (600, `.8rem`, tracking `.06em`): the topbar product name. The largest type in the system, and still under 11px at the 13px root.
- **Label** (700, `.72rem`, uppercase, tracking `.06em`): panel titles, inset into the border row. Uppercase plus heavy weight plus tracking is how a 9-10px string stays readable at panel scale.
- **Body** (400, `13px`, `1.5`): all panel content. The root size; everything else is a `rem` fraction of it.
- **Caption** (400, `.72rem`, tracking `.02em`): dock chips, badges, hints, spinner labels, tooltip text.
- **Micro** (400, `.65rem`): the maximize hint only. The floor of legibility; nothing smaller exists.

### Named Rules

**The Tracked-Caps Rule.** Any string set below `.75rem` that must be scanned
rather than read is uppercase with `.06em` tracking. Monospace at that size
loses word-shape; caps and tracking give it back.

**The One Family Rule.** No proportional font enters the system, and `button`
inherits `--mono` explicitly rather than falling back to the UA default. The
stack is a token (`tokens::MONO`), not a default, because it is a rule that
can be broken silently: the boot shell shipped `"Courier Prime", …` and the
embedded editor shipped Monaco's own stack until 1.0.0. Both now take the
canonical value, including the editor's portaled widgets.

## Layout

**The grid is the character cell.** Both renderers reason in a fixed monospace
advance; the web surface just happens to measure it in pixels. `TILE_ROW_PX`
carries that row quantum out of the web backend and into shared core math.

**Workspace shell.** A vertical flex column at exactly `100vh`: a 36px topbar
(`flex: 0 0 36px`), the workspace filling the remainder (`flex: 1; min-height: 0`),
and a 30px dock footer (`flex: 0 0 30px`). Both chrome bands are fixed-height
and hairline-separated from the field; the workspace is the only elastic region.

**Floating mode.** Panels are absolutely positioned with app-supplied geometry,
clamped so x/width/height never exceed the viewport while stored `y` is
preserved (floored at 0) so low-placed panels can extend below the fold and be
reached with workspace-level vertical scroll. Wheel events chain manually:
a hovered `.panel-body` that can still scroll in the wheel's direction absorbs
it; otherwise the workspace scrolls.

**Tiling mode.** A grid, not a wrapping flex line — because `tile_w` / `tile_h`
are *spans*, and only a grid can honour a span while still dividing a bounded
height among rows. Columns are `repeat(var(--tile-cols), minmax(0, 1fr))` and
rows are `grid-auto-rows: minmax(var(--tile-row-min), 1fr)`, with `8px` gap and
`8px` padding. A panel emits `grid-column: span <tile_w>` and
`grid-row: span <tile_h>`; the row height comes from the grid, never from the
panel, so a tile is free to be shorter than its content and `.panel-body`
scrolls instead of growing the row. The workspace fills exactly when the tiles
fit and scrolls once `--tile-row-min` is reached. Tiles stay
`position: relative` so the resize grip can anchor to them.

`--tile-cols` is **not** set per tier in CSS. It is written inline on `.ws`
from `SurfaceProfile::tile_columns()` — 1 compact, 2 tablet, `TILE_W_MAX`
regular — and the renderer clamps each emitted span to it. Column count is
surface policy, so core owns it and no stylesheet re-tabulates the tiers. A
span wider than the grid is meaningless, and a DOM grid answers one by
silently manufacturing zero-width implicit columns that swallow the panel.

**Spacing rhythm.** An informal 2/4/6/8/12/16 progression: `2px` badge internal
gap, `4px`/`6px` badge padding and light gap, `8px` tiling gap and padding,
`.55rem` (≈7px) panel-body inset, `.75rem` dock padding, `1rem` topbar padding.
`.panel-body` carries an asymmetric `1.75rem` top inset — that value exists to
clear the border-inset title row, and it is the one magic number in the layout.

**Density and responsive behavior.** Three tiers, not one binary. The surface
is classified in core by `SurfaceProfile::from_logical_width` — **compact**
below 760, **tablet** below 1180, **regular** above — and the root element
carries exactly one of `compact` / `tablet` / `regular`. Classification happens
in Rust and is re-evaluated on every resize rather than in a media query,
because the shell changes *behavior* and not just style; the stylesheet
contains zero `@media` queries. The same policy drives the terminal from cell
counts (`CELLS_COMPACT_MAX` / `CELLS_TABLET_MAX`), so both renderers agree on
what a cramped surface is.

- **Compact** withholds *geometry manipulation*: one column, spans collapse, a
  `200px` row floor, the topbar wraps, and the affordances that move or resize
  a panel by hand (header drag, corner grip, grab cursor) are gone. A phone
  stack is a list. It does **not** withhold the means to leave a window state
  — see the Escape Hatch Rule below.
- **Tablet** keeps the *complete* window-management surface — lights, resize
  grip, reorder, mode toggle — in two columns. This is the tier that makes
  iPad a real target rather than a degraded phone.
- **Regular** is the full four-column grid.

**Sizing follows the pointer, not the width.** `--hit-min` is `24px` by
default and `44px` under `.ws-root.coarse`, which the renderer emits from the
detected `SurfaceCapabilities::coarse_pointer`
(`matchMedia('(pointer: coarse)')`). Keying it to the tier got both ends
backwards: a 1400px kiosk touchscreen is a `regular` surface that still needs
44px, and a 900px desktop window driven by a mouse does not. Width describes
how much room there is; only the pointer describes how precisely it can be
hit. One token still moves every control together.

**The Escape Hatch Rule.** A tier may withhold the means to *enter* a window
state. It must never withhold the means to *leave* one. `SavedLayoutV2`
carries `WinState` across surfaces by design, so a panel maximized on a
regular surface arrives maximized on a compact one; if compact also hid
restore, the operator would be stranded on a single full-bleed panel with
every sibling unreachable and nothing to tap. Compact therefore renders the
magenta restore light — and only that one — for a maximized panel
(`SurfaceProfile::must_offer_restore`). Minimized panels already escape
through the dock, so they need no override.

## Elevation & Depth

**Flat by rule, parity-first.** Depth is expressed with tonal layering and
hairline borders, because those are the only two depth cues a terminal cell
grid can draw. A raised surface is `--panel` on `--bg` with a `--line2` edge;
a divided region is a `--line` hairline. That is the whole system, and it
translates to cells without loss.

Shadow is a **web-only garnish reserved for genuinely floating things** — an
element that has left the document plane and is temporarily above everything
else. A tooltip overlay qualifies. A panel sitting at rest in a workspace does
not.

### Shadow Vocabulary

- **Overlay lift** (`box-shadow: 0 10px 30px #000c`): the fixed tooltip overlay only. It is genuinely detached and viewport-placed.

Note that three further `box-shadow` declarations exist but are **rings, not elevation** — `0 0 0 2px` spreads used for light hover/focus (`assets/panel-kit.css:83`) and badge hover/active halos (`:164`, `:170`). They borrow the shadow channel to draw a state ring. That is legitimate output but mis-housed: rings belong to a `--focus-ring` / state token, so that removing elevation never removes state feedback.

### Named Rules

**The Cell-Expressible Rule.** No depth cue may be load-bearing unless a
terminal can draw it. If removing shadows from the web surface makes z-order
illegible, the fix is a tonal or border treatment that both renderers share —
not a bigger shadow.

**Why this is the whole vocabulary.** The resting panel shadow
(`box-shadow: 0 6px 24px #0007`) was removed in 1.0.0: it shadowed every panel
whether floating, tiled, or docked, and the terminal renderer had no
counterpart for any of it. Z-order legibility now rides the tonal/border
channel both renderers can draw, and a shadow appears only on
`.ws.floating .panel.dragging` — a panel genuinely lifted mid-drag.

Three `0 0 0 2px` declarations remain in the shadow channel but are **rings,
not elevation**: light hover/focus and badge hover/active. They draw state, so
removing elevation never removes feedback.

## Shapes

**Small radii, consistent stroke, one pill.** Every bounded object is a
1px-stroked rectangle with a small radius: `4px` for panels, `5px` for the
tooltip overlay, `3px` for dock chips. The radius ramp is tight on purpose —
at these sizes the corner reads as "machined," not "rounded."

Two shapes break the rectangle, and both earn it: the traffic lights are
`50%` circles (`12px`), because a control indicator should not look like a
surface; and badges are full pills (`border-radius: 999px`, which at their
height is exactly a stadium), inherited from the egui badge they port.
Borders do the work that fills would do elsewhere: a badge's *kind* is carried
by its border color with a near-black fill behind it, so ten badge kinds
coexist without ten colored blocks fighting for attention. The resize grip is
not a border at all but a 14px corner triangle drawn with a
`linear-gradient(135deg, transparent 50%, var(--line2) 50%)` — the same edge
grey, folded.

## Components

### Panels

- **Character:** an instrument module bolted into a rack.
- **Shape:** `4px` radius, `1px` `--line2` border, `--panel` fill, `overflow: hidden`, floors at `180x110px`.
- **Chrome:** controls and title are inset into the top border row, not a header band. The body compensates with a `1.75rem` top inset.
- **States:** `.tile-dragging` drops to `.55` opacity with a `1px` dashed accent outline inset `-1px` — the accent's live-state role applied to drag.
- **Cursor:** `grab` on a tiling panel head, `grabbing` while active; `default` on mobile.

### Traffic Lights

- **Character:** lit indicators on a dark faceplate.
- **Shape:** `12px` circles, `6px` apart, no border, `opacity: .9` at rest.
- **Color:** per-operation via `--light-c` — blue mode, amber minimize, magenta maximize.
- **Hover / Focus:** full opacity plus a color-matched ring; transitions on `opacity` and `box-shadow` at `.12s ease`.
- **Never:** red/yellow/green macOS mimicry. None of these lights destroys anything.

### Dock Chips

- **Character:** a minimized panel's parked nameplate.
- **Shape:** `3px` radius, `1px` `--line2` border, `--bg` fill (recessed below the dock's `--panel`), `.2rem .5rem` padding, `nowrap`.
- **Hover:** border goes to `--fg`. The fill never changes.
- **Empty state:** `.dock-empty` in `--dim` (4.53:1). It was `--line2` — a border token used as 9.1px text at 1.71:1 — until 1.0.0. The Seam/Edge Rule forbids structural tokens as text.

### Badges

- **Character:** a metadata chip that is also a filter control.
- **Shape:** stadium pill, `1px` border, `4px 10px` padding, label clamped to `28ch` with ellipsis and the full value in `title`.
- **Color model:** per-kind classes set `--badge-c` (border) and `--badge-fg` (text) over a `--bg` fill; inline style overrides those plus `--badge-bg` for community tints.
- **Hover** (inactive only): fill drifts 18% toward the border color, plus a 2px 20% halo of the same.
- **Active:** 2px 40% accent halo and an accent-tinted fill, unless a community tint owns the fill.
- **Affordances:** trailing `+` / `×` buttons at 70% of the label color, going to full `--fg` on hover.

### Spinner

- **Character:** the smallest possible "working."
- **Shape:** `12px` ring, `2px` `--line2` border with an accent top edge, rotating `.7s linear infinite`.
- **Label:** optional, accent-colored caption; the span is omitted entirely when empty.

### Tooltip Overlay

- **Shape:** fixed, `228px` wide, `5px` radius, `1px` `--line2` border, `var(--panel)` fill, the system's only true elevation shadow. The fill was a literal `#0d0d0d` until 1.0.0 — identical in the default theme, but it stopped following any retheme, so the overlay stayed near-black over a light workspace.
- **Placement:** viewport-aware and computed in Rust (`tip_pos`) — prefer left of the cursor, flip right without room, clamp inside the window. CSS anchor positioning would do this natively but WebKit does not support it yet.

### Content States

- **Empty** (`.empty`): centered `--dim` caption at `.78rem`, tracking `.03em`.
- **Loading** (`.skeleton`): the same box in accent, pulsing `1.1s`.

### Progress (`.pk-progress`)

- **Character:** the honest report. A bar beats a spinner for anything measurable; the spinner is for tiny inline waits only.
- **Shape:** `.55rem` track, `3px` radius, `1px` `--line2` border, `var(--bg)` fill recessed below the surface it sits on.
- **Fill:** `--accent` — loading is live state, so this is the accent doing its actual job.
- **Honesty rule:** a determinate bar *always* shows its percentage; `None` stays indeterminate rather than inventing a number. The track was `rgba(255,255,255,.06)` until 1.0.0, which is a translucent surface and therefore unexpressible in cells; `var(--bg)` reads identically and survives translation.
- **Reduced motion:** the indeterminate fill goes static at `40%` — not `100%`, which would claim completion, and not faded, which would reintroduce translucency.

### Embedded editor viewport

A third-party code editor is the one component panel-kit does not draw itself,
which makes it the sharpest test of whether the world is real. It is not
granted an exception: its theme is built from `tokens::DARK` values plumbed in
from Rust, so the surface is `--panel`, the text is `--fg`, guides and rulers
are `--line`/`--line2`, syntax is drawn from the existing palette
(`--badge-info`, `--green`, `--yellow`, `--red`, `--dim`), and it renders in
`tokens::MONO` at the console's own `13px`. The accent appears on the cursor
and on active/focused matches — live state — and never on ordinary syntax.

The editor is also why pointer capture exists: a panel drag that crosses the
editor would otherwise die the moment the pointer entered it.

### The pre-WASM boot shell

The one surface that cannot use this design system at runtime, because it
paints before the WASM module exists and so cannot read the injected
stylesheet. It is still held to the system — by generation rather than by
discipline (see The One Source Rule). It previews the real chrome: the panel
skeleton, the hairline borders, and the traffic-light cluster as three
discrete dots, which replaced a striped gradient that approximated the lights
and could never have been drawn in cells.

## Do's and Don'ts

### Do:

- **Do** put anything both renderers must agree on in `panel-kit-core` first, then let each backend translate it. "Web and terminal both happen to do this" is not the same as "core owns it."
- **Do** keep every visual decision expressible in terminal cells: tone, hairline, monospace cell, color. Check a new treatment against the cell grid before shipping it.
- **Do** reserve `--accent` for live state — activity, loading, selection, drag-in-flight — and keep it under ~10% of any screen.
- **Do** use `--line` for region dividers and `--line2` for object edges, never interchangeably.
- **Do** carry hierarchy with size, weight, case, and tracking, since there is only one font family.
- **Do** inset panel chrome into the border row and give reclaimed height to content.
- **Do** express depth as `--panel` on `--bg` plus a `--line2` edge.
- **Do** theme by overriding `:root` variables in a stylesheet layered after `panel_kit::CSS`, and mirror the change in `Theme` so the terminal stays the same product.

### Don't:

- **Don't** put `box-shadow` on a resting surface. Shadow belongs only to things that have genuinely left the plane, like the tooltip overlay.
- **Don't** use the accent on static chrome — borders, titles, dividers, or decoration. A green that is always on cannot signal.
- **Don't** introduce a third grey between `--fg` and `--dim`.
- **Don't** give the traffic lights macOS red/yellow/green semantics; none of them destroys anything, and the CMY assignment is the mapping.
- **Don't** add a proportional typeface, a gradient, a blur, or a translucent surface. None of them survives a cell grid.
- **Don't** set scannable text below `.65rem`, and don't set it at `.72rem` or below without caps and tracking.
- **Don't** hardcode a color, radius, or font stack that a `:root` token already names.
- **Don't** edit the generated boot stylesheet or a derived `Theme` field to change a colour; change the token and let both follow. Editing a derivative is how the three copies drifted in the first place.
- **Don't** solve a layout problem with a media query when the shell needs to change behavior — the breakpoint is evaluated in Rust so behavior and style can change together.
- **Don't** hand-copy a palette into a fourth place — a boot stylesheet, a JavaScript theme table, a native backend. Derive it from `panel_kit_core::tokens` or pin it with a test; those are the only two options.
- **Don't** grant an embedded third-party surface an exemption from the world. Theme it from the tokens; a foreign widget parked inside the console is more jarring than anything it saves.
- **Don't** express progress you have not measured. A determinate bar without a percentage, or a fabricated percentage, is worse than an honest indeterminate one.
